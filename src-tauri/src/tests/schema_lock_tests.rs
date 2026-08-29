//! 스키마 고정(lock) 테스트.
//!
//! 목적: schema.sql이 바뀌었는데 `SCHEMA_VERSION`을 올리지 않은 채 배포되는 사고를 막는다.
//! 정식 출시 이후에는 사용자 PC에 이미 특정 구조의 DB 파일이 존재하므로,
//! 스키마 변경에는 반드시 버전 bump + 마이그레이션이 따라야 한다.
//!
//! ## 스키마를 변경할 때의 절차 (예: v1 → v2)
//! 1. `schema.sql`을 수정한다.
//! 2. `tests/schema_history/v1.sql`은 **그대로 둔다**(배포된 구조의 기록).
//!    수정한 `schema.sql`을 `tests/schema_history/v2.sql`로 복사한다.
//! 3. `db.rs`의 `SCHEMA_VERSION`을 2로 올리고, `MIGRATIONS`에 v1→v2 SQL을 추가한다.
//! 4. `SCHEMA_BASELINES`에 `v2.sql`, `SCHEMA_FINGERPRINTS`에 새 지문을 추가한다.
//!    (지문 값은 이 테스트 실패 메시지에 실제 값이 출력된다)
//! 5. `tauri.conf.json`의 앱 버전도 함께 올린다 (릴리즈 노트 모달 표시 조건).
//!
//! 1~4 중 하나라도 빠지면 이 모듈의 테스트가 실패한다.

use crate::db;
use rusqlite::Connection;
use sha2::{Digest, Sha256};

/// 버전별 스키마 지문(정규화된 sqlite_master 덤프의 SHA-256).
/// 인덱스 i = 스키마 버전 i+1.
///
/// ⚠️ 기존 항목은 절대 수정 금지. 이미 배포된 DB 파일의 구조를 기록한 값이다.
/// 스키마가 바뀌었다면 기존 값을 고치는 것이 아니라 새 항목을 추가해야 한다.
pub(crate) const SCHEMA_FINGERPRINTS: &[&str] = &[
    // v1 — 정식 출시 스키마
    "fc7c11a3d03c8d4a104f2ec9788745928bcb11cef517bab90b0be463dedb6be2",
];

/// 버전별 스키마 원본. 인덱스 i = 스키마 버전 i+1.
///
/// ⚠️ 기존 파일은 절대 수정 금지 (`SCHEMA_FINGERPRINTS`와 같은 이유).
const SCHEMA_BASELINES: &[&str] = &[
    include_str!("schema_history/v1.sql"), // v1
];

// ── 헬퍼 ─────────────────────────────────────────────────────

