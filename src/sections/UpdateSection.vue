<script setup>
import {RELEASE_NOTES} from '../data/releaseNotes'
import {useUpdateStore} from '../stores/updateStore.js'
import UpdateStatusPanel from '../components/UpdateStatusPanel.vue'
import DeveloperInfoPanel from '../components/DeveloperInfoPanel.vue'
import ReleaseNoteList from '../components/ReleaseNoteList.vue'

const update = useUpdateStore()
</script>

<template>
  <div class="flex flex-col h-full overflow-hidden box-border">

    <!-- 섹션 헤더 -->
    <div class="flex items-start justify-between px-10 py-9 border-b border-line flex-shrink-0 gap-4">
      <div>
        <h2 class="text-[22px] font-bold text-ink m-0 mb-1.5">업데이트 및 정보</h2>
        <p class="text-base text-ink-3 m-0">
          최신 버전을 확인하고, 지금까지 무엇이 바뀌었는지 볼 수 있습니다.
        </p>
      </div>
    </div>

    <div class="flex-1 overflow-y-auto px-10 py-8 flex flex-col gap-9">

      <!-- 업데이트 -->
      <section class="flex flex-col gap-4">
        <div>
          <h3 class="text-lg font-semibold text-ink m-0 mb-1">업데이트</h3>
          <p class="text-base text-ink-4 m-0">
            현재 버전 v{{ update.currentVersion }} · 버튼을 누르셨을 때만 조회하며, 프로그램을 켤 때 자동으로 확인하지 않습니다.
          </p>
        </div>
        <UpdateStatusPanel/>
      </section>

      <!-- 개발자 정보 -->
      <section class="flex flex-col gap-4">
        <div>
          <h3 class="text-lg font-semibold text-ink m-0 mb-1">개발자 정보</h3>
          <p class="text-base text-ink-4 m-0">문의나 제안은 아래로 보내주시면 됩니다.</p>
        </div>
        <DeveloperInfoPanel/>
      </section>

      <!-- 릴리즈 노트 (최신이 위) -->
      <section class="flex flex-col gap-4">
        <div>
          <h3 class="text-lg font-semibold text-ink m-0 mb-1">업데이트 기록</h3>
          <p class="text-base text-ink-4 m-0">최신 버전이 위에 옵니다.</p>
        </div>
        <ReleaseNoteList :notes="RELEASE_NOTES"/>
      </section>
    </div>
  </div>
</template>
