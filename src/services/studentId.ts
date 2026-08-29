/**
 * 학생 식별 정보 파싱.
 *
 * 임포트에서 어떤 행을 받아들이고 어떤 행을 버릴지 결정하는 규칙이라
 * 컴포넌트가 아니라 여기에 둔다. 잘못 판정하면 학생 기록이 통째로 빠지거나
 * 엉뚱한 학생에게 붙는다.
 */

export interface StudentIdParts {
  grade: number
  classNum: number
  number: number
}

/**
 * 학년·반·번호로 쓸 수 있는 값인지 판정한다.
 *
 * 1 이상의 정수만 허용한다. 음수는 DB의 CHECK 제약에 걸리고, 소수는 백엔드에서
 * i64로 역직렬화되지 않아 둘 다 영문 오류가 그대로 노출된다. 0은 학번 '0101'
 * 같은 입력에서 나온다.
 */
export function isValidIdentityPart(v: unknown): boolean {
  return Number.isInteger(v) && (v as number) >= 1
}

/**
 * 학번 문자열을 학년·반·번호로 나눈다.
 *
 * 숫자가 아닌 문자는 버리고 자릿수로 형식을 판단한다.
 *   4자리 ABCC  → 1학년 1반 01번
 *   5자리 ABBCC → 1학년 01반 01번
 *   6자리 ABBCCC→ 1학년 01반 001번
 * 그 외 자릿수는 형식을 알 수 없으므로 null.
 *
 * 자릿수만 보므로 '0101' 같은 값은 학년 0을 만든다. 값의 타당성은
 * isValidIdentityPart이 따로 판정한다.
 */
export function parseStudentId(val: unknown): StudentIdParts | null {
  const s = String(val ?? '').trim().replace(/\D/g, '')
  if (s.length === 4) return {grade: +s[0], classNum: +s[1], number: +s.slice(2, 4)}
  if (s.length === 5) return {grade: +s[0], classNum: +s.slice(1, 3), number: +s.slice(3, 5)}
  if (s.length === 6) return {grade: +s[0], classNum: +s.slice(1, 3), number: +s.slice(3, 6)}
  return null
}