/// sqlite_master를 정규화해 덤프한다.
/// 공백은 단일 스페이스로 축약하므로 들여쓰기·줄바꿈만 바뀐 경우는 동일하게 취급된다.
fn schema_dump(conn: &Connection) -> String {
    let mut stmt = conn
        .prepare(
            "SELECT type, name, COALESCE(sql, '') FROM sqlite_master
             WHERE name NOT LIKE 'sqlite\\_%' ESCAPE '\\'
             ORDER BY type, name",
        )
        .unwrap();

    let rows = stmt
        .query_map([], |r| {
            let obj_type: String = r.get(0)?;
            let name: String = r.get(1)?;
            let sql: String = r.get(2)?;
            let normalized = sql.split_whitespace().collect::<Vec<_>>().join(" ");
            Ok(format!("{obj_type}|{name}|{normalized}"))
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect::<Vec<_>>();

    rows.join("\n")
}

fn fingerprint(conn: &Connection) -> String {
    let mut hasher = Sha256::new();
    hasher.update(schema_dump(conn).as_bytes());
    // sha2 0.11의 finalize()는 LowerHex를 구현하지 않는 Array를 반환하므로 직접 hex 변환한다.
    // 바이트당 소문자 2자리 = 이전 `{:x}` 출력과 동일한 문자열이어야 한다.
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// 현재 schema.sql로 만든 DB (= 신규 프로젝트 생성 결과)
fn fresh_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(include_str!("../schema.sql")).unwrap();
    conn
}

/// 지정 버전 시점의 스키마로 만든 DB
fn baseline_db(version: u32) -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(SCHEMA_BASELINES[(version - 1) as usize])
        .unwrap();
    conn.pragma_update(None, "user_version", version).unwrap();
    conn
}

// ── 테스트 ───────────────────────────────────────────────────

#[test]
fn test_fresh_schema_matches_locked_fingerprint() {
    let conn = fresh_db();
    let actual = fingerprint(&conn);
    let expected = *SCHEMA_FINGERPRINTS
        .get((db::SCHEMA_VERSION - 1) as usize)
        .unwrap_or_else(|| {
            panic!(
                "SCHEMA_VERSION={}에 해당하는 지문이 SCHEMA_FINGERPRINTS에 없습니다. \
                 새 버전의 지문 {}을(를) 추가하세요.",
                db::SCHEMA_VERSION, actual
            )
        });

    assert_eq!(
        actual,
        expected,
        "\n\n\
         ===== 스키마가 변경되었습니다 (v{ver} 지문 불일치) =====\n\
         schema.sql이 스키마 버전 {ver}로 고정된 구조와 다릅니다.\n\
         버전을 올리지 않고 배포하면 기존 사용자의 DB 파일이 마이그레이션되지 않습니다.\n\n\
         schema_lock_tests.rs 상단의 '스키마를 변경할 때의 절차'를 따르세요.\n\
         새 버전의 지문: {actual}\n\n\
         현재 schema.sql 덤프:\n{dump}\n",
        ver = db::SCHEMA_VERSION,
        actual = actual,
        dump = schema_dump(&conn),
    );
}

#[test]
fn test_baseline_snapshots_match_locked_fingerprints() {
    // 과거 버전 스냅샷 파일이 몰래 수정되지 않았는지 확인한다.
    for (i, expected) in SCHEMA_FINGERPRINTS.iter().enumerate() {
        let version = (i + 1) as u32;
        let conn = baseline_db(version);
        assert_eq!(
            &fingerprint(&conn),
            expected,
            "schema_history/v{version}.sql이 수정되었습니다. \
             배포된 구조의 기록이므로 되돌려야 합니다."
        );
    }
}

#[test]
fn test_migration_path_matches_fresh_install() {
    // 모든 과거 버전 DB가 마이그레이션 후 신규 설치와 동일한 구조가 되어야 한다.
    let target = fingerprint(&fresh_db());

    for i in 0..SCHEMA_BASELINES.len() {
        let version = (i + 1) as u32;
        let mut conn = baseline_db(version);
        db::migrate(&mut conn, version).unwrap();

        assert_eq!(
            fingerprint(&conn),
            target,
            "\n\nv{version} DB를 마이그레이션한 결과가 신규 설치 스키마와 다릅니다.\n\
             MIGRATIONS[{i}..]가 schema.sql의 변경 내용을 모두 반영하는지 확인하세요.\n\n\
             마이그레이션 결과:\n{migrated}\n\n신규 설치:\n{fresh}\n",
            migrated = schema_dump(&conn),
            fresh = schema_dump(&fresh_db()),
        );

        let user_version: u32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(user_version, db::SCHEMA_VERSION);
    }
}

#[test]
fn test_migrations_cover_every_schema_version() {
    assert_eq!(
        db::MIGRATIONS.len(),
        db::SCHEMA_VERSION as usize,
        "SCHEMA_VERSION={}이면 MIGRATIONS는 {}개여야 합니다 (v0→v1 … v{}→v{}).",
        db::SCHEMA_VERSION,
        db::SCHEMA_VERSION,
        db::SCHEMA_VERSION - 1,
        db::SCHEMA_VERSION,
    );
}

#[test]
fn test_lock_tables_cover_every_schema_version() {
    assert_eq!(
        SCHEMA_FINGERPRINTS.len(),
        db::SCHEMA_VERSION as usize,
        "SCHEMA_VERSION을 올렸다면 SCHEMA_FINGERPRINTS에도 항목을 추가해야 합니다."
    );
    assert_eq!(
        SCHEMA_BASELINES.len(),
        db::SCHEMA_VERSION as usize,
        "SCHEMA_VERSION을 올렸다면 schema_history/vN.sql을 추가하고 \
         SCHEMA_BASELINES에 등록해야 합니다."
    );
}
