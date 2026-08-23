<script setup>
import {computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch} from 'vue'
import {ALargeSmall, ArrowLeftRight, ArrowUpDown, Circle, CircleAlert, ChevronsRight, Eye, EyeOff, Maximize2, Minimize2, Moon, Pin, PinOff, Search, Sun} from 'lucide-vue-next'
import {useAreaStore} from '../stores/area'
import {useRecordStore} from '../stores/record'
import {useConfigStore} from '../stores/configStore'
import CellHistoryModal from '../components/CellHistoryModal.vue'
import QuickReplaceModal from '../components/QuickReplaceModal.vue'

const areaStore = useAreaStore()
const recordStore = useRecordStore()
const configStore = useConfigStore()

const selectedAreaId = ref(null)
const loadError = ref('')

// 툴바 토글은 configStore(APP_CONFIGS)에 저장되어 재진입/재시작 후에도 유지된다.
const freezeColumns = computed(() => configStore.recordToolbar.freezeColumns)
const smartScroll = computed(() => configStore.recordToolbar.smartScroll)
const compactCell = computed(() => configStore.recordToolbar.compactCell)
const highlightEmpty = computed(() => configStore.recordToolbar.highlightEmpty)
const showPreview = computed(() => configStore.recordToolbar.showPreview)
const collapsePersonalInfo = computed(() => configStore.recordToolbar.collapsePersonalInfo)
const collapsedActivities = ref(new Set())

const settingError = ref('')

// 토글 저장 실패를 사용자에게 알린다. 저장에 성공했을 때만 true.
async function setToolbarOption(name, value) {
  settingError.value = ''
  try {
    await configStore.setRecordToolbarOption(name, value)
    return true
  } catch (e) {
    settingError.value = `설정을 저장하지 못했습니다: ${e}`
    return false
  }
}

const FONT_SIZE_MIN = 10
const FONT_SIZE_MAX = 28

async function changeFontSize(delta) {
  const next = Math.min(FONT_SIZE_MAX, Math.max(FONT_SIZE_MIN, configStore.recordCellFontSize + delta))
  if (next === configStore.recordCellFontSize) return
  settingError.value = ''
  try {
    await configStore.setRecordCellFontSize(next)
  } catch (e) {
    settingError.value = `설정을 저장하지 못했습니다: ${e}`
  }
}

function toggleActivity(actId) {
  const next = new Set(collapsedActivities.value)
  if (next.has(actId)) next.delete(actId)
  else next.add(actId)
  collapsedActivities.value = next
}

const savingState = ref(new Map())
const cellContent = reactive(new Map())
const debounceTimers = new Map()
const showQuickReplace = ref(false)

onMounted(async () => {
  await areaStore.fetchAreas()
})

function clearAllTimers() {
  debounceTimers.forEach(t => clearTimeout(t))
  debounceTimers.clear()
}

onBeforeUnmount(clearAllTimers)

async function reloadGrid(id) {
  await recordStore.fetchAreaGrid(id)
  if (selectedAreaId.value !== id) return
  cellContent.clear()
  for (const r of recordStore.gridData.records) {
    cellContent.set(cellKey(r.activity_id, r.student_id), r.content)
  }
  savingState.value = new Map()
  await resyncHeights()
}

watch(selectedAreaId, async (id) => {
  clearAllTimers()
  loadError.value = ''
  if (!id) {
    recordStore.gridData = null
    return
  }
  try {
    await reloadGrid(id)
    if (selectedAreaId.value !== id) return
    collapsedActivities.value = new Set()
  } catch (e) {
    if (selectedAreaId.value !== id) return
    loadError.value = String(e)
  }
})

// 디바운스 대기 중인 셀을 즉시 DB에 저장한다.
// QuickReplace는 DB를 직접 읽어 교체하므로, 실행 전에 미저장 내용을 먼저 반영해야 한다.
async function flushPendingDebounces() {
  if (debounceTimers.size === 0) return
  const saves = []
  for (const [key, timerId] of debounceTimers) {
    clearTimeout(timerId)
    const [actId, stuId] = key.split('-').map(Number)
    saves.push(recordStore.upsertRecord(actId, stuId, cellContent.get(key) ?? ''))
  }
  // clear는 await 성공 후에 실행 — 실패 시 재시도에서 다시 flush 가능하도록
  await Promise.all(saves)
  debounceTimers.clear()
}

async function handleQuickReplaceDone() {
  showQuickReplace.value = false
  const id = selectedAreaId.value
  if (!id) return
  try {
    await reloadGrid(id)
  } catch (e) {
    if (selectedAreaId.value !== id) return
    loadError.value = String(e)
  }
}

