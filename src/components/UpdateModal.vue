<script setup>
import {onMounted, ref} from 'vue'
import {openUrl} from '@tauri-apps/plugin-opener'
import {AlertTriangle, Check, CircleAlert, Download, X} from 'lucide-vue-next'
import {useEscapeKey} from '../composables/useEscapeKey.js'
import {useUpdateStore} from '../stores/updateStore.js'

const update = useUpdateStore()
const emit = defineEmits(['close'])

useEscapeKey(() => emit('close'))

// 모달을 여는 것 자체가 "확인해줘"라는 뜻이다. 이미 확인한 결과가 있으면 그대로 보여준다.
// 이 모달은 "업데이트 확인" 버튼으로만 열려야 한다 — updateStore의 주석 참고.
onMounted(() => {
  update.loadCurrentVersion()
  if (update.status === 'idle') update.checkUpdate()
})

// 기본 브라우저가 없거나 열기가 거부되면 버튼이 아무 반응도 없는 것처럼 보인다.
// 그때는 주소를 직접 보여 준다.
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
  <div class="modal-overlay">
    <div class="modal-container max-w-[560px] p-8">
      <div class="flex items-start justify-between mb-6">
        <div>
          <h2 class="text-lg font-semibold text-ink m-0">업데이트 확인</h2>
          <p class="text-base text-ink-4 mt-1 mb-0">현재 버전 v{{ update.currentVersion }}</p>
        </div>
        <button
            class="flex bg-transparent border-none p-2 rounded-lg text-ink-5 cursor-pointer transition-colors hover:bg-line hover:text-ink-3"
            @click="emit('close')"
        >
          <X :size="20"/>
        </button>
      </div>

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

        <div class="flex gap-2.5">
          <button
              v-if="update.status === 'found'"
              class="flex items-center justify-center gap-2.5 flex-1 py-3.5 px-5 rounded-btn text-base font-medium cursor-pointer border-none bg-blue text-white transition-[background-color,transform] hover:bg-blue-2 active:scale-[0.98]"
              @click="openRelease"
          >
            <Download :size="18"/>
            GitHub에서 내려받기
          </button>
          <button
              v-if="update.status === 'latest' || update.status === 'error' || update.status === 'found'"
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
    </div>
  </div>
</template>
