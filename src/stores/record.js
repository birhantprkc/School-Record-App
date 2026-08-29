import {defineStore} from 'pinia'
import {ref} from 'vue'
import {invoke} from '@tauri-apps/api/core'

export const useRecordStore = defineStore('record', () => {
    const gridData = ref(null)
    const loading = ref(false)
    const error = ref('')

    // 영역을 빠르게 바꾸면 이전 요청이 나중에 도착할 수 있다. 그때 옛 데이터로
    // gridData를 덮으면, 컴포넌트가 셀 내용은 새 영역 것으로 들고 있으면서
    // 학생·활동 목록만 옛 영역 것이 되어 다른 학생의 칸에 남의 기록이 보인다.
    let gridRequestSeq = 0

    async function fetchAreaGrid(areaId) {
        const seq = ++gridRequestSeq
        loading.value = true
        error.value = ''
        try {
            const data = await invoke('get_area_grid', {areaId})
            if (seq !== gridRequestSeq) return data
            gridData.value = data
            return data
        } catch (e) {
            if (seq === gridRequestSeq) {
                error.value = String(e)
                gridData.value = null
            }
            throw e
        } finally {
            // 늦게 온 응답이 진행 중인 최신 요청의 로딩 표시를 꺼뜨리지 않게 한다.
            if (seq === gridRequestSeq) loading.value = false
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
