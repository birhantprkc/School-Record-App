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
- `ExportSection`의 `normalizeContent`(줄바꿈·이중 공백 정리)는 치환 규칙과 무관하게 항상 적용된다. 기본 치환 규칙에 같은 정규식이 있지만, 사용자가 그 규칙을 꺼도 내보내기는 정리해야 하므로 의도적으로 독립된 로직이다. 치환 엔진과 묶지 말 것.
- `backup_project`는 파일을 열 때마다 무조건 백업본을 만든다. 마이그레이션 유무와 무관하게 매 열기마다 백업하는 것이 의도된 정책이며, 앱이 백업 파일을 스캔하거나 자동 삭제하지 않는 것도 의도된 동작이다. 파일명만으로는 그 파일이 이 앱이 만든 것인지 사용자가 보관 중인 것인지 구분할 수 없기 때문이다.

## PROHIBITED
- Silent failures
- Business logic in frontend

## 문서에 낡는 숫자를 적지 않는다
주석·README·매뉴얼·설계 문서에 **세면 바뀌는 수**를 적지 말 것. 테스트 개수,
파일 개수, 줄 수, 단어 수, 커맨드 개수 따위는 다음 커밋에 바로 틀린 값이 되고,
아무도 고치지 않아 결국 문서 전체의 신뢰를 깎는다.

- ❌ `// 엣지 케이스 (32개)` → ✅ `// 엣지 케이스`
- ❌ `테스트 161개 전부 통과` → ✅ `전체 통과` (수는 `cargo test`가 알려준다)
- ❌ `유의어 345개 제공` → ✅ `경쟁 앱 수준 이상` 또는 수를 아예 언급하지 않기

**예외 — 적어도 되는 수:**
- 날짜가 붙은 **이력**. "2026-08-29 기준 통과", 세션 로그(`.claude/SESSIONS.md`),
  "그 세션에 테스트 3개 추가" 같은 과거 사실은 낡지 않는다.
- **고정 상수**: PBKDF2 반복 600,000, nonce 12바이트, `SCHEMA_VERSION`, 스키마 지문.
- **UI 목업 속 예시 숫자**(매뉴얼의 가짜 화면 등) — 실제 값에 대한 주장이 아니다.

판단 기준은 하나다: **"코드가 바뀌면 이 문장이 틀려지는가?"** 그렇다면 수를 빼고
확인 방법을 대신 적는다.

## GIT / COMMIT RULES
- **GPG 서명 필수**: 모든 커밋에 `-S` 플래그 사용. `git commit -S -m "..."`
- **Co-Authored-By / Co-Worked 문구 삽입 금지**: 커밋 메시지에 Claude 관련 문구 일절 포함하지 않는다.
- 커밋 메시지: 한국어 또는 영어, 간결하게 작성.

### PR 머지는 로컬에서 (웹 UI 머지 금지)
GitHub 웹 UI나 `gh pr merge`로 머지하면 **GitHub이 자기 키(web-flow)로 서명**한다.
로컬에 그 공개키가 없으면 `git log --format=%G?`에서 `E`(검증 불가)로 뜨고,
"모든 커밋에 서명" 규칙이 히스토리상 깨진다. 그래서 **로컬에서 머지하고 직접 서명한다.**

```
git fetch origin
git checkout master && git pull --ff-only
git merge --no-ff origin/<PR 브랜치> -m "Merge pull request #N from <브랜치>"
git log --format="%h %G? %s" -1     # G인지 확인
git push origin master
```
- `commit.gpgsign = true`가 설정돼 있어 `git merge`도 자동으로 서명한다(확인 완료).
  명시하고 싶으면 `git merge -S`.
- `--no-ff`로 머지 커밋을 남긴다. PR head가 조상이 되므로 GitHub이 PR을 자동으로
  Merged 처리한다. 별도로 닫지 않아도 된다.
- **`merge.verifySignatures`는 켜지 말 것.** dependabot 커밋은 GitHub 키로 서명돼
  있는데 로컬에 그 공개키가 없으면 켜는 순간 머지가 전부 거부된다.

### GitHub 서명 검증용 공개키 (import 완료)
GitHub이 서명한 커밋이 `%G?`에서 `E`(검증 불가)로 뜨지 않게 하려면 공개키가 필요하다.
```
curl -fsSL https://github.com/web-flow.gpg -o web-flow.gpg
"C:/Program Files/GnuPG/bin/gpg.exe" --import web-flow.gpg
```
- **반드시 `gpg.program`이 가리키는 GnuPG에 넣어야 한다.** Git Bash의 `/usr/bin/gpg`는
  키링이 따로라 거기 넣으면 git이 못 찾는다(실제로 한 번 헛짚었다).
- 파일에 키가 둘 들어 있다. `4AEE18F83AFDEB23`은 **2024-01-16 만료된 구 키**이고,
  현재 서명에 쓰이는 것은 `B5690EEEBB952194`다.
- import 후에도 `%G?`는 `U`(유효하나 신뢰도 미지정)다. `G`로 만들려면 해당 키에
  ownertrust를 부여해야 하는데, 이는 "이 키가 GitHub의 것"이라는 신뢰 선언이므로
  선택 사항이다. 검증 자체는 `U`로도 이미 되고 있다.