<script setup>
import {onMounted, ref} from 'vue'
import {openUrl} from '@tauri-apps/plugin-opener'
import {AlertTriangle, Check, CircleAlert, Download} from 'lucide-vue-next'
import {useUpdateStore} from '../stores/updateStore.js'

const update = useUpdateStore()

// 이 패널이 화면에 나타난다는 것은 사용자가 "업데이트 확인"을 눌렀다는 뜻이다.
// 그래서 여기서 한 번 조회한다 — updateStore의 불변식(클릭에서만 출발한다)을
// 지키려면 **이 패널을 클릭 없이 띄우지 말 것.**
onMounted(() => {
  update.loadCurrentVersion()
  if (update.status === 'idle') update.checkUpdate()
})

// 기본 브라우저가 없거나 열기가 거부되면 버튼이 아무 반응도 없는 것처럼 보인다.
const openError = ref('')

async function openRelease() {
  openError.value = ''
  try {
    await openUrl(update.releaseUrl)
  } catch {
    openError.value = update.releaseUrl
  }
}
</script>

<template>
  <div class="flex flex-col gap-4.5">
    <!-- 확인 중 -->
    <div v-if="update.status === 'checking'" class="flex flex-col items-center gap-4.5 py-7 text-ink-4 text-base">
      <div class="w-7 h-7 border-2 border-line-2 border-t-blue rounded-full animate-spin"/>
      <p class="m-0">최신 버전을 확인하는 중…</p>
    </div>

    <!-- 최신 버전 -->
    <div
        v-else-if="update.status === 'latest'"
        class="flex items-start gap-3.5 py-4.5 px-5 rounded-btn border bg-green/[0.08] border-green/30 text-green"
    >
      <Check :size="24" class="shrink-0 mt-px"/>
      <div>
        <p class="text-base font-semibold m-0 mb-1">최신 버전입니다</p>
        <p class="text-base opacity-75 m-0 leading-relaxed">현재 사용 중인 버전이 최신입니다.</p>
      </div>
    </div>

    <!-- 새 버전 있음 -->
    <div
        v-else-if="update.status === 'found'"
        class="flex items-start gap-3.5 py-4.5 px-5 rounded-btn border bg-amber/[0.08] border-amber/30 text-amber"
    >
      <AlertTriangle :size="24" class="shrink-0 mt-px"/>
      <div>
        <p class="text-base font-semibold m-0 mb-1">새 버전이 있습니다 — {{ update.latestVersion }}</p>
        <p class="text-base opacity-75 m-0 leading-relaxed">GitHub에서 최신 버전을 내려받을 수 있습니다.</p>
      </div>
    </div>

    <!-- 오류 -->
    <div
        v-else-if="update.status === 'error'"
        class="flex items-start gap-3.5 py-4.5 px-5 rounded-btn border bg-red/[0.08] border-red/30 text-red"
    >
      <CircleAlert :size="24" class="shrink-0 mt-px"/>
      <div>
        <p class="text-base font-semibold m-0 mb-1">확인에 실패했습니다</p>
        <p class="text-base opacity-75 m-0 leading-relaxed">
          인터넷 연결을 확인한 후 다시 시도해 주세요. 인터넷이 없어도 프로그램의 모든 기능은 그대로 사용하실 수 있습니다.
        </p>
      </div>
    </div>

    <div v-if="update.status !== 'checking'" class="flex gap-2.5">
      <button
          v-if="update.status === 'found'"
          class="flex items-center justify-center gap-2.5 flex-1 py-3.5 px-5 rounded-btn text-base font-medium cursor-pointer border-none bg-blue text-white transition-[background-color,transform] hover:bg-blue-2 active:scale-[0.98]"
          @click="openRelease"
      >
        <Download :size="18"/>
        GitHub에서 내려받기
      </button>
      <button
          class="flex-1 py-3.5 px-5 rounded-btn text-base font-medium cursor-pointer bg-transparent border border-line text-ink-3 transition-colors hover:bg-line hover:text-ink"
          @click="update.checkUpdate()"
      >
        다시 확인
      </button>
    </div>

    <p v-if="openError" class="text-base text-ink-4 m-0 leading-relaxed">
      브라우저를 열지 못했습니다. 아래 주소를 직접 입력해 주세요.<br>
      <span class="text-ink-3 select-text break-all">{{ openError }}</span>
    </p>
  </div>
</template>
