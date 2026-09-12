<script setup>
import {X} from '@lucide/vue'
import {RELEASE_NOTES} from '../data/releaseNotes'
import {useEscapeKey} from '../composables/useEscapeKey.js'
import {useUpdateStore} from '../stores/updateStore.js'
import UpdateStatusPanel from './UpdateStatusPanel.vue'
import DeveloperInfoPanel from './DeveloperInfoPanel.vue'
import ReleaseNoteList from './ReleaseNoteList.vue'

// 시작 화면 전용이다. 작업 화면에서는 같은 내용을 UpdateSection이 섹션으로 보여준다
// (설정과 같은 층위). 파일을 열기 전에는 섹션을 얹을 자리가 없어 모달로 담는다 —
// 내용을 이루는 세 조각은 양쪽이 그대로 공유하므로 두 화면이 갈라지지 않는다.
const update = useUpdateStore()
const emit = defineEmits(['close'])

useEscapeKey(() => emit('close'))
</script>

<template>
  <div class="modal-overlay">
    <div class="modal-container max-w-[620px] p-8 flex flex-col max-h-[85vh]">
      <div class="flex items-start justify-between mb-6 shrink-0">
        <div>
          <h2 class="text-lg font-semibold text-ink m-0">업데이트 및 정보</h2>
          <p class="text-base text-ink-4 mt-1 mb-0">현재 버전 v{{ update.currentVersion }}</p>
        </div>
        <button
            class="flex bg-transparent border-none p-2 rounded-lg text-ink-5 cursor-pointer transition-colors hover:bg-line hover:text-ink-3"
            @click="emit('close')"
        >
          <X :size="20"/>
        </button>
      </div>

      <div class="modal-scroll flex-1 overflow-y-auto pr-1 flex flex-col gap-8">
        <UpdateStatusPanel/>

        <section class="flex flex-col gap-4">
          <h3 class="text-lg font-semibold text-ink m-0">개발자 정보</h3>
          <DeveloperInfoPanel/>
        </section>

        <section class="flex flex-col gap-4">
          <h3 class="text-lg font-semibold text-ink m-0">업데이트 기록</h3>
          <ReleaseNoteList :notes="RELEASE_NOTES"/>
        </section>
      </div>
    </div>
  </div>
</template>

<style scoped>
.modal-scroll::-webkit-scrollbar { width: 4px; }
.modal-scroll::-webkit-scrollbar-track { background: transparent; }
.modal-scroll::-webkit-scrollbar-thumb {
  background: var(--c-line-2);
  border-radius: 4px;
}
</style>
