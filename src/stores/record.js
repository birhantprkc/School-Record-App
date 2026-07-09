import {defineStore} from 'pinia'
import {ref} from 'vue'
import {invoke} from '@tauri-apps/api/core'

export const useRecordStore = defineStore('record', () => {
    const gridData = ref(null)
    const loading = ref(false)
    const error = ref('')

    async function fetchAreaGrid(areaId) {
        loading.value = true
        error.value = ''
        try {
            gridData.value = await invoke('get_area_grid', {areaId})
            return gridData.value
        } catch (e) {
            error.value = String(e)
            gridData.value = null
            throw e
        } finally {
            loading.value = false
        }
    }

    async function upsertRecord(activityId, studentId, content) {
        await invoke('upsert_record', {activityId, studentId, content})
    }

    async function fetchRecordHistory({activityId, studentId, limit, offset}) {
        return await invoke('get_record_history', {activityId, studentId, limit, offset})
    }

    async function saveHistorySnapshot({activityId, studentId, note}) {
        await invoke('save_history_snapshot', {activityId, studentId, note})
    }

    async function previewImportRecords(records) {
        try {
            return await invoke('preview_import_records', {records})
        } catch (e) {
            error.value = String(e)
            throw e
        }
    }

    async function bulkImportRecords(records) {
        loading.value = true
        error.value = ''
        try {
            return await invoke('bulk_import_records', {records})
        } catch (e) {
            error.value = String(e)
            throw e
        } finally {
            loading.value = false
        }
    }

    async function bulkQuickReplace(areaId, searchText, replaceWith) {
        return await invoke('bulk_quick_replace', {areaId, searchText, replaceWith})
    }

    return {gridData, loading, error, fetchAreaGrid, upsertRecord, fetchRecordHistory, saveHistorySnapshot, previewImportRecords, bulkImportRecords, bulkQuickReplace}
})
