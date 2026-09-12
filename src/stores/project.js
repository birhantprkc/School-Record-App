import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export const useProjectStore = defineStore('project', () => {
  const isOpen = ref(false)
  const filePath = ref('')

  // 파일을 여는 도중 나왔지만 **열기를 막을 정도는 아닌** 문제들.
  //
  // 홈 화면에서 띄우면 곧바로 작업 화면으로 넘어가며 사라지므로 여기 담아 두고
  // WorkspaceView가 배너로 보여준다. 조용히 넘기면 사용자는 파일 안에 평문이
  // 남았다는 것도, 이번 열기에 백업이 없다는 것도 알 수 없다.
  const openWarnings = ref([])

  function setProject(path) {
    filePath.value = path
    isOpen.value = true
  }

  function closeProject() {
    filePath.value = ''
    isOpen.value = false
    openWarnings.value = []
  }

  async function newProject(path) {
    await invoke('new_project', { path })
    setProject(path)
  }

  async function openProject(path) {
    await invoke('open_project', { path })
    setProject(path)
  }

  async function backupProject() {
    await invoke('backup_project')
  }

  async function migrateSchema() {
    await invoke('migrate_schema')
  }

  // 반환값: null = 버전 동일(모달 불필요), string = 이전 버전(모달 표시)
  // migrateSchema() 이후 호출해야 함
  async function checkAndUpdateVersion() {
    return await invoke('check_and_update_app_version')  // null | "" | "0.2.x"
  }

  return { isOpen, filePath, openWarnings, setProject, closeProject, newProject, openProject, backupProject, migrateSchema, checkAndUpdateVersion }
})
