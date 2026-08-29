import { describe, it, expect } from 'vitest'
import { isValidIdentityPart, parseStudentId } from './studentId'

describe('isValidIdentityPart', () => {
  it('1 이상의 정수만 허용', () => {
    expect(isValidIdentityPart(1)).toBe(true)
    expect(isValidIdentityPart(35)).toBe(true)
  })

  it('0과 음수는 거부 — DB CHECK 제약에 걸린다', () => {
    expect(isValidIdentityPart(0)).toBe(false)
    expect(isValidIdentityPart(-1)).toBe(false)
  })

  it('소수는 거부 — 백엔드에서 i64로 역직렬화되지 않는다', () => {
    expect(isValidIdentityPart(1.5)).toBe(false)
  })

  it('숫자가 아닌 값은 거부', () => {
    expect(isValidIdentityPart('1')).toBe(false)
    expect(isValidIdentityPart(null)).toBe(false)
    expect(isValidIdentityPart(undefined)).toBe(false)
    expect(isValidIdentityPart(NaN)).toBe(false)
  })
})

describe('parseStudentId', () => {
  it('4자리 ABCC', () => {
    expect(parseStudentId('1234')).toEqual({grade: 1, classNum: 2, number: 34})
  })

  it('5자리 ABBCC', () => {
    expect(parseStudentId('11203')).toEqual({grade: 1, classNum: 12, number: 3})
  })

  it('6자리 ABBCCC', () => {
    expect(parseStudentId('112003')).toEqual({grade: 1, classNum: 12, number: 3})
  })

  it('숫자가 아닌 문자는 버리고 자릿수로 판단', () => {
    expect(parseStudentId('1-2-34')).toEqual({grade: 1, classNum: 2, number: 34})
    expect(parseStudentId(' 1234 ')).toEqual({grade: 1, classNum: 2, number: 34})
  })

  it('숫자로 들어와도 처리', () => {
    expect(parseStudentId(1234)).toEqual({grade: 1, classNum: 2, number: 34})
  })

  it('지원하지 않는 자릿수는 null', () => {
    expect(parseStudentId('123')).toBeNull()
    expect(parseStudentId('1234567')).toBeNull()
    expect(parseStudentId('')).toBeNull()
    expect(parseStudentId(null)).toBeNull()
    expect(parseStudentId('없음')).toBeNull()
  })

  it("'0101'은 파싱은 되지만 학년 0을 만든다 — 값 판정은 별도", () => {
    // 자릿수만 보므로 여기서는 통과한다. 임포트에서 빠지는 이유는
    // isValidIdentityPart이 0을 거부하기 때문이다.
    const p = parseStudentId('0101')
    expect(p).toEqual({grade: 0, classNum: 1, number: 1})
    expect(isValidIdentityPart(p!.grade)).toBe(false)
  })
})
