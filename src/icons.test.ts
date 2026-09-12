import {describe, expect, it} from 'vitest'
import {readdirSync, readFileSync} from 'node:fs'
import {dirname, join} from 'node:path'
import {fileURLToPath} from 'node:url'
import * as icons from '@lucide/vue'

/**
 * 앱이 쓰는 아이콘이 실제로 아이콘 패키지에 있는지 전수 확인한다.
 *
 * **없는 이름을 import해도 빌드는 통과한다.** Vue는 등록되지 않은 컴포넌트를
 * 렌더 시점에 경고만 내고 넘어가므로, 그 화면을 직접 열어보기 전까지 아무도
 * 모른다. 아이콘 51종이 28개 파일에 흩어져 있어 눈으로 훑는 것은 방법이 못 된다.
 *
 * 패키지를 바꿀 때 특히 중요하다. lucide는 이름을 바꾸거나(AlertCircle →
 * CircleAlert) 통째로 뺀 이력이 있다(브랜드 아이콘). 이 테스트가 없으면
 * 마이그레이션이 "빌드 통과"만 보고 끝난다.
 */

/** 앱이 쓰는 아이콘 패키지. 옮길 때는 위 import와 이 상수를 함께 고친다. */
const PACKAGE = '@lucide/vue'

/** 스캔이 망가져 0개를 찾고도 통과하는 일을 막는 하한. 실제 수보다 넉넉히 낮게 둔다. */
const MIN_EXPECTED_ICONS = 20

// 수동 URL 파싱은 Windows(C:/...)와 Linux(CI)에서 갈린다. 표준 변환을 쓴다.
const SOURCE_DIR = dirname(fileURLToPath(import.meta.url))

function sourceFiles(dir: string): string[] {
  return readdirSync(dir, {withFileTypes: true}).flatMap((entry) => {
    const path = join(dir, entry.name)
    if (entry.isDirectory()) return sourceFiles(path)
    if (!/\.(vue|ts|js)$/.test(entry.name)) return []
    if (/\.(test|spec)\./.test(entry.name)) return []
    return [path]
  })
}

interface IconImport {
  file: string
  source: string
  names: string[]
}

function lucideImports(): IconImport[] {
  const pattern = /import\s*\{([^}]*)\}\s*from\s*['"]([^'"]*lucide[^'"]*)['"]/g
  const found: IconImport[] = []

  for (const file of sourceFiles(SOURCE_DIR)) {
    const text = readFileSync(file, 'utf-8')
    for (const match of text.matchAll(pattern)) {
      const names = match[1]
          .split(',')
          .map((n) => n.trim().split(/\s+as\s+/)[0].trim())
          .filter(Boolean)
      found.push({file, source: match[2], names})
    }
  }
  return found
}

describe('아이콘', () => {
  const imports = lucideImports()
  const allNames = [...new Set(imports.flatMap((i) => i.names))].sort()

  it('스캔이 실제로 아이콘을 찾는다', () => {
    // 이 단언이 없으면 정규식이 깨졌을 때 아래 두 테스트가 빈 목록을 돌며
    // 조용히 통과한다 — 검사하지 않는 테스트가 초록으로 남는다.
    expect(imports.length).toBeGreaterThan(0)
    expect(allNames.length).toBeGreaterThanOrEqual(MIN_EXPECTED_ICONS)
  })

  it(`모두 ${PACKAGE}에서 가져온다`, () => {
    // 패키지를 옮기다 만 상태를 잡는다. 일부 파일만 새 패키지를 보면 두 벌이
    // 번들에 들어가고, 옛 패키지가 사라지는 순간 그 파일들이 깨진다.
    const strays = imports
        .filter((i) => i.source !== PACKAGE)
        .map((i) => `${i.file} → ${i.source}`)
    expect(strays).toEqual([])
  })

  it('앱이 쓰는 아이콘이 패키지에 모두 있다', () => {
    const exported = new Set(Object.keys(icons))
    const missing = allNames.filter((name) => !exported.has(name))
    expect(missing, `${PACKAGE}에 없는 아이콘: ${missing.join(', ')}`).toEqual([])
  })
})
