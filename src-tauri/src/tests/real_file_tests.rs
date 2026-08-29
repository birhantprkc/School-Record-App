//! 임시 검증 하니스 — PR #29 그룹 4(rusqlite 0.40 / 번들 SQLite 3.53) 확인용.
//!
//! 기존 사용자 파일은 SQLite 3.45로 작성됐다. 기존 테스트는 전부 in-memory DB를
//! schema.sql로 새로 만들기 때문에, "예전 엔진이 쓴 실제 파일"을 여는 경로는
//! 한 번도 실행되지 않는다. 이 하니스가 그 공백을 메운다.
//!
//! 원본은 절대 건드리지 않는다. 항상 사본을 만들어 그 위에서만 작업한다.
//! 개인정보가 들어 있으므로 내용은 출력하지 않고 개수·검사 결과만 출력한다.

use crate::commands::activity::get_activities_impl;
use crate::commands::area::{create_area_impl, delete_area_impl, get_areas_impl};
use crate::commands::crypto::is_encryption_enabled;
use crate::commands::project::{migrate_schema_impl, open_project_impl};
use crate::commands::student::get_students_impl;
use crate::state::{
    CryptoState, CryptoStateHandle, DbPathState, DbState, ReplaceCache, ReplaceCacheState,
};
use rusqlite::Connection;
use sha2::{Digest, Sha256};

const LOCKED_V1: &str = "fc7c11a3d03c8d4a104f2ec9788745928bcb11cef517bab90b0be463dedb6be2";
const PROBE: &str = "__pr29_write_probe__";

/// 파일 헤더 오프셋 96: 이 파일을 마지막으로 쓴 SQLite 라이브러리 버전
fn sqlite_write_version(path: &std::path::Path) -> String {
    let bytes = std::fs::read(path).unwrap();
    let n = u32::from_be_bytes([bytes[96], bytes[97], bytes[98], bytes[99]]);
    format!("{}.{}.{}", n / 1_000_000, (n / 1_000) % 1_000, n % 1_000)
}

