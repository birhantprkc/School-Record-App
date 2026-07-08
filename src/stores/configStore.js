import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { computed, ref } from 'vue'

const RECORD_CELL_SIZE_KEY = 'record_section_cell_text_size'
const THEME_MODE_KEY = 'theme_mode'
const EXPORT_C_SEPARATOR_KEY = 'export_c_separator'
const SEPARATOR_MAP = { space: ' ', newline: '\n', double_newline: '\n\n' }
const DEFAULT_SEPARATOR_KEY = 'space'
const DEFAULT_CELL_SIZE = 14

export const useConfigStore = defineStore('config', () => {
    const recordCellFontSize = ref(DEFAULT_CELL_SIZE)
    const encryptionEnabled = ref(false)
    const encryptionUnlocked = ref(false)
    const theme = ref('dark')
    const exportCSeparatorKey = ref(DEFAULT_SEPARATOR_KEY)

    async function loadAll() {
        await loadPreferences()
        await refreshEncryptionStatus()
    }

    async function loadPreferences() {
        const val = await invoke('get_config', { key: RECORD_CELL_SIZE_KEY })
        if (val !== null && val !== undefined) {
            const parsed = parseInt(val, 10)
            if (!isNaN(parsed)) recordCellFontSize.value = parsed
        }

        const themeVal = await invoke('get_config', { key: THEME_MODE_KEY })
        if (themeVal === 'light' || themeVal === 'dark') {
            theme.value = themeVal
        }
        applyThemeToDom(theme.value)

        const sepVal = await invoke('get_config', { key: EXPORT_C_SEPARATOR_KEY })
        if (sepVal && sepVal in SEPARATOR_MAP) {
            exportCSeparatorKey.value = sepVal
        }
    }

    function applyThemeToDom(mode) {
        if (mode === 'light') {
            document.documentElement.dataset.theme = 'light'
        } else {
            delete document.documentElement.dataset.theme
        }
    }

    async function setTheme(mode) {
        theme.value = mode
        await invoke('set_config', { key: THEME_MODE_KEY, value: mode })
        applyThemeToDom(mode)
    }

    async function refreshEncryptionStatus() {
        const status = await invoke('get_encryption_status')
        encryptionEnabled.value = status.enabled
        encryptionUnlocked.value = status.unlocked
    }

    async function setRecordCellFontSize(size) {
        recordCellFontSize.value = size
        await invoke('set_config', { key: RECORD_CELL_SIZE_KEY, value: String(size) })
    }

    const exportCSeparator = computed(() => SEPARATOR_MAP[exportCSeparatorKey.value] ?? ' ')

    async function setExportCSeparator(key) {
        if (!(key in SEPARATOR_MAP)) return
        exportCSeparatorKey.value = key
        await invoke('set_config', { key: EXPORT_C_SEPARATOR_KEY, value: key })
    }

    async function unlockEncryption(password) {
        await invoke('unlock_encryption', { password })
        await refreshEncryptionStatus()
    }

    async function enableEncryption(password) {
        await invoke('enable_encryption', { password })
        await refreshEncryptionStatus()
    }

    async function disableEncryption() {
        await invoke('disable_encryption')
        await refreshEncryptionStatus()
    }

    async function changeEncryptionPassword(oldPassword, newPassword) {
        await invoke('change_encryption_password', { oldPassword, newPassword })
        await refreshEncryptionStatus()
    }

    return {
        recordCellFontSize,
        encryptionEnabled,
        encryptionUnlocked,
        theme,
        exportCSeparatorKey,
        exportCSeparator,
        loadAll,
        loadPreferences,
        refreshEncryptionStatus,
        setRecordCellFontSize,
        setTheme,
        setExportCSeparator,
        unlockEncryption,
        enableEncryption,
        disableEncryption,
        changeEncryptionPassword,
    }
})
