<script setup>
import {ref} from 'vue'
import {openUrl} from '@tauri-apps/plugin-opener'
import {Copy, ExternalLink, Github, ShieldCheck} from 'lucide-vue-next'

const HOMEPAGE = 'https://luminousky.com/teacher-utility-kit/neis-editor/'
const REPOSITORY = 'https://github.com/itmir913/School-Record-App'
const LICENSE_URL = 'https://polyformproject.org/licenses/noncommercial/1.0.0'
const PRIVACY_URL = 'https://github.com/itmir913/School-Record-App/blob/master/PRIVACY.md'
const CONTACT_EMAIL = 'hello@luminousky.com'

// 링크를 열지 못하는 환경(기본 브라우저 미등록 등)에서 버튼이 아무 반응도 없는
// 것처럼 보이면 안 된다. 실패하면 주소를 그대로 보여 준다.
const linkError = ref('')

async function open(url) {
  linkError.value = ''
  try {
    await openUrl(url)
  } catch {
    linkError.value = url
  }
}

// 메일 클라이언트가 없는 PC가 흔하다. mailto로 여는 대신 주소를 복사하게 한다.
const copyState = ref('')

async function copyEmail() {
  try {
    await navigator.clipboard.writeText(CONTACT_EMAIL)
    copyState.value = '복사했습니다.'
  } catch {
    copyState.value = '복사하지 못했습니다. 위 주소를 직접 드래그해 복사해 주세요.'
  }
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <dl class="grid grid-cols-[auto_1fr] gap-x-5 gap-y-2.5 m-0 text-base">
      <dt class="text-ink-4 m-0">만든 사람</dt>
      <dd class="text-ink-2 m-0">itmir913</dd>

      <dt class="text-ink-4 m-0">문의</dt>
      <dd class="text-ink-2 m-0 flex items-center gap-2.5 flex-wrap">
        <span class="select-text break-all">{{ CONTACT_EMAIL }}</span>
        <button
            class="flex items-center gap-1.5 py-1 px-2.5 rounded-lg border border-line bg-transparent text-base text-ink-3 cursor-pointer transition-colors hover:bg-line hover:text-ink"
            @click="copyEmail"
        >
          <Copy :size="14"/>
          복사
        </button>
      </dd>

      <dt class="text-ink-4 m-0">라이선스</dt>
      <dd class="text-ink-2 m-0">
        PolyForm Noncommercial 1.0.0 — 교육·비상업 목적으로만 무료로 사용할 수 있습니다.
      </dd>
    </dl>

    <p v-if="copyState" class="text-base text-ink-4 m-0">{{ copyState }}</p>

    <div class="flex gap-2.5 flex-wrap">
      <button
          class="flex items-center gap-2 py-2.5 px-3.5 rounded-btn border border-line bg-transparent text-base text-ink-3 cursor-pointer transition-colors hover:bg-line hover:text-ink"
          @click="open(HOMEPAGE)"
      >
        <ExternalLink :size="15"/>
        공식 페이지
      </button>
      <button
          class="flex items-center gap-2 py-2.5 px-3.5 rounded-btn border border-line bg-transparent text-base text-ink-3 cursor-pointer transition-colors hover:bg-line hover:text-ink"
          @click="open(REPOSITORY)"
      >
        <Github :size="15"/>
        GitHub
      </button>
      <button
          class="flex items-center gap-2 py-2.5 px-3.5 rounded-btn border border-line bg-transparent text-base text-ink-3 cursor-pointer transition-colors hover:bg-line hover:text-ink"
          @click="open(PRIVACY_URL)"
      >
        <ShieldCheck :size="15"/>
        개인정보처리방침
      </button>
      <button
          class="flex items-center gap-2 py-2.5 px-3.5 rounded-btn border border-line bg-transparent text-base text-ink-3 cursor-pointer transition-colors hover:bg-line hover:text-ink"
          @click="open(LICENSE_URL)"
      >
        <ExternalLink :size="15"/>
        라이선스 전문
      </button>
    </div>

    <p v-if="linkError" class="text-base text-ink-4 m-0 leading-relaxed">
      브라우저를 열지 못했습니다. 아래 주소를 직접 입력해 주세요.<br>
      <span class="text-ink-3 select-text break-all">{{ linkError }}</span>
    </p>
  </div>
</template>
