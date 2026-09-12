import {defineStore} from 'pinia'
import {ref} from 'vue'
import {getVersion} from '@tauri-apps/api/app'

const RELEASES_API = 'https://api.github.com/repos/itmir913/School-Record-App/releases/latest'

/**
 * 업데이트 확인 상태.
 *
 * 시작 화면과 작업 화면 사이드바 두 곳에서 확인할 수 있고, 결과를 양쪽이 공유한다.
 * 시작 화면에서 확인해 새 버전을 찾았다면 파일을 연 뒤에도 사이드바에 배지가 남는다.
 *
 * **자동으로 조회하지 않는다.** 사용자가 버튼을 눌렀을 때만 요청이 나간다 —
 * PRIVACY.md가 그렇게 약속하고 있다. 여기에 onMounted 같은 자동 호출을 붙이지 말 것.
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
    status.value = 'checking'
    try {
      await loadCurrentVersion()
      const res = await fetch(RELEASES_API)
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const data = await res.json()
      latestVersion.value = data.tag_name
      releaseUrl.value = data.html_url
      status.value = normalize(data.tag_name) === normalize(currentVersion.value) ? 'latest' : 'found'
    } catch {
      // 인터넷이 없는 환경에서도 모든 기능이 정상 동작해야 한다. 오류는 모달 안에서만
      // 알리고 앱의 다른 동작에는 영향을 주지 않는다.
      status.value = 'error'
    }
  }

  return {status, currentVersion, latestVersion, releaseUrl, loadCurrentVersion, checkUpdate}
})
