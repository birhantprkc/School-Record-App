use crate::commands::config::check_and_update_app_version_impl;
use crate::commands::project::{new_project_impl, open_project_impl};
use crate::state::{
    current_crypto_key, CryptoState, CryptoStateHandle, DbPathState, DbState, ReplaceCache,
    ReplaceCacheState,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

fn crypto_state_with_key() -> CryptoStateHandle {
    Mutex::new(CryptoState {
        key: Some([7u8; 32]),
    })
}

/// 이전 프로젝트에서 쓰던 것처럼 평문 한 건이 들어 있는 캐시.
fn cache_with_plaintext() -> ReplaceCacheState {
    let mut entries = HashMap::new();
    entries.insert(
        "이전 프로젝트 학생 기록".to_string(),
        ("이전 프로젝트 학생 기록".to_string(), 0u64),
    );
    Mutex::new(ReplaceCache {
        ruleset_version: 0,
        entries,
    })
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "school_record_app_{label}_{}_{}",
        std::process::id(),
        nanos
    ))
}

#[test]
fn test_new_project_clears_crypto_state() {
    let dir = unique_temp_dir("new_project");
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("new.db");
    let db = DbState(Mutex::new(None));
    let db_path = DbPathState(Mutex::new(None));
    let crypto = crypto_state_with_key();
    let cache = cache_with_plaintext();

    new_project_impl(path.to_str().unwrap(), "0.2.13", &db, &db_path, &crypto, &cache).unwrap();

    assert!(db.0.lock().unwrap().is_some());
    assert!(current_crypto_key(&crypto).unwrap().is_none());
    // 키만 지우고 캐시를 두면 이전 프로젝트의 평문이 메모리에 남는다.
    assert!(
        cache.lock().unwrap().entries.is_empty(),
        "프로젝트를 열면 치환 캐시의 평문도 함께 비워져야 한다"
    );

    drop(db); // Windows: 파일 잠금 해제 후 삭제
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_new_project_then_open_does_not_show_modal() {
    // 신규 파일 생성 직후 같은 버전으로 check_and_update → None 반환(모달 없음)
    // new_project_impl이 app_version을 DB에 기록하는지 검증하는 것이 핵심
    let dir = unique_temp_dir("new_then_open");
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("new.db");
    let db = DbState(Mutex::new(None));
    let db_path = DbPathState(Mutex::new(None));
    let crypto = crypto_state_with_key();
    let cache = cache_with_plaintext();

    new_project_impl(path.to_str().unwrap(), "0.2.13", &db, &db_path, &crypto, &cache).unwrap();

    let guard = db.0.lock().unwrap();
    let conn = guard.as_ref().unwrap();
    let result = check_and_update_app_version_impl(conn, "0.2.13").unwrap();
    assert!(result.is_none(), "신규 파일 첫 오픈 시 모달이 표시되면 안 됩니다");

    drop(guard);
    drop(db);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_open_project_clears_crypto_state() {
    let dir = unique_temp_dir("open_project");
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("existing.db");
    drop(crate::db::create_new(&path).unwrap());

    let db = DbState(Mutex::new(None));
    let db_path = DbPathState(Mutex::new(None));
    let crypto = crypto_state_with_key();
    let cache = cache_with_plaintext();

    open_project_impl(path.to_str().unwrap(), &db, &db_path, &crypto, &cache).unwrap();

    assert!(db.0.lock().unwrap().is_some());
    assert!(current_crypto_key(&crypto).unwrap().is_none());
    // 키만 지우고 캐시를 두면 이전 프로젝트의 평문이 메모리에 남는다.
    assert!(
        cache.lock().unwrap().entries.is_empty(),
        "프로젝트를 열면 치환 캐시의 평문도 함께 비워져야 한다"
    );

    drop(db); // Windows: 파일 잠금 해제 후 삭제
    std::fs::remove_dir_all(&dir).unwrap();
}

// ── 열 때 만드는 백업 (감사 F3) ───────────────────────────────
//
// 예전에는 살아 있는 DB를 fs::copy로 떠서, 다른 쓰기와 겹치면 조용히 손상된
// 백업이 만들어질 수 있었다. VACUUM INTO로 바꿔 SQLite가 일관된 스냅샷을
// 만들게 한다. 부수 효과로 프리 페이지(평문이 남을 수 있는 곳)가 복사되지 않는다.

fn open_states() -> (DbState, DbPathState, CryptoStateHandle, ReplaceCacheState) {
    (
        DbState(std::sync::Mutex::new(None)),
        DbPathState(std::sync::Mutex::new(None)),
        std::sync::Mutex::new(CryptoState { key: None }),
        std::sync::Mutex::new(ReplaceCache { ruleset_version: 0, entries: HashMap::new() }),
    )
}

fn temp_project_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("bkp_{}_{}_{}", tag, std::process::id(), nanos));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_backup_project_produces_valid_readable_copy() {
    let dir = temp_project_dir("valid");
    let path = dir.join("학생부.db");
    let (db, db_path, crypto, cache) = open_states();
    new_project_impl(path.to_str().unwrap(), "0.2.22", &db, &db_path, &crypto, &cache).unwrap();

    // 데이터를 넣어 백업에 실려야 할 내용을 만든다.
    {
        let guard = db.0.lock().unwrap();
        let conn = guard.as_ref().unwrap();
        conn.execute("INSERT INTO Student (grade, class_num, number, name) VALUES (1,1,1,'홍길동')", [])
            .unwrap();
    }

    crate::commands::project::backup_project_impl(&db, &db_path).unwrap();

    let backups: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_string_lossy().ends_with(".db.backup"))
        .collect();
    assert_eq!(backups.len(), 1, "백업이 하나 만들어져야 한다");

    // 백업이 실제로 열리고, 무결성이 온전하며, 데이터가 실려 있어야 한다.
    let bconn = rusqlite::Connection::open(&backups[0]).unwrap();
    let integrity: String = bconn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok", "백업이 손상되면 안 된다");

    let name: String = bconn
        .query_row("SELECT name FROM Student WHERE grade=1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(name, "홍길동");

    // VACUUM INTO는 프리 페이지를 복사하지 않는다 (평문 잔재가 딸려가지 않는다).
    let freelist: i64 = bconn
        .query_row("PRAGMA freelist_count", [], |r| r.get(0))
        .unwrap();
    assert_eq!(freelist, 0, "백업에 프리 페이지가 남으면 안 된다");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_backup_project_twice_keeps_both() {
    let dir = temp_project_dir("twice");
    let path = dir.join("학생부.db");
    let (db, db_path, crypto, cache) = open_states();
    new_project_impl(path.to_str().unwrap(), "0.2.22", &db, &db_path, &crypto, &cache).unwrap();

    // 같은 초에 두 번 백업해도 앞의 것을 덮어쓰면 안 된다(F2와 함께 검증).
    crate::commands::project::backup_project_impl(&db, &db_path).unwrap();
    crate::commands::project::backup_project_impl(&db, &db_path).unwrap();

    let count = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_string_lossy().ends_with(".db.backup"))
        .count();
    assert_eq!(count, 2, "두 번째 백업이 첫 번째를 덮어쓰면 안 된다");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_backup_project_without_open_project_errors() {
    let (db, db_path, _c, _ca) = open_states();
    let err = crate::commands::project::backup_project_impl(&db, &db_path).unwrap_err();
    assert!(!err.is_empty(), "열린 프로젝트가 없으면 명시적으로 실패해야 한다");
}


// ── 열 때 밀린 정리(VACUUM) 이어받기 ─────────────────────

/// 암호화 직후 커밋은 됐지만 VACUUM 전에 프로세스가 죽은 파일을 흉내낸다.
/// 예전에는 그 잔재를 지울 방법이 앱 안에 없었다.
#[test]
fn test_open_project_resumes_pending_purge() {
    let dir = temp_project_dir("resume_purge");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("pending.db");
    {
        let conn = crate::db::create_new(&path).unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO APP_CONFIGS (config_key, config_value) \
             VALUES ('encryption_purge_pending', ?1)",
            ["암호화"],
        )
        .unwrap();
    }

    let (db, db_path, crypto, cache) = open_states();
    open_project_impl(path.to_str().unwrap(), &db, &db_path, &crypto, &cache).unwrap();

    let guard = db.0.lock().unwrap();
    let conn = guard.as_ref().unwrap();
    assert!(
        !crate::commands::crypto::is_purge_pending(conn).unwrap(),
        "파일을 열면 밀린 정리를 이어서 끝내고 표시를 지워야 한다"
    );

    drop(guard);
    drop(db);
    std::fs::remove_dir_all(&dir).ok();
}

/// 표시가 없는 평범한 파일은 열기 경로가 그대로여야 한다.
#[test]
fn test_open_project_without_pending_flag_is_unaffected() {
    let dir = temp_project_dir("no_pending_purge");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("plain.db");
    drop(crate::db::create_new(&path).unwrap());

    let (db, db_path, crypto, cache) = open_states();
    open_project_impl(path.to_str().unwrap(), &db, &db_path, &crypto, &cache).unwrap();

    let guard = db.0.lock().unwrap();
    assert!(!crate::commands::crypto::is_purge_pending(guard.as_ref().unwrap()).unwrap());

    drop(guard);
    drop(db);
    std::fs::remove_dir_all(&dir).ok();
}

/// 정리에 실패해도 파일은 열려야 한다.
///
/// 뒷정리 실패로 파일을 아예 못 열게 되면, 읽기 전용 매체나 디스크 여유가 없는
/// 사용자가 자기 기록에 접근할 수 없다. 잔재가 남는 편이 낫다.
#[test]
fn test_open_project_succeeds_even_if_purge_fails() {
    let dir = temp_project_dir("purge_fail_open");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("busy.db");
    {
        let conn = crate::db::create_new(&path).unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO APP_CONFIGS (config_key, config_value) \
             VALUES ('encryption_purge_pending', ?1)",
            ["암호화"],
        )
        .unwrap();
    }

    // RESERVED 락을 잡아 VACUUM만 실패시킨다. 읽기는 통과하므로 열기는 진행된다.
    let blocker = rusqlite::Connection::open(&path).unwrap();
    blocker.execute_batch("BEGIN IMMEDIATE").unwrap();

    let (db, db_path, crypto, cache) = open_states();
    open_project_impl(path.to_str().unwrap(), &db, &db_path, &crypto, &cache).unwrap();

    let guard = db.0.lock().unwrap();
    assert!(
        crate::commands::crypto::is_purge_pending(guard.as_ref().unwrap()).unwrap(),
        "정리에 실패했으면 표시가 남아 다음에 다시 시도되어야 한다"
    );

    drop(guard);
    drop(db);
    blocker.execute_batch("ROLLBACK").unwrap();
    drop(blocker);
    std::fs::remove_dir_all(&dir).ok();
}
