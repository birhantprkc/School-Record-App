import {defineStore} from 'pinia'
import {ref} from 'vue'
import {getVersion} from '@tauri-apps/api/app'

const RELEASES_API = 'https://api.github.com/repos/itmir913/School-Record-App/releases/latest'
const REQUEST_TIMEOUT_MS = 10_000

/**
 * 업데이트 확인 상태.
 *
 * 시작 화면과 작업 화면 사이드바 두 곳에서 확인할 수 있고, 결과를 양쪽이 공유한다.
 * 시작 화면에서 확인해 새 버전을 찾았다면 파일을 연 뒤에도 사이드바에 배지가 남는다.
 *
 * **지켜야 할 불변식: `checkUpdate()`는 사용자의 클릭에서만 출발한다.**
 * PRIVACY.md가 "프로그램을 켤 때 자동으로 조회하지 않습니다"라고 약속하고 있다.
 * UpdateModal이 mount에서 한 번 부르는 것은 이 모달이 "업데이트 확인" 버튼으로만
 * 열리기 때문에 허용된다 — 그 모달을 클릭 없이 뜨게 만들면 그 순간 약속이 깨진다.
 * 앱 시작·라우트 진입·타이머처럼 클릭이 없는 자리에서는 절대 부르지 말 것.
 */
export const useUpdateStore = defineStore('update', () => {
  // 'idle' | 'checking' | 'latest' | 'found' | 'error'
  const status = ref('idle')
  const currentVersion = ref('')
  const latestVersion = ref('')
  const releaseUrl = ref('')

  /** 앞의 v를 떼고 비교한다. 태그는 `v0.3.0`, 앱 버전은 `0.3.0`으로 들어온다. */
  function normalize(version) {
    return String(version ?? '').replace(/^v/, '').trim()
  }

  async function loadCurrentVersion() {
    if (!currentVersion.value) {
      currentVersion.value = await getVersion()
    }
    return currentVersion.value
  }

  async function checkUpdate() {
    // 이미 확인 중이면 새로 요청하지 않는다. 중복 요청이 서로의 결과를 덮어쓴다.
    if (status.value === 'checking') return

    // 버전 조회(IPC)는 'checking'에 들어가기 전에 끝낸다. 아래 시간 제한은 fetch만
    // 끊으므로, 이것을 안에 두면 IPC가 돌아오지 않을 때 '확인 중' 화면에 갇힌다.
    try {
      await loadCurrentVersion()
    } catch {
      status.value = 'error'
      return
    }

    status.value = 'checking'

    // 반드시 시간 제한을 둔다. 학교 망의 캡티브 포털·프록시는 연결만 받아 두고
    // 응답을 주지 않는 경우가 있는데, 그러면 '확인 중' 화면에서 영영 빠져나오지
    // 못한다. 그 화면에는 버튼이 하나도 없어 재시도할 방법까지 사라진다.
    const abort = new AbortController()
    const timer = setTimeout(() => abort.abort(), REQUEST_TIMEOUT_MS)
    try {
      const res = await fetch(RELEASES_API, {signal: abort.signal})
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const data = await res.json()

      // 응답의 모양까지 확인한다. 프록시가 200과 함께 엉뚱한 JSON을 돌려주면
      // tag_name이 undefined가 되고, 그대로 두면 "새 버전이 있습니다 — "라는
      // 빈 제목과 열리지 않는 내려받기 버튼이 뜬다.
      if (!data?.tag_name || !data?.html_url) throw new Error('릴리즈 정보를 알아볼 수 없습니다')

      latestVersion.value = data.tag_name
      releaseUrl.value = data.html_url
      status.value = normalize(data.tag_name) === normalize(currentVersion.value) ? 'latest' : 'found'
    } catch {
      // 인터넷이 없는 환경에서도 모든 기능이 정상 동작해야 한다. 오류는 모달 안에서만
      // 알리고 앱의 다른 동작에는 영향을 주지 않는다.
      status.value = 'error'
    } finally {
      clearTimeout(timer)
    }
  }

  return {status, currentVersion, latestVersion, releaseUrl, loadCurrentVersion, checkUpdate}
})
