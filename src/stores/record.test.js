import { describe, it, expect, vi, beforeEach } from 'vitest'
import { setActivePinia, createPinia } from 'pinia'

const invokeMock = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args) => invokeMock(...args) }))

const { useRecordStore } = await import('./record')

/** 응답을 임의 시점에 풀 수 있는 promise */
function deferred() {
  let resolve
  const promise = new Promise(r => { resolve = r })
  return { promise, resolve }
}

describe('fetchAreaGrid — 늦게 도착한 응답 처리', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    invokeMock.mockReset()
  })

  it('늦게 도착한 이전 요청이 최신 데이터를 덮지 않는다', async () => {
    // 영역 A를 고른 뒤 곧바로 B로 바꿨는데 A의 응답이 나중에 오는 상황.
    // 덮어쓰면 학생·활동 목록은 A인데 셀 내용은 B인 상태가 되어
    // 다른 학생 칸에 남의 기록이 보인다.
    const a = deferred()
    const b = deferred()
    invokeMock.mockReturnValueOnce(a.promise).mockReturnValueOnce(b.promise)

    const store = useRecordStore()
    const callA = store.fetchAreaGrid(1)
    const callB = store.fetchAreaGrid(2)

    b.resolve({ students: [], activities: [], records: [], area: 'B' })
    await callB
    a.resolve({ students: [], activities: [], records: [], area: 'A' })
    await callA

    expect(store.gridData.area).toBe('B')
  })

  it('늦게 도착한 응답도 호출자에게는 자기 데이터를 돌려준다', async () => {
    const a = deferred()
    const b = deferred()
    invokeMock.mockReturnValueOnce(a.promise).mockReturnValueOnce(b.promise)

    const store = useRecordStore()
    const callA = store.fetchAreaGrid(1)
    const callB = store.fetchAreaGrid(2)

    b.resolve({ area: 'B' })
    await callB
    a.resolve({ area: 'A' })

    await expect(callA).resolves.toEqual({ area: 'A' })
  })

  it('늦게 도착한 응답이 진행 중인 요청의 로딩 표시를 끄지 않는다', async () => {
    const a = deferred()
    const b = deferred()
    invokeMock.mockReturnValueOnce(a.promise).mockReturnValueOnce(b.promise)

    const store = useRecordStore()
    const callA = store.fetchAreaGrid(1)
    const callB = store.fetchAreaGrid(2)

    a.resolve({ area: 'A' })
    await callA
    expect(store.loading).toBe(true)  // B가 아직 진행 중

    b.resolve({ area: 'B' })
    await callB
    expect(store.loading).toBe(false)
  })

  it('늦게 도착한 실패가 최신 데이터를 지우지 않는다', async () => {
    const a = deferred()
    const b = deferred()
    invokeMock.mockReturnValueOnce(a.promise.then(() => { throw new Error('A 실패') }))
    invokeMock.mockReturnValueOnce(b.promise)

    const store = useRecordStore()
    const callA = store.fetchAreaGrid(1)
    const callB = store.fetchAreaGrid(2)

    b.resolve({ area: 'B' })
    await callB
    a.resolve()
    await expect(callA).rejects.toThrow('A 실패')

    expect(store.gridData.area).toBe('B')
    expect(store.error).toBe('')
  })

  it('최신 요청이 실패하면 오류를 남기고 데이터를 비운다', async () => {
    invokeMock.mockRejectedValueOnce(new Error('DB 오류'))
    const store = useRecordStore()

    await expect(store.fetchAreaGrid(1)).rejects.toThrow('DB 오류')
    expect(store.gridData).toBeNull()
    expect(store.error).toContain('DB 오류')
    expect(store.loading).toBe(false)
  })
})