function truncateName(name, max = 10) {
  return name.length > max ? name.slice(0, max) + '…' : name
}

function cellKey(activityId, studentId) {
  return `${activityId}-${studentId}`
}

function getCellContent(activityId, studentId) {
  return cellContent.get(cellKey(activityId, studentId)) ?? ''
}

function normalizeForCopy(str) {
  return str.replace(/[\r\n]+/g, ' ').replace(/ {2,}/g, ' ').trim()
}

function getCellSavingState(activityId, studentId) {
  return savingState.value.get(cellKey(activityId, studentId))
}

function verticalPadding(el) {
  const cs = getComputedStyle(el)
  return parseFloat(cs.paddingTop) + parseFloat(cs.paddingBottom)
}

// 활동 셀에서 textarea를 뺀 나머지 높이 (바이트/Copy 줄 + td 상하 패딩)
function cellOverhead(input) {
  const td = input.closest('td')
  if (!td) return 0
  let extra = verticalPadding(td)
  for (const child of td.children) {
    if (child !== input) extra += child.offsetHeight
  }
  return extra
}

// 내용이 잘리지 않는 최소 높이. box-sizing이 border-box이므로
// scrollHeight(테두리 미포함)에 테두리 높이를 더해야 2px이 잘리지 않는다.
function naturalHeight(el) {
  el.style.height = 'auto'
  return el.scrollHeight + (el.offsetHeight - el.clientHeight)
}

function syncRowHeights(tr) {
  const inputs = Array.from(tr.querySelectorAll('.cell-input'))
  if (!inputs.length) return

  // 1) 모든 셀을 내용에 맞는 자연 높이로 되돌린 뒤 가장 큰 값을 찾는다.
  let target = Math.max(...inputs.map(naturalHeight))

  // 2) 미리보기 열이 켜져 있으면 그 내용까지 담기도록 높이를 키운다.
  //    td는 행 높이만큼 늘어나므로 td.scrollHeight를 재면 이미 늘어난 행 높이를
  //    되먹임해 셀이 실제 필요보다 계속 커진다. 내부 래퍼를 재야 한다.
  const previewInner = tr.querySelector('.preview-inner')
  if (previewInner) {
    const needed = previewInner.offsetHeight
        + verticalPadding(previewInner.parentElement)
        - cellOverhead(inputs[0])
    target = Math.max(target, needed)
  }

  inputs.forEach(el => { el.style.height = target + 'px' })
}

function syncAllRows() {
  document.querySelectorAll('.record-table tr').forEach(syncRowHeights)
}

// 셀높이 고정(compact)일 때는 인라인 높이를 걷어내 CSS의 max-h가 적용되게 한다.
function clearAllRowHeights() {
  document.querySelectorAll('.cell-input').forEach(el => { el.style.height = '' })
}

// 열 구성이나 글자 크기가 바뀌면 줄바꿈 위치가 달라져 기존 높이가 어긋난다.
// 토글 함수마다 호출을 흩뿌리면 빠뜨리기 쉬우므로 상태 변화를 한 곳에서 감시한다.
async function resyncHeights() {
  await nextTick()
  if (compactCell.value) clearAllRowHeights()
  else syncAllRows()
}

watch(
    () => [
      configStore.recordToolbar.freezeColumns,
      configStore.recordToolbar.collapsePersonalInfo,
      configStore.recordToolbar.showPreview,
      configStore.recordToolbar.compactCell,
      configStore.recordCellFontSize,
      collapsedActivities.value,
    ],
    resyncHeights
)

async function toggleCompactCell() {
  await setToolbarOption('compactCell', !compactCell.value)
}

function onCellInput(activityId, studentId, event) {
  const key = cellKey(activityId, studentId)
  const content = event.target.value
  cellContent.set(key, content)
  if (!compactCell.value) {
    const tr = event.target.closest('tr')
    if (tr) syncRowHeights(tr)
  }

  if (savingState.value.get(key) === 'error') {
    const cleared = new Map(savingState.value)
    cleared.delete(key)
    savingState.value = cleared
  }

  if (debounceTimers.has(key)) {
    clearTimeout(debounceTimers.get(key))
  }
  const timer = setTimeout(() => saveCell(activityId, studentId, content), 1000)
  debounceTimers.set(key, timer)
}

