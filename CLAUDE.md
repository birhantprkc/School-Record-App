# GLOBAL RULES

## ARCHITECTURE
- Component MUST NOT call invoke()
- ALWAYS use Store
- Rust handles ALL DB and core logic

## PRINCIPLES
- Store = single source of truth
- Frontend = UI + state only
- No duplicated logic

## CONVENTIONS
- Rust commands: snake_case
- MUST handle errors explicitly
- Font size: `text-base` minimum. `text-sm` / `text-xs` only for exceptions (table cell preview, badge, caption) with explicit justification.

## DESIGN DECISIONS (의도된 설계, 버그 아님)
- `restore_snapshot_impl`은 복원 전 현재 상태를 `ActivityRecordHistory`에 저장하지 않는다. 복원은 명시적 사용자 액션이므로, 히스토리 자동 저장 없이 스냅샷 시점으로 덮어쓰는 것이 의도된 동작이다.
- `upsert_record_impl`은 셀 편집 시 히스토리를 생성하지 않는다. 과도한 히스토리 누적 방지를 위한 설계다. 히스토리는 치환 적용(`apply_replace_impl`)과 수동 스냅샷 생성 시에만 기록된다.

## PROHIBITED
- Silent failures
- Business logic in frontend

## GIT / COMMIT RULES
- **GPG 서명 필수**: 모든 커밋에 `-S` 플래그 사용. `git commit -S -m "..."`
- **Co-Authored-By / Co-Worked 문구 삽입 금지**: 커밋 메시지에 Claude 관련 문구 일절 포함하지 않는다.
- 커밋 메시지: 한국어 또는 영어, 간결하게 작성.