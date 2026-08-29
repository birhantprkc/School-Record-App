import {defineStore} from 'pinia'
import {ref} from 'vue'
import {invoke} from '@tauri-apps/api/core'

export const useAreaStore = defineStore('area', () => {
    const areas = ref([])
    const loading = ref(false)
    const error = ref('')

    async function fetchAreas() {
        loading.value = true
        error.value = ''
        try {
            areas.value = await invoke('get_areas')
        } catch (e) {
            error.value = String(e)
            // record.js와 같은 계약: error에 담고 **다시 던진다.**
            // 삼키면 호출부의 try/catch가 죽은 코드가 되고, 읽기가 실패해도
            // "데이터가 없음"과 구분되지 않는 빈 목록이 그대로 보인다.
            throw e
        } finally {
            loading.value = false
        }
    }

    async function createArea(name, byteLimit) {
        const id = await invoke('create_area', {name, byteLimit})
        await fetchAreas()
        return id
    }

    async function updateArea(id, name, byteLimit) {
        await invoke('update_area', {id, name, byteLimit})
        await fetchAreas()
    }

    async function deleteArea(id) {
        await invoke('delete_area', {id})
        await fetchAreas()
    }

    async function setAreaActivities(areaId, activityIds) {
        await invoke('set_area_activities', {areaId, activityIds})
        await fetchAreas()
    }

    async function getAreaStudents(areaId) {
        return await invoke('get_area_students', {areaId})
    }

    async function setAreaStudents(areaId, studentIds) {
        await invoke('set_area_students', {areaId, studentIds})
    }

    return {areas, loading, error, fetchAreas, createArea, updateArea, deleteArea, setAreaActivities, getAreaStudents, setAreaStudents}
})
