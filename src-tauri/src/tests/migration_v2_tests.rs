//! v1 → v2 마이그레이션 테스트.
//!
//! v2가 바꾸는 것은 DDL이 아니라 **데이터의 표현**이다 —
//! `ActivityRecordHistory.note`와 `Snapshot.memo`가 암호화 대상이 됐다.
//! 그래서 지문 테스트(schema_lock_tests)가 잡아주는 것이 없고, 여기서 본다.
//!
//! 이 모듈이 지키려는 불변식은 셋이다.
//!   1. 버전 승격과 데이터 변환은 한 커밋이다 (둘 중 하나만 남은 파일은 복구가 안 된다).
//!   2. 이미 암호문인 컬럼은 다시 암호화하지 않는다 (이중 암호화 = 복구 불능).
//!   3. 마이그레이션 이후 새로 쓰는 note/memo도 암호문이다 (평문이 섞이면
//!      decrypt_all_data가 실패해 암호화 해제가 영구히 막힌다).

use super::{insert_activity, insert_area, insert_record, insert_student, setup_temp_db_path_state,
            setup_test_db};
use crate::commands::crypto::{
    change_encryption_password_impl, disable_encryption_impl, enable_encryption_impl,
    encrypt_columns_introduced_in, is_purge_pending, resolve_data_key,
};
use crate::commands::project::{backup_project_impl, migrate_schema_impl};
use crate::commands::record::{get_record_history_impl, save_snapshot_internal};
use crate::commands::snapshot::{create_snapshot_impl, get_snapshots_impl};
use crate::crypto::decrypt;
use crate::db;
use crate::state::{CryptoState, CryptoStateHandle, DbPathState, DbState};
use rusqlite::Connection;
use std::sync::Mutex;

// ── 헬퍼 ─────────────────────────────────────────────────────

fn crypto_state(key: Option<[u8; 32]>) -> CryptoStateHandle {
    Mutex::new(CryptoState { key })
}

/// 원문(raw) 그대로 읽는다. 암호문인지 평문인지 판정하려면 복호화를 거치면 안 된다.
fn raw_col(conn: &Connection, table: &str, column: &str) -> Vec<(i64, Option<String>)> {
    let sql = format!("SELECT id, {column} FROM {table} ORDER BY id");
    let mut stmt = conn.prepare(&sql).unwrap();
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    rows
}

/// 읽기 경로를 거쳐 나온 note들. 화면에 보이는 값과 같다.
fn plain_notes(conn: &Connection, act: i64, stu: i64, key: Option<[u8; 32]>) -> Vec<String> {
    get_record_history_impl(conn, act, stu, 100, 0, key)
        .unwrap()
        .iter()
        .filter_map(|h| h.note.clone())
        .collect()
}

fn user_version(conn: &Connection) -> u32 {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap()
}

/// 기록 하나 + 히스토리 두 줄(note 있는 것/NULL) + 스냅샷 두 개(memo 있는 것/NULL).
/// 전부 평문 상태다.
fn seed_plaintext(conn: &Connection) -> (i64, i64) {
    let stu = insert_student(conn, 1, 1, 1, "홍길동");
    let area = insert_area(conn, "자율활동", 500);
    let act = insert_activity(conn, "학급회의");
    conn.execute(
        "INSERT INTO AreaActivity (area_id, activity_id) VALUES (?1, ?2)",
        rusqlite::params![area, act],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO AreaStudent (area_id, student_id) VALUES (?1, ?2)",
        rusqlite::params![area, stu],
    )
    .unwrap();
    insert_record(conn, act, stu, "학급 회의를 주도함");

    // note가 있는 줄 / NULL인 줄을 섞는다. NULL에서 터지지 않아야 한다.
    save_snapshot_internal(conn, act, stu, Some("첫 메모"), None).unwrap();
    conn.execute(
        "INSERT INTO ActivityRecordHistory (activity_record_id, content, note)
         SELECT id, content, NULL FROM ActivityRecord WHERE activity_id=?1 AND student_id=?2",
        rusqlite::params![act, stu],
    )
    .unwrap();

    create_snapshot_impl(conn, Some("스냅샷 메모".to_string()), None).unwrap();
    create_snapshot_impl(conn, None, None).unwrap();

    (act, stu)
}

