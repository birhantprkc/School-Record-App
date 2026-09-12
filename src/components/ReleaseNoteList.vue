<script setup>
import {AlertTriangle, Bug, Sparkles, Wrench} from '@lucide/vue'

// 릴리즈 노트 목록을 그리는 것만 한다. 어디에 담기는지(모달·섹션)는 호출부가 정한다.
//
// 같은 마크업이 "업데이트 완료" 모달과 업데이트 화면 두 곳에 필요하다. 복사해 두면
// 한쪽만 고쳐져 두 화면이 서로 다른 모양이 된다.
defineProps({
  notes: {type: Array, required: true},
})

// 표시 순서와 아이콘·색을 한 곳에서 정한다. 템플릿에 네 벌을 펼쳐 두면
// 항목이 하나 늘 때마다 같은 블록을 또 복사하게 된다.
const GROUPS = [
  {key: 'breaking', label: '주요 변경 사항', icon: AlertTriangle, tone: 'text-amber'},
  {key: 'features', label: '새 기능', icon: Sparkles, tone: 'text-blue-2'},
  {key: 'improvements', label: '개선 사항', icon: Wrench, tone: 'text-violet'},
  {key: 'bugFixes', label: '버그 수정', icon: Bug, tone: 'text-green'},
]
</script>

<template>
  <div class="flex flex-col gap-4">
    <template v-for="(note, index) in notes" :key="note.version">
      <div class="text-lg font-semibold text-ink-3 pb-0.5">
        v{{ note.version }}
        <span class="text-base font-normal text-ink-5 ml-1.5">{{ note.date }}</span>
      </div>

      <div v-for="group in GROUPS" :key="group.key">
        <div v-if="note[group.key]?.length" class="flex flex-col gap-2">
          <div class="flex items-center gap-1.5 text-lg font-semibold uppercase tracking-[0.05em] text-ink-3">
            <component :is="group.icon" :size="14" class="shrink-0" :class="group.tone"/>
            {{ group.label }}
          </div>
          <ul class="list-disc list-outside pl-[18px] flex flex-col gap-[5px] m-0">
            <li v-for="item in note[group.key]" :key="item" class="text-base text-ink-2 leading-[1.5]">{{ item }}</li>
          </ul>
        </div>
      </div>

      <hr v-if="index < notes.length - 1" class="border-0 border-t border-line-2 my-2 opacity-60"/>
    </template>
  </div>
</template>
