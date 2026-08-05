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

## DB SCHEMA RULES (정식 출시 이후 적용)
- 이미 배포된 앱이므로 사용자 PC에 기존 구조의 DB 파일이 존재한다. **`schema.sql` 변경 시 아래를 모두 수행한다.**
  1. `db.rs`의 `SCHEMA_VERSION`을 올린다.
  2. `db.rs`의 `MIGRATIONS`에 이전 버전 → 새 버전 SQL을 추가한다.
  3. 수정한 `schema.sql`을 `src-tauri/src/tests/schema_history/vN.sql`로 복사한다.
  4. `schema_lock_tests.rs`의 `SCHEMA_FINGERPRINTS` / `SCHEMA_BASELINES`에 항목을 추가한다.
  5. `tauri.conf.json`의 앱 버전도 함께 올린다 (릴리즈 노트 모달 표시 조건).
- **`schema_history/vN.sql`과 기존 지문 값은 절대 수정 금지.** 배포된 DB 구조의 기록이다.
- 위를 빠뜨리면 `schema_lock_tests.rs`가 실패한다. 테스트를 맞추려고 지문만 고치는 것은 금지.

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