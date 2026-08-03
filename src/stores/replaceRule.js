import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { ref } from 'vue'
import { DEFAULT_REPLACE_RULES, DEFAULT_REPLACE_RULES_VERSION } from '../data/defaultReplaceRules'

const RULES_VERSION_KEY = 'replace_rules_version'

export const useReplaceRuleStore = defineStore('replaceRule', () => {
  const rules = ref([])
  const loading = ref(false)
  const error = ref('')
  const needsRuleUpdate = ref(false)

  async function checkRuleUpdateStatus() {
    const stored = await invoke('get_config', { key: RULES_VERSION_KEY })
    needsRuleUpdate.value = stored !== String(DEFAULT_REPLACE_RULES_VERSION)
  }

  async function fetchRules() {
    loading.value = true
    error.value = ''
    try {
      const fetched = await invoke('get_replace_rules')
      if (!Array.isArray(fetched) || fetched.length === 0) {
        await invoke('apply_default_replace_rules', { rules: DEFAULT_REPLACE_RULES })
        await invoke('set_config', { key: RULES_VERSION_KEY, value: String(DEFAULT_REPLACE_RULES_VERSION) })
        rules.value = await invoke('get_replace_rules')
      } else {
        rules.value = fetched
      }
      await checkRuleUpdateStatus()
    } catch (e) {
      error.value = e?.toString() ?? '규칙 목록을 불러오지 못했습니다.'
    } finally {
      loading.value = false
    }
  }

  async function applyRuleUpdate() {
    await invoke('apply_default_replace_rules', { rules: DEFAULT_REPLACE_RULES })
    await invoke('set_config', { key: RULES_VERSION_KEY, value: String(DEFAULT_REPLACE_RULES_VERSION) })
    needsRuleUpdate.value = false
    await fetchRules()
  }

  async function createRule(oldText, newText, priority, isRegex = false) {
    await invoke('create_replace_rule', { oldText, newText, isRegex, priority })
    await fetchRules()
  }

  async function updateRule(id, oldText, newText, enabled, priority, isRegex = false) {
    await invoke('update_replace_rule', { id, oldText, newText, isRegex, enabled, priority })
    await fetchRules()
  }

  async function deleteRule(id) {
    await invoke('delete_replace_rule', { id })
    await fetchRules()
  }

  async function previewReplace(scopeType, areaIds = []) {
    return await invoke('preview_replace', { scopeType, areaIds })
  }

  async function applyReplace(scopeType, areaIds = []) {
    return await invoke('apply_replace', { scopeType, areaIds })
  }

  return {
    rules,
    loading,
    error,
    needsRuleUpdate,
    fetchRules,
    applyRuleUpdate,
    createRule,
    updateRule,
    deleteRule,
    previewReplace,
    applyReplace,
  }
})