function onGridWheel(event) {
  const el = event.currentTarget
  if (Math.abs(event.deltaX) > 0) {
    event.preventDefault()
    el.scrollLeft += event.deltaX
    return
  }
  if (!smartScroll.value) {
    if (event.shiftKey) {
      event.preventDefault()
      el.scrollLeft += event.deltaY
    }
    return
  }
  const inFixedArea = event.target.closest('.td-fixed, .th-fixed') !== null
  if (inFixedArea) return
  event.preventDefault()
  el.scrollLeft += event.deltaY
}

async function saveCell(activityId, studentId, content) {
  const key = cellKey(activityId, studentId)
  const stateMap = new Map(savingState.value)
  stateMap.set(key, 'saving')
  savingState.value = stateMap
  try {
    await recordStore.upsertRecord(activityId, studentId, content)
    const next = new Map(savingState.value)
    next.set(key, 'saved')
    savingState.value = next
    setTimeout(() => {
      const clear = new Map(savingState.value)
      clear.delete(key)
      savingState.value = clear
    }, 500)
  } catch (e) {
    const next = new Map(savingState.value)
    next.set(key, 'error')
    savingState.value = next
  }
}

const _encoder = new TextEncoder()

function byteLength(str) {
  if (!str) return 0
  const safeStr = String(str)
  const normalizedStr = safeStr.replace(/\r/g, '').replace(/\n/g, '\r\n')
  return _encoder.encode(normalizedStr).length
}

const byteLimit = computed(() => {
  if (!selectedAreaId.value || !areaStore.areas.length) return null
  const area = areaStore.areas.find(a => a.id === selectedAreaId.value)
  return area ? area.byte_limit : null
})

function isOverLimit(activityId, studentId) {
  if (!byteLimit.value) return false
  const content = getCellContent(activityId, studentId)
  return byteLength(content) > byteLimit.value
}

const totalBytesCache = computed(() => {
  if (!recordStore.gridData) return new Map()
  const map = new Map()
  for (const student of recordStore.gridData.students) {
    let total = 0
    for (const act of recordStore.gridData.activities) {
      total += byteLength(getCellContent(act.id, student.id))
    }
    map.set(student.id, total)
  }
  return map
})

function studentTotalBytes(studentId) {
  return totalBytesCache.value.get(studentId) ?? 0
}

function isStudentOverLimit(studentId) {
  if (!byteLimit.value) return false
  return studentTotalBytes(studentId) > byteLimit.value
}

function isStudentEmpty(studentId) {
  return studentTotalBytes(studentId) === 0
}

function getCellBgClass(actId, studentId) {
  const state = getCellSavingState(actId, studentId)
  if (state === 'saving') return '!bg-blue/30'
  if (state === 'saved') return '!bg-green/30'
  if (state === 'error') return '!bg-red/40 outline outline-2 outline-red/80'
  if (isOverLimit(actId, studentId)) return 'cell-overlimit-bg'
  return ''
}

const copiedCells = ref(new Set())
const copiedStudents = ref(new Set())

function markCopied(setRef, key) {
  setRef.value.add(key)
  setRef.value = new Set(setRef.value)
  setTimeout(() => {
    setRef.value.delete(key)
    setRef.value = new Set(setRef.value)
  }, 1000)
}

async function copyCell(activityId, studentId) {
  try {
    await navigator.clipboard.writeText(normalizeForCopy(getCellContent(activityId, studentId)))
    markCopied(copiedCells, cellKey(activityId, studentId))
  } catch (e) {
    console.error('클립보드 복사 실패:', e)
  }
}

async function copyStudentRecord(studentId) {
  const joined = recordStore.gridData.activities
    .map(act => normalizeForCopy(getCellContent(act.id, studentId)))
    .filter(c => c !== '')
    .join(configStore.exportCSeparator ?? ' ')
  try {
    await navigator.clipboard.writeText(joined)
    markCopied(copiedStudents, studentId)
  } catch (e) {
    console.error('클립보드 복사 실패:', e)
  }
}

const nameColLeft = computed(() => collapsePersonalInfo.value ? 'left-0' : 'left-144px')
const previewColLeft = computed(() => collapsePersonalInfo.value ? 'left-100px' : 'left-244px')
const byteColLeft = computed(() => {
  if (collapsePersonalInfo.value) return showPreview.value ? 'left-460px' : 'left-100px'
  return showPreview.value ? 'left-604px' : 'left-244px'
})

function studentRowBgClass(studentId) {
  if (isStudentOverLimit(studentId)) return 'cell-overlimit'
  if (highlightEmpty.value && isStudentEmpty(studentId)) return 'cell-empty-row'
  return ''
}

