use crate::commands::config::set_config_impl;
use crate::engine::{validate_existing_path, validate_parent_dir_path};
use crate::state::{
    clear_crypto_state, CryptoStateHandle, DbPathState, DbState, ReplaceCacheState,
};
use rusqlite::Connection;
use tauri::State;

pub(crate) fn new_project_impl(
    path: &str,
    current_version: &str,
    state: &DbState,
    db_path_state: &DbPathState,
    crypto: &CryptoStateHandle,
    cache: &ReplaceCacheState,
) -> Result<(), String> {
    validate_parent_dir_path(path, "디렉토리가 존재하지 않습니다.")?;
    let p = std::path::Path::new(&path);
    if p.exists() {
        return Err(format!("이미 파일이 존재합니다: {path}"));
    }
    let conn = crate::db::create_new(p).map_err(|e| e.to_string())?;
    set_config_impl(&conn, "app_version", current_version)?;
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    *guard = Some(conn);
    *db_path_state.0.lock().map_err(|e| e.to_string())? = Some(p.to_path_buf());
    clear_crypto_state(crypto)?;
    // 키만 지우고 캐시를 두면 이전 프로젝트의 평문이 메모리에 그대로 남는다.
    cache.lock().map_err(|e| e.to_string())?.invalidate();
    Ok(())
}

pub(crate) fn open_project_impl(
    path: &str,
    state: &DbState,
    db_path_state: &DbPathState,
    crypto: &CryptoStateHandle,
    cache: &ReplaceCacheState,
) -> Result<(), String> {
    validate_existing_path(path, "파일이 존재하지 않거나 접근할 수 없습니다.")?;
    let src = std::path::Path::new(&path);
    let conn = crate::db::open_existing(src).map_err(|e| e.to_string())?;
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    *guard = Some(conn);
    *db_path_state.0.lock().map_err(|e| e.to_string())? = Some(src.to_path_buf());
    clear_crypto_state(crypto)?;
    // 키만 지우고 캐시를 두면 이전 프로젝트의 평문이 메모리에 그대로 남는다.
    cache.lock().map_err(|e| e.to_string())?.invalidate();
    Ok(())
}

/// 열 때마다 만드는 백업.
///
/// 예전에는 살아 있는 DB 파일을 `fs::copy`로 그대로 떴다. 이 방식은 SQLite 락을
/// 거치지 않으므로, 다른 쓰기와 겹치면 저널 없이 반쯤 커밋된 페이지를 담은
/// **조용히 손상된 백업**이 만들어질 수 있다. 정작 필요한 순간에야 발견된다.
///
/// `VACUUM INTO`는 SQLite가 직접 일관된 스냅샷을 만든다. 부수 효과로
/// **프리 페이지를 복사하지 않으므로**, 예전에 프리리스트에 남아 있던 평문이
/// 백업으로 복제되지도 않는다. 실패 시 대상 파일을 남기지 않는 점도 fs::copy보다 낫다.
pub(crate) fn backup_project_impl(
    db_state: &DbState,
    db_path_state: &DbPathState,
) -> Result<(), String> {
    // 락 순서는 코드베이스 공통 순서(DbState → DbPathState)를 따른다. 뒤집으면 교착.
    let guard = db_state.0.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("열린 프로젝트가 없습니다.")?;

    let path_guard = db_path_state.0.lock().map_err(|e| e.to_string())?;
    let src = path_guard.as_ref().ok_or("DB path not set")?;
    let dest = crate::engine::unique_backup_path(src, "")?;
    let dest_str = dest
        .to_str()
        .ok_or("백업 경로를 문자열로 변환하지 못했습니다.")?;

    // 경로를 SQL에 직접 넣지 않고 바인딩한다 — 한글·역슬래시 이스케이프 문제를 피한다.
    conn.execute("VACUUM INTO ?1", rusqlite::params![dest_str])
        .map_err(|e| format!("백업 생성 실패: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn new_project(
    path: String,
    app: tauri::AppHandle,
    state: State<DbState>,
    db_path: State<DbPathState>,
    crypto: State<CryptoStateHandle>,
    cache: State<ReplaceCacheState>,
) -> Result<(), String> {
    let version = app.package_info().version.to_string();
    new_project_impl(&path, &version, &state, &db_path, &crypto, &cache)
}

#[tauri::command]
pub fn open_project(
    path: String,
    state: State<DbState>,
    db_path: State<DbPathState>,
    crypto: State<CryptoStateHandle>,
    cache: State<ReplaceCacheState>,
) -> Result<(), String> {
    open_project_impl(&path, &state, &db_path, &crypto, &cache)
}

#[tauri::command]
pub fn backup_project(state: State<DbState>, db_path: State<DbPathState>) -> Result<(), String> {
    backup_project_impl(&state, &db_path)
}

pub fn migrate_schema_impl(conn: &mut Connection) -> Result<(), String> {
    let from: u32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if from < crate::db::SCHEMA_VERSION {
        crate::db::migrate(conn, from).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn migrate_schema(state: State<DbState>) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_mut().ok_or("DB not open")?;
    migrate_schema_impl(conn)
}
