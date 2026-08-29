import {defineStore} from 'pinia'
import {ref} from 'vue'
import {invoke} from '@tauri-apps/api/core'

export const useActivityStore = defineStore('activity', () => {
    const activities = ref([])  // ActivityDetail[]
    const loading = ref(false)
    const error = ref('')

    const activityRecords = ref([])  // ActivityRecordItem[]
    const recordsLoading = ref(false)
    const recordsError = ref('')
    let fetchRecordsGen = 0

    async function fetchActivities() {
        loading.value = true
        error.value = ''
        try {
            activities.value = await invoke('get_activities')
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

    async function deleteActivity(id) {
        await invoke('delete_activity', {id})
        await fetchActivities()
    }

    async function saveActivity({mode, id, name, areaIds}) {
        loading.value = true
        error.value = ''
        try {
            let activityId
            if (mode === 'add') {
                activityId = await invoke('create_activity', {name})
            } else {
                activityId = id
                await invoke('update_activity', {id: activityId, name})
            }
            await invoke('set_activity_areas', {activityId, areaIds})
            await fetchActivities()
        } catch (e) {
            error.value = String(e)
            throw e
        } finally {
            loading.value = false
        }
    }

    async function createActivitiesBatch(names) {
        loading.value = true
        error.value = ''
        try {
            const nameToId = await invoke('create_activities_batch', {names})
            await fetchActivities()
            return nameToId
        } catch (e) {
            error.value = String(e)
            throw e
        } finally {
            loading.value = false
        }
    }

    async function createActivity(name) {
        loading.value = true
        error.value = ''
        try {
            const id = await invoke('create_activity', {name})
            await fetchActivities()
            return id
        } catch (e) {
            error.value = String(e)
            throw e
        } finally {
            loading.value = false
        }
    }

    async function fetchActivityRecords(activityId) {
        const gen = ++fetchRecordsGen
        recordsLoading.value = true
        recordsError.value = ''
        activityRecords.value = []
        try {
            const result = await invoke('get_activity_records', {activityId})
            if (gen === fetchRecordsGen) activityRecords.value = result
        } catch (e) {
            if (gen === fetchRecordsGen) recordsError.value = String(e)
        } finally {
            if (gen === fetchRecordsGen) recordsLoading.value = false
        }
    }

    return {
        activities, loading, error,
        fetchActivities, deleteActivity, saveActivity, createActivity, createActivitiesBatch,
        activityRecords, recordsLoading, recordsError, fetchActivityRecords,
    }
})
