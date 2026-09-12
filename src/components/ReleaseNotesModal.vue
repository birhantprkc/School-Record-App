<script setup>
import { Sparkles } from '@lucide/vue'
import ReleaseNoteList from './ReleaseNoteList.vue'
import { useEscapeKey } from '../composables/useEscapeKey.js'

defineProps({
  notes: { type: Array, required: true },
})

const emit = defineEmits(['close'])

useEscapeKey(() => emit('close'))
</script>

<template>
  <div class="modal-overlay">
    <div class="modal-container max-w-[580px] p-8">

      <div class="flex items-center gap-[14px] mb-[22px]">
        <div class="flex items-center justify-center w-[42px] h-[42px] rounded-xl bg-blue/15 border border-blue/30 text-ink-3 shrink-0">
          <Sparkles :size="20" />
        </div>
        <div>
          <h2 class="text-lg font-semibold text-ink m-0 leading-[1.2]">업데이트 완료</h2>
          <p class="text-base text-ink-5 m-0">새로운 버전으로 정상 적용되었습니다.</p>
        </div>
      </div>

      <div v-if="notes.length" class="notes-body mb-6 max-h-[40vh] overflow-y-auto pr-1">
        <ReleaseNoteList :notes="notes"/>
      </div>

      <div v-else class="text-base text-ink-5 text-center py-5 mb-6">
        이 버전의 릴리즈 노트가 없습니다.
      </div>

      <div class="flex justify-end">
        <button class="btn-primary" @click="emit('close')">확인</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.notes-body::-webkit-scrollbar { width: 4px; }
.notes-body::-webkit-scrollbar-track { background: transparent; }
.notes-body::-webkit-scrollbar-thumb {
  background: var(--c-line-2);
  border-radius: 4px;
}
</style>
