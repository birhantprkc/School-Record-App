import { describe, it, expect } from 'vitest'
import { byteLength, extractTopic } from './recordText'

describe('byteLength', () => {
  it('빈 값은 0', () => {
    expect(byteLength('')).toBe(0)
    expect(byteLength(null)).toBe(0)
    expect(byteLength(undefined)).toBe(0)
  })

  it('ASCII는 글자당 1바이트', () => {
    expect(byteLength('abc')).toBe(3)
  })

  it('한글은 글자당 3바이트 (UTF-8)', () => {
    expect(byteLength('가')).toBe(3)
    expect(byteLength('홍길동')).toBe(9)
  })

  it('줄바꿈은 CRLF 2바이트로 센다 — NEIS 기준', () => {
    // LF 1바이트로 세면 앱에서는 제한 이내인데 실제 입력에서 초과가 난다.
    expect(byteLength('a\nb')).toBe(4)
  })

  it('입력의 CR은 무시하고 LF만 CRLF로 환산한다 (중복 계산 방지)', () => {
    expect(byteLength('a\r\nb')).toBe(4)
    expect(byteLength('a\rb')).toBe(2)
  })

  it('연속 줄바꿈도 각각 2바이트', () => {
    expect(byteLength('a\n\nb')).toBe(6)
  })

  it('이모지 같은 4바이트 문자도 정확히 센다', () => {
    expect(byteLength('😀')).toBe(4)
  })

  it('여러 줄 기록: 한글 바이트 + 줄바꿈 2바이트', () => {
    // '가나' 6 + CRLF 2 + '다' 3 = 11
    expect(byteLength('가나\n다')).toBe(11)
  })

  it('제한 경계에서 LF를 1바이트로 세면 초과를 놓친다', () => {
    // 줄바꿈 100개짜리 기록: CRLF 기준 200바이트, LF 기준이면 100바이트.
    // 제한이 150이라면 앱은 통과시키지만 NEIS는 거부한다.
    expect(byteLength('\n'.repeat(100))).toBe(200)
  })
})

describe('extractTopic', () => {
  it('빈 값은 빈 문자열', () => {
    expect(extractTopic('')).toBe('')
    expect(extractTopic(null)).toBe('')
    expect(extractTopic('   ')).toBe('')
  })

  it('따옴표가 없으면 첫 문장을 반환', () => {
    expect(extractTopic('발표를 훌륭히 수행함. 이후 토론에도 참여함.'))
      .toBe('발표를 훌륭히 수행함.')
  })

  it('큰따옴표 안의 주제를 뽑는다', () => {
    expect(extractTopic('‘기후 변화와 우리’를 주제로 발표함.'))
      .toBe('기후 변화와 우리')
  })

  it('여러 주제는 쉼표로 잇는다', () => {
    expect(extractTopic('‘물의 순환’과 ‘탄소 순환’을 조사함.'))
      .toBe('물의 순환, 탄소 순환')
  })

  it('다른 값에 포함되는 값은 걸러낸다', () => {
    // '순환'은 '물의 순환'에 포함되므로 제외된다.
    expect(extractTopic('‘물의 순환’과 ‘순환’을 다룸.'))
      .toBe('물의 순환')
  })

  it('최대 5개까지만 반환', () => {
    const c = '‘가’, ‘나’, ‘다’, ‘라’, ‘마’, ‘바’를 조사함.'
    expect(extractTopic(c).split(', ')).toHaveLength(5)
  })

  it('낫표(「」·『』)도 인식', () => {
    expect(extractTopic('「토지」를 읽고 감상문을 씀.')).toBe('토지')
  })

  it('첫 줄에 문장 부호가 없으면 다음 줄의 첫 문장에서 찾는다', () => {
    // m 플래그로 ^가 모든 줄 시작에 걸리므로, 첫 문장은 첫 줄에 한정되지 않는다.
    expect(extractTopic('첫 줄 내용\n둘째 줄에 ‘주제’ 있음.')).toBe('주제')
  })

  it('어느 줄에도 문장 부호가 없으면 첫 줄을 쓴다', () => {
    expect(extractTopic('첫 줄 내용\n둘째 줄 내용')).toBe('첫 줄 내용')
  })

  it('문장 부호가 없으면 첫 줄을 100자로 자른다', () => {
    const long = '가'.repeat(150)
    const result = extractTopic(long)
    expect(result).toBe('가'.repeat(100))
  })

  it('따옴표가 열리기만 하면 주제로 인정하지 않는다', () => {
    expect(extractTopic('‘닫히지 않은 인용 발표함.')).toBe('‘닫히지 않은 인용 발표함.')
  })
})