const activityColorMap = computed(() => {
  if (!recordStore.gridData) return new Map()
  return new Map(
    recordStore.gridData.activities.map((act, i) => [act.id, i % 12])
  )
})

function getActivityColorClass(actId) {
  const idx = activityColorMap.value.get(actId)
  return idx !== undefined ? `act-hl-${idx}` : ''
}

function studentPreviewSpans(studentId) {
  if (!recordStore.gridData) return []
  return recordStore.gridData.activities
    .map(act => ({ act, content: getCellContent(act.id, studentId) }))
    .filter(s => s.content.trim() !== '')
}

async function togglePreview() {
  if (!await setToolbarOption('showPreview', !showPreview.value)) return
  // 미리보기 열은 셀 높이가 자동일 때만 행 높이가 맞으므로 함께 해제한다.
  if (showPreview.value && compactCell.value) {
    await setToolbarOption('compactCell', false)
  }
}

async function focusActivityCell(actId, studentId) {
  if (collapsedActivities.value.has(actId)) {
    const next = new Set(collapsedActivities.value)
    next.delete(actId)
    collapsedActivities.value = next
    await nextTick()
  }
  const el = document.querySelector(`[data-cell-key="${cellKey(actId, studentId)}"]`)
  if (!el) return
  el.scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'nearest' })
  el.focus()
}

const historyModal = ref(null)

function openHistory(act, student) {
  historyModal.value = {
    activityId: act.id,
    studentId: student.id,
    activityName: act.name,
    studentName: student.name,
    currentContent: getCellContent(act.id, student.id),
  }
}

function isNewGroup(students, index) {
  if (index === 0) return false
  const prev = students[index - 1]
  const curr = students[index]
  return prev.grade !== curr.grade || prev.class_num !== curr.class_num
}
</script>

