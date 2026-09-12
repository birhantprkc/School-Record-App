use crate::state::DbPathState;
use rusqlite::Connection;
use std::sync::Mutex;

pub mod engine_tests;
pub mod db_tests;
pub mod schema_lock_tests;
pub mod migration_v2_tests;
pub mod project_tests;
pub mod area_tests;
pub mod activity_tests;
pub mod config_tests;
pub mod student_tests;
pub mod record_tests;
pub mod snapshot_tests;
pub mod replace_tests;
pub mod synonym_tests;
pub mod crypto_tests;
pub mod crypto_cmd_tests;
pub mod crypto_vector_tests;
pub mod real_file_tests;

/// 테스트에서 백업 경로가 필요한 경우 사용. 임시 파일을 생성하고 DbPathState를 반환한다.
/// 반환된 PathBuf(디렉토리)는 테스트 종료 후 직접 삭제해야 한다.
pub fn setup_temp_db_path_state() -> (DbPathState, std::path::PathBuf) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir()
        .join(format!("school_record_path_test_{}_{}", std::process::id(), nanos));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.db");
    std::fs::write(&path, b"").unwrap();
    let state = DbPathState(Mutex::new(Some(path)));
    (state, dir)
}

pub fn setup_test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    conn.execute_batch(include_str!("../schema.sql")).unwrap();
    // 실제 파일과 같은 상태로 맞춘다. db::create_new도 여기서 user_version을 기록하며,
    // resolve_data_key가 "마이그레이션이 끝난 파일인가"를 이 값으로 판단한다.
    conn.pragma_update(None, "user_version", crate::db::SCHEMA_VERSION)
        .unwrap();
    conn
}

pub fn insert_student(conn: &Connection, grade: i64, class_num: i64, number: i64, name: &str) -> i64 {
    conn.execute(
        "INSERT INTO Student (grade, class_num, number, name) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![grade, class_num, number, name],
    )
    .unwrap();
    conn.last_insert_rowid()
}

pub fn insert_area(conn: &Connection, name: &str, byte_limit: i64) -> i64 {
    conn.execute(
        "INSERT INTO Area (name, byte_limit) VALUES (?1, ?2)",
        rusqlite::params![name, byte_limit],
    )
    .unwrap();
    conn.last_insert_rowid()
}

pub fn insert_activity(conn: &Connection, name: &str) -> i64 {
    conn.execute(
        "INSERT INTO Activity (name) VALUES (?1)",
        rusqlite::params![name],
    )
    .unwrap();
    conn.last_insert_rowid()
}

pub fn insert_record(conn: &Connection, activity_id: i64, student_id: i64, content: &str) {
    conn.execute(
        "INSERT INTO ActivityRecord (activity_id, student_id, content) VALUES (?1, ?2, ?3)",
        rusqlite::params![activity_id, student_id, content],
    )
    .unwrap();
}
