import {defineStore} from 'pinia'
import {ref} from 'vue'
import {invoke} from '@tauri-apps/api/core'

export const useStudentStore = defineStore('student', () => {
    const students = ref([])
    const loading = ref(false)
    const error = ref('')

    async function fetchStudents() {
        loading.value = true
        error.value = ''
        try {
            students.value = await invoke('get_students')
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

    async function createStudent(grade, classNum, number, name) {
        await invoke('create_student', {grade, classNum, number, name})
        await fetchStudents()
    }

    async function updateStudent(id, grade, classNum, number, name) {
        await invoke('update_student', {id, grade, classNum, number, name})
        await fetchStudents()
    }

    async function deleteStudent(id) {
        await invoke('delete_student', {id})
        await fetchStudents()
    }

    async function bulkUpsertStudents(students) {
        return await invoke('bulk_upsert_students', {students})
    }

    return {students, loading, error, fetchStudents, createStudent, updateStudent, deleteStudent, bulkUpsertStudents}
})