<template>
  <div class="h-full" :style="{ '--cell-fs': configStore.recordCellFontSize + 'px' }">
    <div
        class="flex flex-col box-border"
        :class="freezeColumns ? 'h-full overflow-hidden' : ''"
    >

      <!-- 툴바 -->
      <div class="flex flex-wrap items-center px-6 py-2 border-b border-line-2 shrink-0 gap-2 bg-base min-h-15">
        <div class="flex items-center gap-2 min-w-0">
          <select
              v-model="selectedAreaId"
              class="py-2.5 px-3.5 rounded-btn border border-line bg-base text-ink text-base cursor-pointer outline-none min-w-[180px] focus:border-blue/50"
          >
            <option :value="null" disabled>영역(Area) 선택</option>
            <option v-for="area in areaStore.areas" :key="area.id" :value="area.id">{{ area.name }}</option>
          </select>
        </div>

        <div class="flex items-center flex-wrap justify-end gap-2 ml-auto">
          <!-- 글자 크기 -->
          <div class="flex items-center gap-1 py-2 px-3.5 rounded-lg border border-blue/30 bg-blue/[0.08] text-blue-2" title="셀 글자 크기">
            <ALargeSmall :size="15" class="shrink-0 mr-0.5 opacity-70"/>
            <button
                class="flex items-center justify-center w-[22px] h-[22px] rounded-[5px] border-none bg-transparent text-base text-ink-3 leading-none cursor-pointer transition-[background-color,color] shrink-0 enabled:hover:bg-blue/20 enabled:hover:text-ink-2 disabled:opacity-30 disabled:cursor-default"
                :disabled="configStore.recordCellFontSize <= FONT_SIZE_MIN"
                @click="changeFontSize(-1)"
            >−</button>
            <span class="text-base font-semibold text-blue-2 min-w-8 text-center">{{ configStore.recordCellFontSize }}px</span>
            <button
                class="flex items-center justify-center w-[22px] h-[22px] rounded-[5px] border-none bg-transparent text-base text-ink-3 leading-none cursor-pointer transition-[background-color,color] shrink-0 enabled:hover:bg-blue/20 enabled:hover:text-ink-2 disabled:opacity-30 disabled:cursor-default"
                :disabled="configStore.recordCellFontSize >= FONT_SIZE_MAX"
                @click="changeFontSize(+1)"
            >+</button>
          </div>

          <button
              class="toolbar-btn"
              :class="freezeColumns ? 'text-blue-2 border-blue/30 bg-blue/[0.08]' : 'text-ink-3 border-line'"
              title="틀고정 켜기/끄기"
              @click="setToolbarOption('freezeColumns', !freezeColumns)"
          >
            <Pin v-if="freezeColumns" :size="15"/>
            <PinOff v-else :size="15"/>
            틀고정
          </button>

          <button
              class="toolbar-btn"
              :class="smartScroll ? 'text-blue-2 border-blue/30 bg-blue/[0.08]' : 'text-ink-3 border-line'"
              title="스마트 스크롤: 활동 영역에서 휠 → 좌우 스크롤"
              @click="setToolbarOption('smartScroll', !smartScroll)"
          >
            <ArrowLeftRight v-if="smartScroll" :size="15"/>
            <ArrowUpDown v-else :size="15"/>
            스마트스크롤
          </button>

          <button
              class="toolbar-btn"
              :class="compactCell ? 'text-blue-2 border-blue/30 bg-blue/[0.08]' : 'text-ink-3 border-line'"
              title="셀 높이: 고정(ON) / 자동(OFF)"
              @click="toggleCompactCell"
          >
            <Minimize2 v-if="compactCell" :size="15"/>
            <Maximize2 v-else :size="15"/>
            {{ compactCell ? '셀높이 고정' : '셀높이 자동' }}
          </button>

          <button
              class="toolbar-btn"
              :class="highlightEmpty ? 'text-amber border-amber/30 bg-amber/[0.08]' : 'text-ink-3 border-line'"
              title="기록이 없는 학생 행 강조 켜기/끄기"
              @click="setToolbarOption('highlightEmpty', !highlightEmpty)"
          >
            <CircleAlert v-if="highlightEmpty" :size="15"/>
            <Circle v-else :size="15"/>
            빈 학생
          </button>

          <button
              class="toolbar-btn"
              :class="showPreview ? 'text-blue-2 border-blue/30 bg-blue/[0.08]' : 'text-ink-3 border-line'"
              title="미리보기 열 켜기/끄기"
              @click="togglePreview"
          >
            <Eye v-if="showPreview" :size="15"/>
            <EyeOff v-else :size="15"/>
            미리보기
          </button>

          <button
              class="flex items-center justify-center py-2.5 px-3.5 rounded-lg border bg-transparent cursor-pointer transition-[background-color,color,border-color] text-ink-3 border-line hover:bg-line hover:text-ink-2 disabled:opacity-40 disabled:cursor-not-allowed"
              :disabled="!selectedAreaId || !recordStore.gridData"
              title="빠른 텍스트 교체"
              @click="showQuickReplace = true"
          >
            <Search :size="15"/>
          </button>

          <button
              class="flex items-center justify-center py-2.5 px-3.5 rounded-lg border bg-transparent cursor-pointer transition-[background-color,color,border-color] text-ink-3 border-line hover:bg-line hover:text-ink-2"
              :title="configStore.theme === 'dark' ? '라이트 모드로 전환' : '다크 모드로 전환'"
              @click="configStore.setTheme(configStore.theme === 'dark' ? 'light' : 'dark')"
          >
            <Sun v-if="configStore.theme === 'dark'" :size="15"/>
            <Moon v-else :size="15"/>
          </button>
        </div>
      </div>

      <!-- 설정 저장/로드 실패 알림 -->
      <div
          v-if="settingError || configStore.preferencesError"
          class="px-6 py-2 border-b border-line-2 shrink-0 bg-amber/[0.08]"
      >
        <p class="text-base text-amber m-0">{{ settingError || configStore.preferencesError }}</p>
      </div>

      <!-- 빈 상태: 영역 미선택 -->
      <div v-if="!selectedAreaId" class="flex items-center justify-center flex-1 min-h-[300px] p-12">
        <p class="text-base text-ink-3 m-0 text-center leading-relaxed">상단 드롭다운 메뉴에서 영역(Area)을 선택하세요.</p>
      </div>

      <!-- 로딩 -->
      <div v-else-if="recordStore.loading" class="flex items-center justify-center flex-1 min-h-[300px] p-12">
        <p class="text-base text-ink-3 m-0 text-center leading-relaxed">불러오는 중...</p>
      </div>

      <!-- 에러 -->
      <div v-else-if="loadError" class="flex items-center justify-center flex-1 min-h-[300px] p-12">
        <p class="text-base text-red m-0 text-center leading-relaxed">{{ loadError }}</p>
      </div>

      <!-- 그리드 없음 -->
      <div
          v-else-if="!recordStore.gridData || recordStore.gridData.students.length === 0 || recordStore.gridData.activities.length === 0"
          class="flex items-center justify-center flex-1 min-h-[300px] p-12"
      >
        <p class="text-base text-ink-3 m-0 text-center leading-relaxed">
          <template v-if="recordStore.gridData && recordStore.gridData.students.length === 0">이 영역에 배정된 학생이 없습니다. 영역(Area) 관리에서 <strong><u>학생 배정</u></strong> 버튼을 눌러 학생을 배정하세요.</template>
          <template v-else-if="recordStore.gridData && recordStore.gridData.activities.length === 0">이 영역에 등록된 활동이 없습니다. 영역(Area) 관리에서 <strong><u>포함할 활동</u></strong>을 추가하세요.</template>
          <template v-else>데이터를 불러올 수 없습니다.</template>
        </p>
      </div>

      <!-- 그리드 -->
      <div
          v-else
          :class="freezeColumns ? 'flex-1 overflow-auto' : 'overflow-x-auto'"
          @wheel="onGridWheel"
      >
        <table class="record-table border-separate border-spacing-0 min-w-full">
          <thead>
          <tr>
            <th
                v-if="!collapsePersonalInfo"
                class="th-fixed text-[13px] font-semibold text-ink-2 bg-base py-2.5 px-[5px] border-b border-line border-r border-line whitespace-nowrap text-center tracking-[0.03em] w-12 min-w-12 max-w-12 left-0 cursor-pointer select-none underline hover:bg-surface"
                :class="freezeColumns ? 'sticky top-0 z-[5]' : ''"
                title="클릭하여 학년·반·번호 숨기기"
                @click="setToolbarOption('collapsePersonalInfo', true)"
            >학년</th>
            <th
                v-if="!collapsePersonalInfo"
                class="th-fixed text-[13px] font-semibold text-ink-2 bg-base py-2.5 px-[5px] border-b border-line border-r border-line whitespace-nowrap text-center tracking-[0.03em] w-12 min-w-12 max-w-12 left-48px cursor-pointer select-none underline hover:bg-surface"
                :class="freezeColumns ? 'sticky top-0 z-[5]' : ''"
                title="클릭하여 학년·반·번호 숨기기"
                @click="setToolbarOption('collapsePersonalInfo', true)"
            >반</th>
            <th
                v-if="!collapsePersonalInfo"
                class="th-fixed text-[13px] font-semibold text-ink-2 bg-base py-2.5 px-[5px] border-b border-line border-r border-line whitespace-nowrap text-center tracking-[0.03em] w-12 min-w-12 max-w-12 left-96px cursor-pointer select-none underline hover:bg-surface"
                :class="freezeColumns ? 'sticky top-0 z-[5]' : ''"
                title="클릭하여 학년·반·번호 숨기기"
                @click="setToolbarOption('collapsePersonalInfo', true)"
            >번호</th>
            <th
                class="th-fixed text-[13px] font-semibold bg-base py-2.5 px-2.5 border-b border-line border-r border-line whitespace-nowrap text-center tracking-[0.03em] w-[100px] min-w-[100px] max-w-[100px]"
                :class="[
                  freezeColumns ? 'sticky top-0 z-[5]' : '',
                  nameColLeft,
                  collapsePersonalInfo
                    ? 'text-amber border-l-2 border-l-amber/50 cursor-pointer select-none underline'
                    : 'text-ink-2'
                ]"
                :title="collapsePersonalInfo ? '학년·반·번호 숨김 — 클릭하여 복원' : ''"
                @click="setToolbarOption('collapsePersonalInfo', false)"
            >
              <span class="flex items-center justify-center gap-1">
                <ChevronsRight v-if="collapsePersonalInfo" :size="13" class="shrink-0"/>
                이름
              </span>
            </th>
            <th
                v-if="showPreview"
                class="th-fixed text-[13px] font-semibold text-ink-2 bg-base py-2.5 px-2.5 border-b border-line border-r border-line whitespace-nowrap text-center tracking-[0.03em] w-[360px] min-w-[360px] max-w-[360px]"
                :class="[freezeColumns ? 'sticky top-0 z-[5]' : '', previewColLeft]"
            >미리보기</th>
            <th
                class="th-fixed text-[13px] font-semibold text-ink-2 bg-base py-2.5 px-2.5 border-b border-line border-r border-line whitespace-nowrap text-center tracking-[0.03em] w-[90px] min-w-[90px] max-w-[90px]"
                :class="[freezeColumns ? 'sticky top-0 z-[5] freeze-border-right' : '', byteColLeft]"
            >바이트</th>
            <th
                v-for="act in recordStore.gridData.activities"
                :key="act.id"
                class="text-[13px] font-semibold text-ink-2 bg-base py-2.5 px-2.5 border-b border-line border-r border-line text-center tracking-[0.03em] w-[320px] min-w-[280px] cursor-pointer select-none underline hover:text-ink-2 hover:bg-surface"
                :class="[
                  freezeColumns ? 'sticky top-0 z-[3]' : '',
                  showPreview ? getActivityColorClass(act.id) : '',
                  collapsedActivities.has(act.id)
                    ? 'act-col-collapsed whitespace-nowrap overflow-hidden text-ellipsis !py-2.5 !px-2 text-blue-2'
                    : 'whitespace-normal break-keep'
                ]"
                @click="toggleActivity(act.id)"
            >{{ collapsedActivities.has(act.id) ? truncateName(act.name) : act.name }}</th>
          </tr>
          </thead>
          <tbody>
          <tr
              v-for="(student, idx) in recordStore.gridData.students"
              :key="student.id"
              :class="isNewGroup(recordStore.gridData.students, idx) ? 'row-group-start' : ''"
          >
            <!-- 학년 -->
            <td
                v-if="!collapsePersonalInfo"
                class="td-fixed text-ink-3 bg-base py-1.5 px-1 border-b border-line-2 border-r border-line-2 align-top text-center w-12 min-w-12 max-w-12 left-0"
                :class="[freezeColumns ? 'sticky z-[2]' : '', studentRowBgClass(student.id)]"
            >{{ student.grade }}</td>
            <!-- 반 -->
            <td
                v-if="!collapsePersonalInfo"
                class="td-fixed text-ink-3 bg-base py-1.5 px-1 border-b border-line-2 border-r border-line-2 align-top text-center w-12 min-w-12 max-w-12 left-48px"
                :class="[freezeColumns ? 'sticky z-[2]' : '', studentRowBgClass(student.id)]"
            >{{ student.class_num }}</td>
            <!-- 번호 -->
            <td
                v-if="!collapsePersonalInfo"
                class="td-fixed text-ink-3 bg-base py-1.5 px-1 border-b border-line-2 border-r border-line-2 align-top text-center w-12 min-w-12 max-w-12 left-96px"
                :class="[freezeColumns ? 'sticky z-[2]' : '', studentRowBgClass(student.id)]"
            >{{ student.number }}</td>
            <!-- 이름 -->
            <td
                class="td-fixed text-ink-2 bg-base py-1.5 px-2.5 border-b border-line-2 border-r border-line-2 align-top text-center w-[100px] min-w-[100px] max-w-[100px] break-all"
                :class="[
                  freezeColumns ? 'sticky z-[2]' : '',
                  nameColLeft,
                  collapsePersonalInfo ? 'border-l-2 border-l-amber/40' : '',
                  studentRowBgClass(student.id)
                ]"
            >{{ student.name }}</td>
            <!-- 미리보기 -->
            <td
                v-if="showPreview"
                class="preview-col td-fixed bg-base text-ink py-2 px-3 border-b border-line-2 border-r border-line-2 align-top w-[360px] min-w-[360px] max-w-[360px] leading-relaxed overflow-hidden break-all"
                :class="[freezeColumns ? 'sticky z-[2]' : '', previewColLeft, studentRowBgClass(student.id)]"
            >
              <!-- td는 행 높이만큼 늘어나므로, 실제 내용 높이는 이 래퍼로 측정한다 -->
              <div class="preview-inner">
                <template v-for="(seg, i) in studentPreviewSpans(student.id)" :key="seg.act.id">
                  <span v-if="i > 0" class="whitespace-pre-wrap">{{ configStore.exportCSeparator ?? ' ' }}</span>
                  <span
                      class="act-hl-base cursor-pointer hover:opacity-75 transition-opacity duration-100"
                      :class="getActivityColorClass(seg.act.id)"
                      :title="seg.act.name"
                      @click="focusActivityCell(seg.act.id, student.id)"
                  >{{ seg.content }}</span>
                </template>
              </div>
            </td>
            <!-- 바이트 -->
            <td
                class="td-fixed bg-base py-1.5 px-2.5 border-b border-line-2 border-r border-line-2 align-top text-center w-[90px] min-w-[90px] max-w-[90px]"
                :class="[freezeColumns ? 'sticky z-[2] freeze-border-right' : '', byteColLeft, studentRowBgClass(student.id)]"
            >
              <span
                  v-if="byteLimit"
                  class="text-[12px] block leading-tight"
                  :class="isStudentOverLimit(student.id) ? 'text-red font-bold' : (highlightEmpty && isStudentEmpty(student.id) ? 'text-amber' : 'text-ink-3')"
              >{{ studentTotalBytes(student.id) }} / {{ byteLimit }} Bytes</span>
              <button
                  class="bg-transparent border-none px-4 py-2 text-[11px] text-blue-2/70 cursor-pointer leading-none hover:text-blue-2 hover:underline block mx-auto"
                  @click.stop="copyStudentRecord(student.id)"
              >{{ copiedStudents.has(student.id) ? 'Copied!' : 'Copy' }}</button>
            </td>
            <!-- 활동 셀 -->
            <td
                v-for="act in recordStore.gridData.activities"
                :key="act.id"
                class="text-ink-2 py-1.5 px-2 border-b border-line-2 border-r border-line-2 align-top relative transition-[background-color] duration-500 w-[600px] min-w-[480px]"
                :class="[
                  collapsedActivities.has(act.id) ? 'act-col-collapsed !p-0 !bg-blue/[0.04]' : getCellBgClass(act.id, student.id)
                ]"
            >
              <template v-if="!collapsedActivities.has(act.id)">
                <textarea
                    class="cell-input w-full box-border py-1.5 px-2 leading-[1.5] bg-transparent border rounded-[6px] text-ink resize-none outline-none transition-[border-color,background-color] duration-150 min-h-[60px] overflow-y-auto placeholder:text-ink-5"
                    :class="compactCell ? 'max-h-[60px] overflow-y-auto' : ''"
                    :data-cell-key="cellKey(act.id, student.id)"
                    :value="getCellContent(act.id, student.id)"
                    @input="onCellInput(act.id, student.id, $event)"
                    rows="1"
                />
                <div
                    class="text-[11px] text-right pt-0.5 flex items-center justify-end gap-[5px]"
                    :class="isOverLimit(act.id, student.id) ? 'text-red' : 'text-ink-5'"
                >
                  {{ byteLength(getCellContent(act.id, student.id) || '') }} Bytes
                  <span class="text-ink-5 select-none">|</span>
                  <button
                      class="bg-transparent border-none p-0 text-[11px] text-blue-2/70 cursor-pointer leading-none hover:text-blue-2 hover:underline"
                      @click.stop="copyCell(act.id, student.id)"
                  >{{ copiedCells.has(cellKey(act.id, student.id)) ? 'Copied!' : 'Copy' }}</button>
                  <span class="text-ink-5 select-none">|</span>
                  <button
                      class="bg-transparent border-none p-0 text-[11px] text-blue-2/70 cursor-pointer leading-none hover:text-blue-2 hover:underline"
                      @click.stop="openHistory(act, student)"
                  >History</button>
                </div>
              </template>
            </td>
          </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- 히스토리 모달 -->
    <CellHistoryModal
        v-if="historyModal"
        :activity-id="historyModal.activityId"
        :student-id="historyModal.studentId"
        :activity-name="historyModal.activityName"
        :student-name="historyModal.studentName"
        :current-content="historyModal.currentContent"
        @close="historyModal = null"
    />

    <!-- 빠른 텍스트 교체 모달 -->
    <QuickReplaceModal
        v-if="showQuickReplace && selectedAreaId && recordStore.gridData"
        :area-id="selectedAreaId"
        :grid-data="recordStore.gridData"
        :cell-content="cellContent"
        :flush-pending="flushPendingDebounces"
        @close="showQuickReplace = false"
        @done="handleQuickReplaceDone"
    />
  </div>
</template>

<style scoped>
.record-table { font-size: var(--cell-fs, 14px); }

/* 반 구분선 — 자식 td 선택자 */
.row-group-start td {
  border-top: 1px solid color-mix(in srgb, var(--c-blue) 30%, transparent);
}

.cell-input {
  font-size: calc(var(--cell-fs, 14px) + 2px);
  border-color: var(--c-cell-border);
}
.cell-input:hover {
  border-color: var(--c-line-2);
}
.cell-input:focus {
  border-color: color-mix(in srgb, var(--c-blue) 70%, transparent);
  background-color: var(--c-cell-focus);
}

/* sticky 셀 행 강조 — 불투명 (반투명 bg는 스크롤 시 뒤 내용이 비침) */
.td-fixed.cell-overlimit,
.cell-overlimit-bg {
  background-color: var(--c-overlimit-bg) !important;
}

.td-fixed.cell-empty-row {
  background-color: color-mix(in srgb, var(--c-amber) 18%, var(--c-base)) !important;
}
</style>