fn schema_fingerprint(conn: &Connection) -> String {
    let sql = "SELECT type, name, COALESCE(sql, '') FROM sqlite_master \
               WHERE name NOT LIKE 'sqlite\\_%' ESCAPE '\\' ORDER BY type, name";
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt
        .query_map([], |r| {
            let t: String = r.get(0)?;
            let n: String = r.get(1)?;
            let s: String = r.get(2)?;
            let normalized = s.split_whitespace().collect::<Vec<_>>().join(" ");
            Ok(format!("{t}|{n}|{normalized}"))
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect::<Vec<_>>();
    let mut h = Sha256::new();
    h.update(rows.join("\n").as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap_or(-1)
}

fn fresh_states() -> (DbState, DbPathState, CryptoStateHandle, ReplaceCacheState) {
    (
        DbState(std::sync::Mutex::new(None)),
        DbPathState(std::sync::Mutex::new(None)),
        std::sync::Mutex::new(CryptoState { key: None }),
        std::sync::Mutex::new(ReplaceCache {
            ruleset_version: 0,
            entries: std::collections::HashMap::new(),
        }),
    )
}

#[test]
#[ignore = "실제 사용자 파일이 필요하다. REAL_DB_DIR로 경로를 지정해 수동 실행한다."]
fn verify_real_user_files() {
    let src_dir = std::env::var("REAL_DB_DIR").expect("REAL_DB_DIR 미설정");
    let work = std::env::temp_dir().join(format!("real_db_verify_{}", std::process::id()));
    std::fs::create_dir_all(&work).unwrap();

    let mut files: Vec<_> = std::fs::read_dir(&src_dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("db"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "db 파일이 없습니다: {src_dir}");

    let mut failures: Vec<String> = Vec::new();
    let mut fail = |name: &str, what: String| failures.push(format!("{name} — {what}"));

    for (i, src) in files.iter().enumerate() {
        let name = src.file_name().unwrap().to_string_lossy().to_string();
        let before = std::fs::read(src).unwrap();

        // 원본 보호: 반드시 사본에서만 작업한다.
        let dst = work.join(format!("case{i}.db"));
        std::fs::copy(src, &dst).unwrap();

        println!("\n=== [{}] {name} ===", i + 1);
        println!("  이 파일을 쓴 SQLite: {}", sqlite_write_version(&dst));
        println!("  지금 여는 엔진:      {}", rusqlite::version());

        let (db, path_state, crypto, cache) = fresh_states();

        // 1) 실제 open_project 커맨드 경로
        match open_project_impl(dst.to_str().unwrap(), &db, &path_state, &crypto, &cache) {
            Ok(()) => println!("  [OK]   open_project"),
            Err(e) => {
                println!("  [FAIL] open_project: {e}");
                fail(&name, format!("open: {e}"));
                continue;
            }
        }

        let mut guard = db.0.lock().unwrap();
        let conn = guard.as_mut().unwrap();

        // 2) 무결성
        let integrity: String = conn
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .unwrap();
        println!("  integrity_check: {integrity}");
        if integrity != "ok" {
            fail(&name, format!("integrity: {integrity}"));
        }

        let fk_bad = conn
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .query_map([], |_| Ok(()))
            .unwrap()
            .count();
        println!("  foreign_key_check 위반: {fk_bad}");
        if fk_bad != 0 {
            fail(&name, format!("fk 위반 {fk_bad}건"));
        }

        // 3) 버전 / 스키마 구조
        let uv: u32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        let fp = schema_fingerprint(conn);
        println!(
            "  user_version: {uv} (앱 SCHEMA_VERSION: {})",
            crate::db::SCHEMA_VERSION
        );
        println!("  스키마 지문 == 고정 v1: {}", fp == LOCKED_V1);
        if fp != LOCKED_V1 {
            println!("    실제 지문: {fp}");
        }
        println!("  암호화 사용: {}", is_encryption_enabled(conn).unwrap());

        // 4) 데이터 규모
        print!("  행 수:");
        for t in ["Student", "Area", "Activity", "ActivityRecord"] {
            print!(" {t}={}", count(conn, t));
        }
        println!();

        // 5) 마이그레이션 경로
        match migrate_schema_impl(conn) {
            Ok(()) => {
                let uv2: u32 = conn
                    .query_row("PRAGMA user_version", [], |r| r.get(0))
                    .unwrap();
                println!("  [OK]   migrate_schema → user_version {uv2}");
            }
            Err(e) => {
                println!("  [FAIL] migrate_schema: {e}");
                fail(&name, format!("migrate: {e}"));
            }
        }

        // 6) 실제 조회 커맨드 (암호화 파일은 키 없이 = 저장된 원문 컬럼만 확인)
        match get_students_impl(conn, None) {
            Ok(v) => println!("  [OK]   get_students: {}건", v.len()),
            Err(e) => {
                println!("  [FAIL] get_students: {e}");
                fail(&name, format!("get_students: {e}"));
            }
        }
        match get_areas_impl(conn) {
            Ok(v) => println!("  [OK]   get_areas: {}건", v.len()),
            Err(e) => {
                println!("  [FAIL] get_areas: {e}");
                fail(&name, format!("get_areas: {e}"));
            }
        }
        match get_activities_impl(conn) {
            Ok(v) => println!("  [OK]   get_activities: {}건", v.len()),
            Err(e) => {
                println!("  [FAIL] get_activities: {e}");
                fail(&name, format!("get_activities: {e}"));
            }
        }

        // 7) 쓰기 → 되읽기 → 되돌리기 (새 엔진이 예전 파일에 쓸 수 있는가)
        match create_area_impl(conn, PROBE, 1500) {
            Ok(id) => {
                let wrote = get_areas_impl(conn).unwrap().iter().any(|a| a.name == PROBE);
                delete_area_impl(conn, id).unwrap();
                let cleaned = !get_areas_impl(conn).unwrap().iter().any(|a| a.name == PROBE);
                println!("  [OK]   쓰기/되읽기/삭제: write={wrote} cleanup={cleaned}");
                if !wrote || !cleaned {
                    fail(&name, "write probe 불일치".into());
                }
            }
            Err(e) => {
                println!("  [FAIL] 쓰기: {e}");
                fail(&name, format!("write: {e}"));
            }
        }

        let integrity2: String = conn
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .unwrap();
        println!("  쓰기 후 integrity_check: {integrity2}");
        if integrity2 != "ok" {
            fail(&name, format!("쓰기 후 integrity: {integrity2}"));
        }

        drop(guard);

        // 8) 닫았다 다시 열기
        let (db2, p2, c2, ca2) = fresh_states();
        match open_project_impl(dst.to_str().unwrap(), &db2, &p2, &c2, &ca2) {
            Ok(()) => println!(
                "  [OK]   쓰기 후 재열기 (이제 {} 기록)",
                sqlite_write_version(&dst)
            ),
            Err(e) => {
                println!("  [FAIL] 재열기: {e}");
                fail(&name, format!("reopen: {e}"));
            }
        }

        // 원본이 1바이트도 안 변했는지 확인
        let after = std::fs::read(src).unwrap();
        let untouched = before == after;
        println!("  원본 무변경: {untouched}");
        if !untouched {
            fail(&name, "원본 파일이 변경됨".into());
        }
    }

    std::fs::remove_dir_all(&work).ok();

    println!("\n===== 요약 =====");
    if failures.is_empty() {
        println!("전 파일 통과 ({}개)", files.len());
    } else {
        for f in &failures {
            println!("실패: {f}");
        }
    }
    assert!(failures.is_empty(), "{}건 실패", failures.len());
}
