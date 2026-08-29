/**
 * 엑셀·CSV 열 자동 인식에 쓰는 헤더 별칭표.
 *
 * 학생 식별 열은 두 임포트 경로(명렬표 일괄 등록, 활동 기록 가져오기)가
 * 같은 표를 써야 한다. 따로 두면 별칭을 한쪽에만 추가했을 때 그 경로에서만
 * 열이 인식된다.
 */
export const STUDENT_COL_ALIASES: Record<string, string[]> = {
  grade: ['학년', 'grade'],
  classNum: ['반', 'class', '학급', '반번호', 'classnum', 'class_num'],
  number: ['번호', 'number', 'num', '번', '출석번호'],
  name: ['이름', 'name', '성명', '학생명', '학생이름'],
}

/** 활동 기록 가져오기에서만 쓰는 열. */
export const RECORD_COL_ALIASES: Record<string, string[]> = {
  activityName: ['활동명', '활동 명', '활동', '분류', 'activity', 'activity_name', 'activityname'],
  activityContent: ['활동내용', '활동 내용', '내용', 'content', '기록', '활동기록'],
  studentId: ['학번', 'studentid', 'student_id'],
}
