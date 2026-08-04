export interface DefaultReplaceRule {
  oldText: string
  newText: string
  priority: number
  isRegex?: boolean
}

// DEFAULT_REPLACE_RULES 항목을 추가/변경할 때마다 +1 (기존 사용자의 "기본 규칙 갱신" 버튼이 감지하는 기준값)
export const DEFAULT_REPLACE_RULES_VERSION = 1

export const DEFAULT_REPLACE_RULES: DefaultReplaceRule[] = [
  // 대시·물결·기호 → ASCII
  { oldText: '–', newText: '-', priority: 0 },    // 엔 대시
  { oldText: '—', newText: '-', priority: 1 },    // 엠 대시
  { oldText: '−', newText: '-', priority: 2 },    // 수학 마이너스
  { oldText: '~', newText: '-', priority: 3 },    // 물결
  { oldText: '～', newText: '-', priority: 4 },   // 전각 물결
  { oldText: '〜', newText: '-', priority: 5 },   // CJK 물결
  { oldText: '…', newText: '.', priority: 6 },    // 줄임표
  { oldText: '•', newText: '·', priority: 7 },    // 블릿 → 가운뎃점

  // 전각 구두점 → 반각
  { oldText: '，', newText: ',', priority: 8 },    // 전각 쉼표
  { oldText: '、', newText: ',', priority: 9 },    // CJK 쉼표
  { oldText: '︐', newText: ',', priority: 10 },   // 세로쓰기 쉼표
  { oldText: '︑', newText: ',', priority: 11 },   // 세로쓰기 CJK 쉼표
  { oldText: '﹐', newText: ',', priority: 12 },   // 소형 쉼표
  { oldText: '﹑', newText: ',', priority: 13 },   // 소형 CJK 쉼표
  { oldText: '．', newText: '.', priority: 14 },   // 전각 마침표
  { oldText: '。', newText: '.', priority: 15 },   // CJK 마침표
  { oldText: '：', newText: ':', priority: 16 },   // 전각 콜론
  { oldText: '；', newText: ';', priority: 17 },   // 전각 세미콜론
  { oldText: '？', newText: '?', priority: 18 },   // 전각 물음표
  { oldText: '！', newText: '!', priority: 19 },   // 전각 느낌표

  // 전각·CJK 괄호 → 반각
  { oldText: '（', newText: '(', priority: 20 },   // 전각 소괄호
  { oldText: '）', newText: ')', priority: 21 },
  { oldText: '【', newText: "'", priority: 22 },   // 검정 대괄호
  { oldText: '】', newText: "'", priority: 23 },
  { oldText: '〈', newText: "'", priority: 24 },   // 홑꺾쇠
  { oldText: '〉', newText: "'", priority: 25 },
  { oldText: '《', newText: "'", priority: 26 },   // 겹꺾쇠
  { oldText: '》', newText: "'", priority: 27 },

  // 따옴표·인용부호 통일
  { oldText: '‘', newText: "'", priority: 28 },   // 곡선 홑따옴표 (좌)
  { oldText: '’', newText: "'", priority: 29 },   // 곡선 홑따옴표 (우)
  { oldText: '“', newText: "'", priority: 30 },   // 곡선 쌍따옴표 (좌)
  { oldText: '”', newText: "'", priority: 31 },   // 곡선 쌍따옴표 (우)
  { oldText: '「', newText: "'", priority: 32 },  // 홑낫표
  { oldText: '」', newText: "'", priority: 33 },
  { oldText: '『', newText: "'", priority: 34 },   // 겹낫표
  { oldText: '』', newText: "'", priority: 35 },
  { oldText: '′', newText: "'", priority: 36 },   // 프라임
  { oldText: '″', newText: '"', priority: 37 },   // 더블 프라임
  { oldText: '«', newText: '"', priority: 38 },   // 프랑스식 따옴표
  { oldText: '»', newText: '"', priority: 39 },
  { oldText: '‚', newText: ',', priority: 40 },   // 하단 인용부호 (쉼표형)
  { oldText: '„', newText: '"', priority: 41 },   // 하단 겹 인용부호
  { oldText: '<', newText: "'", priority: 42 },   // 홑화살괄호
  { oldText: '>', newText: "'", priority: 43 },
  { oldText: '`', newText: "'", priority: 44 },   // 백틱

  // 정규식 치환
  { oldText: '\\n+', newText: ' ', priority: 45, isRegex: true },   // 두 줄 이상 줄바꿈
  { oldText: ' {2,}', newText: ' ', priority: 46, isRegex: true },  // 두 개 이상 연속된 공백
  { oldText: '\\.{2,}', newText: '.', priority: 47, isRegex: true }, // 마침표 반복 (.. ... ....)
  { oldText: '!{2,}', newText: '!', priority: 48, isRegex: true },   // 느낌표 반복
  { oldText: '\\?{2,}', newText: '?', priority: 49, isRegex: true }, // 물음표 반복
  { oldText: ',{2,}', newText: ',', priority: 50, isRegex: true },   // 쉼표 반복

  // 비가시 문자 제거
  { oldText: '[\\u00A0]', newText: ' ', priority: 51, isRegex: true },    // NBSP
  { oldText: '[\\u200B\\u200C\\u200D\\uFEFF]', newText: '', priority: 52, isRegex: true }, // ZWSP·ZWNJ·ZWJ·BOM
]
