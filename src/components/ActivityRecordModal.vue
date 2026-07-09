<script setup>
import {computed, onMounted} from 'vue'
import BaseModal from './BaseModal.vue'
import {useActivityStore} from '../stores/activity'

const props = defineProps({
  activity: {type: Object, required: true},
})

const emit = defineEmits(['close'])

const activityStore = useActivityStore()

onMounted(() => {
  activityStore.fetchActivityRecords(props.activity.id)
})

function isNewGroup(list, idx) {
  if (idx === 0) return false
  const prev = list[idx - 1]
  const curr = list[idx]
  return prev.grade !== curr.grade || prev.class_num !== curr.class_num
}
</script>

<template>
  <BaseModal
      :title="`활동 기록 명단`"
      :label="activity.name"
      max-width="92vw"
      max-height="88vh"
      height="88vh"
      @close="emit('close')"
  >
    <div class="flex-1 overflow-auto">
      <!-- 로딩 -->
      <div v-if="activityStore.recordsLoading" class="flex items-center justify-center py-20">
        <p class="text-base text-ink-4 m-0">불러오는 중...</p>
      </div>

      <!-- 에러 -->
      <div v-else-if="activityStore.recordsError"
           class="mx-5 my-4 px-4 py-3 bg-red/[8%] border border-red/20 rounded-lg">
        <p class="text-base text-red/80 m-0">{{ activityStore.recordsError }}</p>
      </div>

      <!-- 빈 상태 -->
      <div v-else-if="activityStore.activityRecords.length === 0"
           class="flex flex-col items-center justify-center py-20 gap-2">
        <p class="text-base text-ink-4 m-0">기록된 학생이 없습니다.</p>
      </div>

      <!-- 테이블 -->
      <table v-else class="record-list-table border-separate border-spacing-0 w-full">
        <thead>
          <tr>
            <th class="tbl-th w-14 min-w-14 text-center">학년</th>
            <th class="tbl-th w-14 min-w-14 text-center">반</th>
            <th class="tbl-th w-14 min-w-14 text-center">번호</th>
            <th class="tbl-th w-24 min-w-24 text-center">이름</th>
            <th class="tbl-th text-left">{{ activity.name }}</th>
          </tr>
        </thead>
        <tbody>
          <tr
              v-for="(r, idx) in activityStore.activityRecords"
              :key="r.student_id"
              :class="isNewGroup(activityStore.activityRecords, idx) ? 'row-group-start' : ''"
          >
            <td class="tbl-td text-center text-ink-3">{{ r.grade }}</td>
            <td class="tbl-td text-center text-ink-3">{{ r.class_num }}</td>
            <td class="tbl-td text-center text-ink-3">{{ r.number }}</td>
            <td class="tbl-td text-center text-ink-2 font-medium">{{ r.student_name }}</td>
            <td class="tbl-td text-ink-2 whitespace-pre-wrap leading-[1.65]">{{ r.content }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <template #footer>
      <span class="text-base text-ink-5">
        총 {{ activityStore.activityRecords.length }}명
      </span>
      <button class="btn-secondary" @click="emit('close')">닫기</button>
    </template>
  </BaseModal>
</template>

<style scoped>
.tbl-th {
  position: sticky;
  top: 0;
  z-index: 2;
  padding: 10px 12px;
  font-size: 13px;
  font-weight: 600;
  color: var(--c-ink-2);
  background-color: var(--c-base);
  border-bottom: 1px solid var(--c-line);
  border-right: 1px solid var(--c-line);
  white-space: nowrap;
  letter-spacing: 0.03em;
}
.tbl-th:last-child { border-right: none; }

.tbl-td {
  padding: 10px 12px;
  font-size: var(--base-fs, 1rem);
  border-bottom: 1px solid var(--c-line-2);
  border-right: 1px solid var(--c-line-2);
  vertical-align: top;
}
.tbl-td:last-child { border-right: none; }

.row-group-start td {
  border-top: 1px solid color-mix(in srgb, var(--c-blue) 30%, transparent);
}
</style>
