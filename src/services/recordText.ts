/**
 * 생활기록부 텍스트 규칙.
 *
 * 앱의 핵심 판정 로직이라 컴포넌트가 아니라 여기에 둔다. SFC 안에 있으면
 * 테스트할 수 없고, 이 규칙이 틀리면 교사가 글자 수 초과를 모른 채 제출한다.
 */

const encoder = new TextEncoder()

/**
 * NEIS 기준 바이트 수를 센다.
 *
 * 줄바꿈은 CRLF 2바이트로 계산한다. NEIS가 그렇게 세기 때문에, LF 1바이트로
 * 세면 앱에서는 제한 이내인데 실제 입력에서 초과가 난다.
 */
export function byteLength(str: string | null | undefined): number {
  if (!str) return 0
  const normalized = String(str).replace(/\r/g, '').replace(/\n/g, '\r\n')
  return encoder.encode(normalized).length
}

/**
 * 활동 내용에서 주제를 뽑는다. 체크리스트 내보내기에서 쓴다.
 *
 * 1) 첫 문장을 찾고 2) 그 안의 따옴표 내용을 모아 3) 다른 값에 포함되는 것을
 * 걸러낸 뒤 4) 최대 5개를 잇는다. 따옴표가 없으면 첫 문장을 잘라 쓴다.
 */
export function extractTopic(content: string | null | undefined): string {
  if (!content?.trim()) return ''

  // 첫 문장 추출
  // - s 플래그 제거: .이 줄바꿈을 넘지 않도록
  // - m 플래그 추가: $가 각 줄 끝과 매칭
  // - \s*$ : 온점 뒤 공백만 남은 경우도 첫 문장으로 인정
  const sentenceMatch = content.match(
      /^(.+?[.!?][“”‘’"']?)(?=\s+[A-Z가-힣]|\s*$)/m
  )

  const firstSentence = sentenceMatch
      ? sentenceMatch[1].trim()
      : content.split(/\r?\n/)[0].slice(0, 100).trim()

  // 따옴표 내용 전부 수집
  // 열기: " (U+0022) ' (U+0027) “ ” ‘ ’ 「 『
  // 닫기: 위 + 」(U+300D) 』(U+300F) (「→」, 『→』 대응)
  const matches = [
    ...firstSentence.matchAll(
        /["'“”‘’「『]([^"'“”‘’」』]{1,120})["'“”‘’」』]/g
    )
  ]

  const values = matches
      .map(m => m[1].trim())
      .filter(Boolean)

  // 중첩 제거 (부분 포함 제거)
  const filtered = values.filter((val, i, arr) =>
      !arr.some((other, j) =>
          i !== j && other.includes(val)
      )
  )

  if (filtered.length > 0) {
    return filtered.slice(0, 5).join(', ')
  }

  const trimmed = firstSentence.slice(0, 100).trim()
  return trimmed + (firstSentence.length > 100 ? '…' : '')
}