/// "v1 시점에 암호화가 켜져 있던 파일"을 만든다.
///
/// 현재 코드의 `enable_encryption_impl`은 note/memo까지 암호화하므로(v2 동작),
/// 그 둘만 평문으로 되돌리고 user_version을 1로 내려 v1 파일을 재현한다.
fn make_v1_encrypted(
    conn: &Connection,
    crypto: &CryptoStateHandle,
    path_state: &DbPathState,
    password: &str,
) -> [u8; 32] {
    enable_encryption_impl(conn, crypto, path_state, password).unwrap();
    let key = resolve_data_key(conn, crypto).unwrap().unwrap();

    for (table, column) in [("ActivityRecordHistory", "note"), ("Snapshot", "memo")] {
        for (id, value) in raw_col(conn, table, column) {
            let Some(value) = value else { continue };
            let plain = decrypt(&value, &key).unwrap();
            conn.execute(
                &format!("UPDATE {table} SET {column}=?1 WHERE id=?2"),
                rusqlite::params![plain, id],
            )
            .unwrap();
        }
    }
    conn.pragma_update(None, "user_version", 1u32).unwrap();
    key
}

// ── 시나리오: v1 + 암호화 켜짐 ───────────────────────────────

#[test]
fn test_v1_encrypted_migrates_note_and_memo() {
    let conn = setup_test_db();
    let (path_state, dir) = setup_temp_db_path_state();
    let crypto = crypto_state(None);
    let (act, stu) = seed_plaintext(&conn);
    let key = make_v1_encrypted(&conn, &crypto, &path_state, "password");

    // since_version=1 컬럼은 마이그레이션이 건드리면 안 된다(이중 암호화 방지).
    // 그 증거는 "바이트가 그대로인가"뿐이다.
    let before_names = raw_col(&conn, "Student", "name");
    let before_contents = raw_col(&conn, "ActivityRecord", "content");
    let before_hist_contents = raw_col(&conn, "ActivityRecordHistory", "content");

    let mut conn = conn;
    migrate_schema_impl(&mut conn, &crypto, &path_state).unwrap();

    assert_eq!(user_version(&conn), db::SCHEMA_VERSION);
    assert_eq!(raw_col(&conn, "Student", "name"), before_names);
    assert_eq!(raw_col(&conn, "ActivityRecord", "content"), before_contents);
    assert_eq!(
        raw_col(&conn, "ActivityRecordHistory", "content"),
        before_hist_contents,
        "since_version=1 컬럼을 다시 암호화하면 이중 암호화가 된다"
    );

    // note/memo는 암호문이 됐고, 그 키로 복호화하면 원래 값이 나온다.
    for (table, column, expected) in [
        ("ActivityRecordHistory", "note", "첫 메모"),
        ("Snapshot", "memo", "스냅샷 메모"),
    ] {
        let values: Vec<String> = raw_col(&conn, table, column)
            .into_iter()
            .filter_map(|(_, v)| v)
            .collect();
        assert_eq!(values.len(), 1, "{table}.{column}: NULL 줄은 그대로여야 한다");
        assert_ne!(values[0], expected, "{table}.{column}이 평문으로 남아 있다");
        assert_eq!(decrypt(&values[0], &key).unwrap(), expected);
    }

    // 읽기 경로는 평문을 돌려준다.
    let history = get_record_history_impl(&conn, act, stu, 10, 0, Some(key)).unwrap();
    let notes: Vec<_> = history.iter().filter_map(|h| h.note.clone()).collect();
    assert_eq!(notes, vec!["첫 메모".to_string()]);
    let snaps = get_snapshots_impl(&conn, Some(key)).unwrap();
    let memos: Vec<_> = snaps.iter().filter_map(|s| s.memo.clone()).collect();
    assert_eq!(memos, vec!["스냅샷 메모".to_string()]);

    // 마이그레이션은 정리 표시를 남기고, 성공하면 곧바로 정리하고 표시를 지운다.
    // 표시가 남아 있다면 정리가 끝나지 않은 것이고, 설정 화면에 경고가 뜬다.
    assert!(!is_purge_pending(&conn).unwrap());

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_v1_plaintext_file_only_bumps_version() {
    let conn = setup_test_db();
    let (path_state, dir) = setup_temp_db_path_state();
    conn.pragma_update(None, "user_version", 1u32).unwrap();
    seed_plaintext(&conn);
    let before_notes = raw_col(&conn, "ActivityRecordHistory", "note");
    let before_memos = raw_col(&conn, "Snapshot", "memo");

    let mut conn = conn;
    migrate_schema_impl(&mut conn, &crypto_state(None), &path_state).unwrap();

    assert_eq!(user_version(&conn), db::SCHEMA_VERSION);
    assert_eq!(raw_col(&conn, "ActivityRecordHistory", "note"), before_notes);
    assert_eq!(raw_col(&conn, "Snapshot", "memo"), before_memos);
    assert!(
        !is_purge_pending(&conn).unwrap(),
        "바꾼 행이 없으면 정리 표시도 남기지 않는다"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_v2_file_is_untouched() {
    let conn = setup_test_db();
    let (path_state, dir) = setup_temp_db_path_state();
    let crypto = crypto_state(None);
    seed_plaintext(&conn);
    enable_encryption_impl(&conn, &crypto, &path_state, "password").unwrap();

    let before_notes = raw_col(&conn, "ActivityRecordHistory", "note");
    let before_memos = raw_col(&conn, "Snapshot", "memo");

    let mut conn = conn;
    migrate_schema_impl(&mut conn, &crypto, &path_state).unwrap();

    // migrate_schema_impl은 from >= SCHEMA_VERSION에서 곧바로 반환하므로, 위 호출만으로는
    // 변환 로직이 아예 실행되지 않는다. 그것이 no-op이라는 것은 증명하지 못한다.
    // 변환 함수를 직접 돌려 "이미 v2인 데이터에 돌려도 바뀌지 않는다"를 본다.
    let key = resolve_data_key(&conn, &crypto).unwrap();
    assert_eq!(
        encrypt_columns_introduced_in(&conn, key, 2).unwrap(),
        0,
        "이미 암호문이면 바꿀 행이 없어야 한다"
    );

    // 재암호화됐다면 nonce가 달라져 바이트가 바뀐다.
    assert_eq!(raw_col(&conn, "ActivityRecordHistory", "note"), before_notes);
    assert_eq!(raw_col(&conn, "Snapshot", "memo"), before_memos);
    let _ = std::fs::remove_dir_all(dir);
}

// ── 원자성 ───────────────────────────────────────────────────

#[test]
fn test_locked_file_refuses_to_migrate() {
    let conn = setup_test_db();
    let (path_state, dir) = setup_temp_db_path_state();
    let crypto = crypto_state(None);
    seed_plaintext(&conn);
    make_v1_encrypted(&conn, &crypto, &path_state, "password");

    // 잠금 상태 재현 — 키를 잊은 CryptoState로 마이그레이션을 시도한다.
    let locked = crypto_state(None);
    let before_notes = raw_col(&conn, "ActivityRecordHistory", "note");

    let mut conn = conn;
    let err = migrate_schema_impl(&mut conn, &locked, &path_state).unwrap_err();

    assert!(err.contains("잠금"), "잠금 상태임을 알려야 한다: {err}");
    assert_eq!(
        user_version(&conn),
        1,
        "키가 없으면 버전을 올리면 안 된다 — v2인데 note가 평문인 파일은 복구가 안 된다"
    );
    assert_eq!(raw_col(&conn, "ActivityRecordHistory", "note"), before_notes);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_failure_after_version_bump_rolls_everything_back() {
    // user_version 승격이 정말 트랜잭션에 포함되는지 본다.
    //
    // 트리거로 데이터 변환 자체를 실패시키면 pragma_update 이전에 멈추므로
    // "롤백됐다"가 아니라 "애초에 쓰이지 않았다"만 확인된다. 그래서 훅은 성공시키고,
    // pragma_update **이후**에 도는 foreign_key_check에서 걸리게 만든다.
    let conn = setup_test_db();
    conn.pragma_update(None, "user_version", 1u32).unwrap();
    seed_plaintext(&conn);
    let before_notes = raw_col(&conn, "ActivityRecordHistory", "note");

    let mut conn = conn;
    let err = db::migrate(&mut conn, 1, &|tx, _| {
        // foreign_keys = OFF 상태라 부모 없는 행도 들어간다.
        tx.execute(
            "INSERT INTO ActivityRecordHistory (activity_record_id, content, note)
             VALUES (999999, 'orphan', '고아 메모')",
            [],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
    .unwrap_err();

    assert!(err.contains("외래키"), "무결성 위반을 알려야 한다: {err}");
    assert_eq!(user_version(&conn), 1, "user_version이 롤백되어야 한다");
    assert_eq!(
        raw_col(&conn, "ActivityRecordHistory", "note"),
        before_notes,
        "같은 트랜잭션의 데이터 변경도 함께 롤백되어야 한다"
    );

    let fk_on: i64 = conn
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .unwrap();
    assert_eq!(fk_on, 1, "실패해도 foreign_keys는 ON으로 복구되어야 한다");
}

#[test]
fn test_migration_is_idempotent_when_already_ciphertext() {
    // 마이그레이션이 실패해 v1에 머무른 파일은 닫히지 않는다. 그 사이에 새 표현으로
    // 쓰인 행이 다음 열기에서 한 번 더 변환되면 이중 암호화가 된다.
    let conn = setup_test_db();
    let (path_state, dir) = setup_temp_db_path_state();
    let crypto = crypto_state(None);
    seed_plaintext(&conn);
    let key = make_v1_encrypted(&conn, &crypto, &path_state, "password");

    // v1 파일에 이미 암호문인 메모가 섞인 상태를 만든다.
    let already = crate::crypto::encrypt("이미 암호문", &key).unwrap();
    conn.execute(
        "INSERT INTO Snapshot (memo) VALUES (?1)",
        rusqlite::params![already.clone()],
    )
    .unwrap();

    let mut conn = conn;
    migrate_schema_impl(&mut conn, &crypto, &path_state).unwrap();

    let memos: Vec<String> = raw_col(&conn, "Snapshot", "memo")
        .into_iter()
        .filter_map(|(_, v)| v)
        .collect();
    assert!(
        memos.contains(&already),
        "이미 이 키로 암호화된 값은 그대로 두어야 한다"
    );
    // 한 번의 복호화로 평문이 나와야 한다. 두 번 암호화됐다면 암호문이 나온다.
    for m in &memos {
        let plain = decrypt(m, &key).unwrap();
        assert!(!plain.contains(':'), "이중 암호화된 값이 있다: {plain}");
    }
    let _ = std::fs::remove_dir_all(dir);
}

// ── 마이그레이션 이후의 쓰기 경로 (회귀 방지) ────────────────

/// 이 테스트가 없으면 "암호화를 영영 끌 수 없는 파일"이 그대로 나간다.
///
/// `ENCRYPTED_COLUMNS`에 컬럼만 추가하고 개별 행 쓰기 경로를 고치지 않으면,
/// 마이그레이션 이후 새로 쓰는 note/memo가 평문으로 저장된다. 그러면 한 컬럼에
/// 평문과 암호문이 섞이고, `decrypt_all_data`가 평문 행에서 실패해
/// 암호화 해제와 비밀번호 변경이 **영구히** 막힌다.
///
/// disable_encryption이 성공한다는 것은 곧 ENCRYPTED_COLUMNS의 모든 컬럼이
/// 빠짐없이 암호문이라는 뜻이기도 하다 — decrypt_all_data가 전부를 훑기 때문이다.
#[test]
fn test_writes_after_migration_do_not_block_disable() {
    let conn = setup_test_db();
    let (path_state, dir) = setup_temp_db_path_state();
    let crypto = crypto_state(None);
    let (act, stu) = seed_plaintext(&conn);
    make_v1_encrypted(&conn, &crypto, &path_state, "password");

    let mut conn = conn;
    migrate_schema_impl(&mut conn, &crypto, &path_state).unwrap();
    let conn = conn;
    let key = resolve_data_key(&conn, &crypto).unwrap();

    // 마이그레이션 이후 새로 쓰는 메모들 — 화면에서 오는 모든 경로를 흉내낸다.
    crate::commands::record::upsert_record_impl(&conn, act, stu, "내용을 고침", key).unwrap();
    save_snapshot_internal(&conn, act, stu, Some("마이그레이션 뒤 메모"), key).unwrap();
    create_snapshot_impl(&conn, Some("마이그레이션 뒤 스냅샷".to_string()), key).unwrap();

    // 여기가 핵심 — 평문이 하나라도 섞였다면 복호화가 실패해 Err가 된다.
    disable_encryption_impl(&conn, &crypto, &path_state).unwrap();

    let notes: Vec<String> = raw_col(&conn, "ActivityRecordHistory", "note")
        .into_iter()
        .filter_map(|(_, v)| v)
        .collect();
    assert!(notes.contains(&"마이그레이션 뒤 메모".to_string()));
    assert!(notes.contains(&"첫 메모".to_string()));
    let memos: Vec<String> = raw_col(&conn, "Snapshot", "memo")
        .into_iter()
        .filter_map(|(_, v)| v)
        .collect();
    assert!(memos.contains(&"마이그레이션 뒤 스냅샷".to_string()));
    assert!(memos.contains(&"스냅샷 메모".to_string()));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_change_password_roundtrip_keeps_note_and_memo() {
    let conn = setup_test_db();
    let (path_state, dir) = setup_temp_db_path_state();
    let crypto = crypto_state(None);
    let (act, stu) = seed_plaintext(&conn);
    make_v1_encrypted(&conn, &crypto, &path_state, "old_password");

    let mut conn = conn;
    migrate_schema_impl(&mut conn, &crypto, &path_state).unwrap();
    let conn = conn;

    change_encryption_password_impl(&conn, &crypto, &path_state, "old_password", "new_password")
        .unwrap();

    let new_key = resolve_data_key(&conn, &crypto).unwrap();
    let history = get_record_history_impl(&conn, act, stu, 10, 0, new_key).unwrap();
    assert!(history.iter().any(|h| h.note.as_deref() == Some("첫 메모")));
    let snaps = get_snapshots_impl(&conn, new_key).unwrap();
    assert!(snaps.iter().any(|s| s.memo.as_deref() == Some("스냅샷 메모")));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_import_and_replace_notes_are_encrypted() {
    // 앱이 자동으로 붙이는 note("import", "치환 적용 전" 등)도 암호화 대상이다.
    // 하나라도 평문으로 남으면 위 테스트가 말하는 상태가 된다.
    let conn = setup_test_db();
    let (path_state, dir) = setup_temp_db_path_state();
    let crypto = crypto_state(None);
    let (act, stu) = seed_plaintext(&conn);
    enable_encryption_impl(&conn, &crypto, &path_state, "password").unwrap();
    let key = resolve_data_key(&conn, &crypto).unwrap();

    crate::commands::record::bulk_import_records_impl(
        &conn,
        &[crate::types::ImportRecordInput {
            grade: 1,
            class_num: 1,
            number: 1,
            name: Some("홍길동".to_string()),
            activity_id: act,
            content: "엑셀에서 가져온 내용".to_string(),
        }],
        key,
    )
    .unwrap();

    // 'import'는 SQL 리터럴이었다. 바인딩으로 바꾸지 않으면 여기서 평문으로 남는다.
    // 빠른 교체가 같은 히스토리 행의 note를 덮어쓰므로(중복 제거 설계) 지금 확인한다.
    let notes = plain_notes(&conn, act, stu, key);
    assert!(notes.iter().any(|n| n == "import"), "{notes:?}");
    assert!(notes.iter().any(|n| n == "가져오기 전"), "{notes:?}");

    crate::commands::record::bulk_quick_replace_impl(&conn, 1, "가져온", "들여온", key).unwrap();

    for (_, note) in raw_col(&conn, "ActivityRecordHistory", "note") {
        let Some(note) = note else { continue };
        decrypt(&note, &key.unwrap())
            .unwrap_or_else(|e| panic!("평문으로 저장된 note가 있다: {note} ({e})"));
    }

    // 저장만 맞고 조회가 틀리면 화면에 암호문이 그대로 찍힌다.
    let notes = plain_notes(&conn, act, stu, key);
    assert!(notes.iter().any(|n| n == "빠른 텍스트 교체"), "{notes:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_migration_marks_purge_when_rows_change() {
    // 표시는 데이터 변경과 **같은 트랜잭션** 안에 들어가야 한다. 커밋 직후 죽어도
    // 표시가 파일에 남아야 다음 열기에 정리를 이어받을 수 있기 때문이다.
    let conn = setup_test_db();
    let (path_state, dir) = setup_temp_db_path_state();
    let crypto = crypto_state(None);
    seed_plaintext(&conn);
    let key = make_v1_encrypted(&conn, &crypto, &path_state, "password");

    let changed = encrypt_columns_introduced_in(&conn, Some(key), 2).unwrap();
    assert!(changed > 0);
    assert!(
        is_purge_pending(&conn).unwrap(),
        "평문이 프리 페이지에 남으므로 정리 표시를 남겨야 한다"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_encrypt_columns_introduced_in_is_noop_without_key() {
    let conn = setup_test_db();
    seed_plaintext(&conn);
    let before = raw_col(&conn, "Snapshot", "memo");
    assert_eq!(encrypt_columns_introduced_in(&conn, None, 2).unwrap(), 0);
    assert_eq!(raw_col(&conn, "Snapshot", "memo"), before);
}

// ── 방어 장치를 하나씩 떼어 보는 테스트 ──────────────────────
//
// 이중 암호화는 두 겹으로 막는다: since_version 필터와 시행 복호화.
// 둘이 서로를 가리면 어느 한쪽을 지우는 회귀가 아무 테스트도 빨갛게 만들지 못한다.
// 아래 두 테스트는 각각 한 겹만 지킨다.

#[test]
fn test_v2_step_does_not_touch_v1_columns() {
    // since_version 필터만 지키는 테스트.
    //
    // 시행 복호화는 "이미 이 키로 암호화된 값"을 걸러낸다. 그래서 v1 컬럼이 암호문인
    // 평범한 상태에서는 필터를 지워도 아무 일이 일어나지 않아 회귀가 드러나지 않는다.
    // 시행 복호화가 걸러내지 못하는 상태 — v1 컬럼이 **평문**인 상태 — 를 일부러 만든다.
    let conn = setup_test_db();
    let (path_state, dir) = setup_temp_db_path_state();
    let crypto = crypto_state(None);
    seed_plaintext(&conn);
    let key = make_v1_encrypted(&conn, &crypto, &path_state, "password");

    for (id, value) in raw_col(&conn, "Student", "name") {
        let plain = decrypt(&value.unwrap(), &key).unwrap();
        conn.execute(
            "UPDATE Student SET name=?1 WHERE id=?2",
            rusqlite::params![plain, id],
        )
        .unwrap();
    }
    let before = raw_col(&conn, "Student", "name");
    assert_eq!(before[0].1.as_deref(), Some("홍길동"));

    encrypt_columns_introduced_in(&conn, Some(key), 2).unwrap();

    assert_eq!(
        raw_col(&conn, "Student", "name"),
        before,
        "v2 단계는 since_version=1 컬럼을 건드리면 안 된다"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_data_key_is_refused_until_migration_finishes() {
    // 마이그레이션이 끝나지 않은 파일에 새 표현으로 쓰는 것을 막는 가드.
    // 암호화 경로 셋과 데이터 키 조회가 **모두** 같은 가드를 지나야 한다 —
    // 하나만 열려 있으면 그 경로로 정확히 "v1인데 메모는 암호문"인 파일이 만들어진다.
    let conn = setup_test_db();
    let (path_state, dir) = setup_temp_db_path_state();
    let crypto = crypto_state(None);
    seed_plaintext(&conn);
    make_v1_encrypted(&conn, &crypto, &path_state, "password");

    const GUARD: &str = "파일 형식 업데이트가 끝나지 않았습니다";
    let err = resolve_data_key(&conn, &crypto).unwrap_err();
    assert!(err.contains(GUARD), "resolve_data_key: {err}");

    let err = disable_encryption_impl(&conn, &crypto, &path_state).unwrap_err();
    assert!(err.contains(GUARD), "disable: {err}");

    let err = enable_encryption_impl(&conn, &crypto, &path_state, "password").unwrap_err();
    assert!(err.contains(GUARD), "enable: {err}");

    let err =
        change_encryption_password_impl(&conn, &crypto, &path_state, "password", "newpassword")
            .unwrap_err();
    assert!(err.contains(GUARD), "change_password: {err}");

    let mut conn = conn;
    migrate_schema_impl(&mut conn, &crypto, &path_state).unwrap();
    assert!(
        resolve_data_key(&conn, &crypto).unwrap().is_some(),
        "마이그레이션이 끝나면 통과해야 한다"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_data_step_error_rolls_back_its_own_writes() {
    // 훅이 몇 행을 바꾼 뒤 실패하는 경우 — 프로덕션에서 가장 있을 법한 실패다.
    let conn = setup_test_db();
    conn.pragma_update(None, "user_version", 1u32).unwrap();
    seed_plaintext(&conn);
    let before = raw_col(&conn, "Snapshot", "memo");

    let mut conn = conn;
    let err = db::migrate(&mut conn, 1, &|tx, _| {
        tx.execute("UPDATE Snapshot SET memo='바뀐 메모' WHERE memo IS NOT NULL", [])
            .map_err(|e| e.to_string())?;
        Err("변환 도중 실패".to_string())
    })
    .unwrap_err();

    assert!(err.contains("변환 도중 실패"), "{err}");
    assert_eq!(user_version(&conn), 1);
    assert_eq!(
        raw_col(&conn, "Snapshot", "memo"),
        before,
        "훅이 이미 쓴 행도 함께 롤백되어야 한다"
    );
}

// ── 업그레이드 백업 ──────────────────────────────────────────

fn backup_files(dir: &std::path::Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().to_string()))
        .filter(|n| n.contains("backup"))
        .collect()
}

#[test]
fn test_upgrade_backup_is_removed_on_success_and_kept_on_failure() {
    // 마이그레이션 직전의 사본에는 **평문 메모**가 들어 있다. 성공했는데 그 사본이
    // 남으면, 본 DB를 암호화해 놓고 그 옆에 비밀번호 없이 읽히는 복사본을 영구히
    // 두는 꼴이 된다. enable_encryption이 -pre-encrypt 백업을 지우는 것과 같은 이유다.
    // 실패했을 때는 반대로 복구 수단이므로 남아야 한다.
    let conn = setup_test_db();
    let (path_state, dir) = setup_temp_db_path_state();
    let crypto = crypto_state(None);
    seed_plaintext(&conn);
    make_v1_encrypted(&conn, &crypto, &path_state, "password");
    assert!(backup_files(&dir).is_empty(), "시작 상태가 깨끗해야 한다");

    // 1) 실패 — memo UPDATE만 막아 변환 도중에 멈춘다.
    conn.execute_batch(
        "CREATE TRIGGER block_memo BEFORE UPDATE ON Snapshot
         BEGIN SELECT RAISE(ABORT, '테스트'); END;",
    )
    .unwrap();
    let mut conn = conn;
    let err = migrate_schema_impl(&mut conn, &crypto, &path_state).unwrap_err();
    assert!(err.contains("복구용 백업"), "백업 경로를 알려야 한다: {err}");
    assert_eq!(user_version(&conn), 1);
    assert_eq!(
        backup_files(&dir).len(),
        1,
        "실패했으면 복구용 사본이 남아야 한다"
    );

    // 2) 성공 — 남아 있던 실패분까지 세지 않도록 지우고 다시 돌린다.
    for f in backup_files(&dir) {
        std::fs::remove_file(dir.join(f)).unwrap();
    }
    conn.execute_batch("DROP TRIGGER block_memo").unwrap();
    migrate_schema_impl(&mut conn, &crypto, &path_state).unwrap();
    assert_eq!(user_version(&conn), db::SCHEMA_VERSION);
    assert!(
        backup_files(&dir).is_empty(),
        "평문 메모가 든 사본이 남았다: {:?}",
        backup_files(&dir)
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_v0_file_without_app_configs_still_migrates() {
    // 버전 도입 이전 파일에는 APP_CONFIGS가 없을 수 있다. 암호화 키를 확보하려고
    // 그 테이블을 읽는 것이 DDL보다 먼저이므로, 무심코 조회하면 v0 파일이 v1로도
    // 올라가지 못한다. MIGRATIONS[0]이 존재하는 이유가 그 승격이다.
    let conn = setup_test_db();
    conn.execute_batch("DROP TABLE APP_CONFIGS").unwrap();
    conn.pragma_update(None, "user_version", 0u32).unwrap();

    let (path_state, dir) = setup_temp_db_path_state();
    let mut conn = conn;
    migrate_schema_impl(&mut conn, &crypto_state(None), &path_state).unwrap();

    assert_eq!(user_version(&conn), db::SCHEMA_VERSION);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn test_open_backup_is_skipped_while_memos_are_plaintext() {
    // 열 때마다 만드는 백업은 마이그레이션 **직전**에 돈다. 그 시점의 사본에는
    // 평문 메모가 들어 있고, 그 백업은 앱이 지우지 않으므로 영구히 남는다.
    // 그래서 이 경우에만 건너뛰고, migrate_schema_impl이 성공 시 지울 수 있는
    // -pre-upgrade 백업을 대신 만든다. 건너뛰기가 빠지면 본 DB를 암호화해 놓고
    // 비밀번호 없이 읽히는 사본을 그 옆에 남기게 된다.
    let conn = setup_test_db();
    let (path_state, dir) = setup_temp_db_path_state();
    let crypto = crypto_state(None);
    seed_plaintext(&conn);
    make_v1_encrypted(&conn, &crypto, &path_state, "password");

    let db_state = DbState(Mutex::new(Some(conn)));
    backup_project_impl(&db_state, &path_state).unwrap();
    assert!(
        backup_files(&dir).is_empty(),
        "변환 전 평문 사본이 남았다: {:?}",
        backup_files(&dir)
    );

    {
        let mut guard = db_state.0.lock().unwrap();
        let conn = guard.as_mut().unwrap();
        migrate_schema_impl(conn, &crypto, &path_state).unwrap();
    }
    assert!(
        backup_files(&dir).is_empty(),
        "변환에 성공했으면 -pre-upgrade 사본도 남지 않는다"
    );

    // v2가 된 뒤에는 평소대로 백업을 만든다. 건너뛰기가 그 상태에 눌러앉으면
    // 사용자는 열 때마다 생기던 백업을 영영 잃는다.
    backup_project_impl(&db_state, &path_state).unwrap();
    assert_eq!(
        backup_files(&dir).len(),
        1,
        "평소 열기에서는 백업을 만들어야 한다"
    );

    let _ = std::fs::remove_dir_all(dir);
}
