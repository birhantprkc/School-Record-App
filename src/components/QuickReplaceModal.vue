<script setup>
import { ref, computed } from 'vue'
import { Search, ArrowRight } from 'lucide-vue-next'
import BaseModal from './BaseModal.vue'
import DiffView from './DiffView.vue'
import { useRecordStore } from '../stores/record'

const props = defineProps({
  areaId:        { type: Number,   required: true },
  gridData:      { type: Object,   required: true },
  cellContent:   { type: Object,   required: true },
  flushPending:  { type: Function, required: true },
})

const emit = defineEmits(['close', 'done'])

const recordStore = useRecordStore()

const searchText     = ref('')
const replaceWith    = ref('')
// 검색 버튼을 눌렀을 때 확정되는 검색어. 프리뷰는 이 값 기준으로만 갱신된다.
const committedSearch = ref('')
const isApplying     = ref(false)
const applyError     = ref('')

const studentMap = computed(() => {
  const m = new Map()
  for (const s of props.gridData.students) m.set(s.id, s)
  return m
})

const activityMap = computed(() => {
  const m = new Map()
  for (const a of props.gridData.activities) m.set(a.id, a)
  return m
})

const canSearch  = computed(() => searchText.value.length > 0)
const hasPreview = computed(() => committedSearch.value.length > 0)

// 매칭 여부만 판단 — committedSearch가 바뀔 때만 재계산
const matchedItems = computed(() => {
  if (!hasPreview.value) return []
  const q = committedSearch.value
  const result = []
  for (const [key, content] of props.cellContent) {
    if (!content.includes(q)) continue
    const [actId, stuId] = key.split('-').map(Number)
    const student  = studentMap.value.get(stuId)
    const activity = activityMap.value.get(actId)
    if (!student || !activity) continue
    result.push({
      key,
      studentLabel: `${student.grade}학년 ${student.class_num}반 ${student.number}번 ${student.name}`,
      activityName: activity.name,
      before: content,
    })
  }
  return result
})

// after 컬럼만 — replaceWith 타이핑 시 cellContent 전체 스캔 없이 재계산
const previewItems = computed(() => {
  const q = committedSearch.value
  return matchedItems.value.map(item => ({
    ...item,
    after: item.before.replaceAll(q, replaceWith.value),
  }))
})

const hasChanges = computed(() => previewItems.value.length > 0)

async function commitSearch() {
  const q = searchText.value
  if (!q) return
  await props.flushPending()
  committedSearch.value = q
  applyError.value = ''
}

async function applyReplace() {
  if (!committedSearch.value || !hasPreview.value || !hasChanges.value || isApplying.value) return
  isApplying.value = true
  applyError.value = ''
  try {
    // Rust는 DB를 직접 읽으므로, 미저장 셀을 먼저 flush해야 프리뷰와 결과가 일치한다.
    await props.flushPending()
    const count = await recordStore.bulkQuickReplace(props.areaId, committedSearch.value, replaceWith.value)
    emit('done', count)
  } catch (e) {
    applyError.value = String(e)
  } finally {
    isApplying.value = false
  }
}
</script>

<template>
  <BaseModal
    title="빠른 텍스트 교체"
    max-width="92vw"
    max-height="88vh"
    height="88vh"
    @close="emit('close')"
  >
    <div class="quick-replace-body">
      <!-- 입력 영역 -->
      <div class="quick-replace-inputs">
        <div class="quick-replace-field">
          <label class="quick-replace-label">찾을 텍스트</label>
          <input
            v-model="searchText"
            class="quick-replace-input"
            placeholder="공백 포함 가능"
            autocomplete="off"
            spellcheck="false"
            :disabled="isApplying"
            @keydown.enter="commitSearch"
          />
        </div>
        <ArrowRight :size="18" class="quick-replace-arrow" />
        <div class="quick-replace-field">
          <label class="quick-replace-label">바꿀 텍스트</label>
          <input
            v-model="replaceWith"
            class="quick-replace-input"
            placeholder="빈 값으로 두면 삭제"
            autocomplete="off"
            spellcheck="false"
            :disabled="isApplying"
          />
        </div>
        <button
          class="quick-replace-search-btn"
          :disabled="!canSearch || isApplying"
          @click="commitSearch"
        >
          <Search :size="15" />
          검색
        </button>
      </div>

      <!-- 미리보기 영역 -->
      <div class="quick-replace-preview">
        <!-- 검색 전 -->
        <div v-if="!hasPreview" class="quick-replace-empty">
          <Search :size="32" class="quick-replace-empty-icon" />
          <p>찾을 텍스트를 입력하고 검색 버튼을 누르세요.</p>
        </div>

        <!-- 검색 결과 없음 -->
        <div v-else-if="previewItems.length === 0" class="quick-replace-empty">
          <Search :size="32" class="quick-replace-empty-icon" />
          <p>일치하는 항목이 없습니다.</p>
        </div>

        <!-- 미리보기 목록 -->
        <template v-else>
          <p class="quick-replace-count">총 {{ previewItems.length }}개 항목이 변경됩니다.</p>
          <div class="quick-replace-list">
            <div
              v-for="item in previewItems"
              :key="item.key"
              class="quick-replace-item"
            >
              <div class="quick-replace-item-meta">
                <span class="quick-replace-item-student">{{ item.studentLabel }}</span>
                <span class="quick-replace-item-sep">·</span>
                <span class="quick-replace-item-activity">{{ item.activityName }}</span>
              </div>
              <DiffView :before="item.before" :after="item.after" />
            </div>
          </div>
        </template>
      </div>

      <!-- 에러 -->
      <p v-if="applyError" class="quick-replace-error">{{ applyError }}</p>
    </div>

    <template #footer>
      <span class="text-base text-ink-3">
        {{ hasPreview && hasChanges ? `${previewItems.length}개 항목 변경 예정` : '' }}
      </span>
      <div class="flex items-center gap-2">
        <button class="btn-secondary" @click="emit('close')">취소</button>
        <button
          class="btn-primary"
          :disabled="!hasPreview || !hasChanges || isApplying"
          @click="applyReplace"
        >
          {{ isApplying ? '처리 중…' : '바꾸기' }}
        </button>
      </div>
    </template>
  </BaseModal>
</template>
