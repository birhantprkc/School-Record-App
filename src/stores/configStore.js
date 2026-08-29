import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { computed, reactive, ref } from 'vue'

const RECORD_CELL_SIZE_KEY = 'record_section_cell_text_size'
const THEME_MODE_KEY = 'theme_mode'
const EXPORT_C_SEPARATOR_KEY = 'export_c_separator'
const SEPARATOR_MAP = { space: ' ', newline: '\n', double_newline: '\n\n' }
const DEFAULT_SEPARATOR_KEY = 'space'
const DEFAULT_CELL_SIZE = 14

// RecordSection 툴바 토글 → APP_CONFIGS 키 매핑
const RECORD_TOOLBAR_CONFIG_KEYS = {
    freezeColumns: 'record_freeze_columns',
    smartScroll: 'record_smart_scroll',
    compactCell: 'record_compact_cell',
    highlightEmpty: 'record_highlight_empty',
    showPreview: 'record_show_preview',
    collapsePersonalInfo: 'record_collapse_personal_info',
}

const RECORD_TOOLBAR_DEFAULTS = {
    freezeColumns: true,
    smartScroll: true,
    compactCell: true,
    highlightEmpty: false,
    showPreview: false,
    collapsePersonalInfo: false,
}

const PREFERENCE_KEYS = [
    RECORD_CELL_SIZE_KEY,
    THEME_MODE_KEY,
    EXPORT_C_SEPARATOR_KEY,
    ...Object.values(RECORD_TOOLBAR_CONFIG_KEYS),
]

export const useConfigStore = defineStore('config', () => {
    const recordCellFontSize = ref(DEFAULT_CELL_SIZE)
    const encryptionEnabled = ref(false)
    const encryptionUnlocked = ref(false)
    const theme = ref('dark')
    const exportCSeparatorKey = ref(DEFAULT_SEPARATOR_KEY)
    const recordToolbar = reactive({ ...RECORD_TOOLBAR_DEFAULTS })
    const preferencesError = ref('')

    async function loadAll() {
        // 환경설정 로드 실패가 암호화 상태 조회를 막지 않도록 분리한다.
        // 실패는 preferencesError로 남겨 화면에서 알린다(무음 실패 금지).
        try {
            await loadPreferences()
        } catch (e) {
            preferencesError.value = `저장된 설정을 불러오지 못해 기본값으로 시작합니다: ${e}`
        }
        await refreshEncryptionStatus()
    }

    async function loadPreferences() {
        preferencesError.value = ''
        const stored = await invoke('get_configs', { keys: PREFERENCE_KEYS })

        const val = stored[RECORD_CELL_SIZE_KEY]
        if (val !== undefined) {
            const parsed = parseInt(val, 10)
            if (!isNaN(parsed)) recordCellFontSize.value = parsed
        }

        const themeVal = stored[THEME_MODE_KEY]
        if (themeVal === 'light' || themeVal === 'dark') {
            theme.value = themeVal
        }
        applyThemeToDom(theme.value)

        const sepVal = stored[EXPORT_C_SEPARATOR_KEY]
        if (sepVal && sepVal in SEPARATOR_MAP) {
            exportCSeparatorKey.value = sepVal
        }

        for (const [name, configKey] of Object.entries(RECORD_TOOLBAR_CONFIG_KEYS)) {
            const raw = stored[configKey]
            // 저장된 적 없거나 알 수 없는 값이면 기본값 유지
            if (raw === '1') recordToolbar[name] = true
            else if (raw === '0') recordToolbar[name] = false
            else recordToolbar[name] = RECORD_TOOLBAR_DEFAULTS[name]
        }
    }

    // 툴바 토글 저장. 실패 시 이전 값으로 되돌리고 예외를 다시 던진다(무음 실패 금지).
    async function setRecordToolbarOption(name, value) {
        const configKey = RECORD_TOOLBAR_CONFIG_KEYS[name]
        if (!configKey) throw new Error(`알 수 없는 기록 툴바 설정입니다: ${name}`)

        const next = Boolean(value)
        const prev = recordToolbar[name]
        if (prev === next) return
        recordToolbar[name] = next
        try {
            await invoke('set_config', { key: configKey, value: next ? '1' : '0' })
        } catch (e) {
            recordToolbar[name] = prev
            throw e
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
        try {
            await invoke('enable_encryption', { password })
        } finally {
            // 커밋 뒤 마무리 단계(백업 삭제·VACUUM)에서 실패해도 DB는 이미 암호화되어
            // 있다. 갱신하지 않으면 화면은 '비활성화됨'인데 다시 켜면 '이미 활성화
            // 되어 있습니다' 오류가 나서 사용자가 상황을 이해할 수 없다.
            await refreshEncryptionStatus()
        }
    }

    async function disableEncryption() {
        try {
            await invoke('disable_encryption')
        } finally {
            await refreshEncryptionStatus()
        }
    }

    async function changeEncryptionPassword(oldPassword, newPassword) {
        try {
            await invoke('change_encryption_password', { oldPassword, newPassword })
        } finally {
            await refreshEncryptionStatus()
        }
    }

    return {
        recordCellFontSize,
        encryptionEnabled,
        encryptionUnlocked,
        theme,
        exportCSeparatorKey,
        exportCSeparator,
        recordToolbar,
        preferencesError,
        loadAll,
        loadPreferences,
        refreshEncryptionStatus,
        setRecordCellFontSize,
        setRecordToolbarOption,
        setTheme,
        setExportCSeparator,
        unlockEncryption,
        enableEncryption,
        disableEncryption,
        changeEncryptionPassword,
    }
})
