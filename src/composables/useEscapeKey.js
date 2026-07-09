import { onMounted, onUnmounted } from 'vue'

export function useEscapeKey(callback) {
  function handler(e) {
    if (e.key === 'Escape') {
      e.stopImmediatePropagation()
      callback()
    }
  }
  onMounted(() => window.addEventListener('keydown', handler))
  onUnmounted(() => window.removeEventListener('keydown', handler))
}
