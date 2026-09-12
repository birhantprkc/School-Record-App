use crate::commands::config::set_config_impl;
use crate::engine::{validate_existing_path, validate_parent_dir_path};
use crate::state::{
    clear_crypto_state, CryptoStateHandle, DbPathState, DbState, ReplaceCacheState,
};
use rusqlite::{Connection, OptionalExtension};
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

    // 지난번에 끝내지 못한 정리(VACUUM)를 이어서 실행한다.
    //
    // 실패해도 파일 열기를 막지 않는다. 정리는 잔재를 지우는 뒷정리일 뿐이고,
    // 여기서 막으면 읽기 전용 매체나 디스크 여유가 없는 상황에서 사용자가 자기
    // 파일을 아예 열 수 없게 된다. 데이터를 잃는 것보다 잔재가 남는 편이 낫다.
    //
    // 조용히 넘기는 것은 아니다. 실패하면 표시가 그대로 남고,
    // get_encryption_status가 purge_pending으로 알려 설정 화면에 경고와
    // "지금 정리" 버튼이 표시된다.
    let _ = crate::commands::crypto::resume_pending_purge(&conn);

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
/// 백업으로 복제되지도 않는다.
///
/// 실패해도 파일이 안 남는 것은 아니다 — 만들기 전에 실패하면 안 남지만 중간에
/// 실패하면 만들다 만 파일이 남는다. 그래서 실패 시 직접 지운다.
pub(crate) fn backup_project_impl(
    db_state: &DbState,
    db_path_state: &DbPathState,
) -> Result<(), String> {
    // 락 순서는 코드베이스 공통 순서(DbState → DbPathState)를 따른다. 뒤집으면 교착.
    let guard = db_state.0.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("열린 프로젝트가 없습니다.")?;

    // 이번 열기에서 메모가 암호화될 파일이면 여기서 백업을 뜨지 않는다.
    //
    // 지금 뜨면 그 사본에는 **평문 메모**가 들어간다. 몇 초 뒤 본 DB는 암호화되는데
    // 비밀번호 없이 읽히는 사본이 그 옆에 영구히 남는 꼴이다. enable_encryption이
    // -pre-encrypt 백업을 성공 시 반드시 지우는 것과 같은 이유다.
    //
    // 대신 migrate_schema_impl이 -pre-upgrade 백업을 직접 만들고, 변환에 성공하면
    // 지운다. 실패하면 남겨서 복구 수단으로 쓴다.
    if will_encrypt_memos(conn)? {
        return Ok(());
    }

    let path_guard = db_path_state.0.lock().map_err(|e| e.to_string())?;
    let src = path_guard.as_ref().ok_or("DB path not set")?;
    let dest = crate::engine::unique_backup_path(src, "")?;
    let dest_str = dest
        .to_str()
        .ok_or("백업 경로를 문자열로 변환하지 못했습니다.")?;

    // 경로를 SQL에 직접 넣지 않고 바인딩한다 — 한글·역슬래시 이스케이프 문제를 피한다.
    if let Err(e) = conn.execute("VACUUM INTO ?1", rusqlite::params![dest_str]) {
        std::fs::remove_file(&dest).ok();
        return Err(format!("백업 생성 실패: {e}"));
    }
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

/// 파일을 현재 스키마 버전까지 올린다.
///
/// 데이터 변환이 필요한 단계(v1→v2의 메모 암호화)가 있으므로 암호화 키를 받는다.
/// 키는 **트랜잭션을 열기 전에** 확보한다. 암호화가 켜져 있는데 잠금 상태면 여기서
/// 멈추고 버전을 올리지 않는다. 버전만 먼저 오르고 데이터가 옛 표현으로 남는 파일은
/// 다음에 열릴 때 새 표현으로 읽히므로 복구가 안 된다.
///
/// 호출 시점에 키가 있는 것은 프론트엔드가 보장한다 — 파일을 열 때
/// unlock → backup → migrate 순서이고, 비밀번호를 취소하면 파일을 닫는다.
/// 암호화 설정을 담는 APP_CONFIGS가 있는가.
///
/// 버전 도입 이전(v0) 파일에는 이 테이블이 아예 없을 수 있다. 그 시절에는 암호화
/// 기능 자체가 없었으므로, 없으면 "암호화를 쓰지 않는 파일"로 본다.
///
/// 테이블 유무를 보지 않고 조회하면 v0 파일이 v1로도 올라가지 못한다 —
/// `MIGRATIONS[0]`이 존재하는 이유가 바로 그 승격이다. 조회 실패를 삼키는 것은
/// 아니다. 테이블이 있으면 읽기 오류는 그대로 올린다(`get_config_impl` 주석 참고 —
/// 읽기 실패를 None으로 뭉개면 암호화된 DB를 평문으로 취급하게 된다).
fn has_app_configs(conn: &Connection) -> Result<bool, String> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='APP_CONFIGS'",
            [],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .is_some())
}

/// 이번 열기에서 메모가 평문에서 암호문으로 바뀌는 파일인가.
///
/// 참이면 이 시점의 DB 사본은 평문 메모를 담는다. 백업을 어떻게 다룰지가 달라진다.
fn will_encrypt_memos(conn: &Connection) -> Result<bool, String> {
    if !has_app_configs(conn)? {
        return Ok(false);
    }
    let version: u32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    Ok(version < crate::db::SCHEMA_VERSION && crate::commands::crypto::is_encryption_enabled(conn)?)
}

pub fn migrate_schema_impl(
    conn: &mut Connection,
    crypto: &CryptoStateHandle,
    db_path_state: &DbPathState,
) -> Result<(), String> {
    let from: u32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if from >= crate::db::SCHEMA_VERSION {
        return Ok(());
    }

    let key = if has_app_configs(conn)? {
        crate::commands::crypto::resolve_data_key_unchecked(conn, crypto)?
    } else {
        None
    };

    // 평문 메모를 담을 백업은 여기서 만들고 성공하면 지운다(backup_project_impl 주석 참고).
    // 실패하면 남는다 — 그때는 복구 수단이 필요하고, 아직 평문인 파일의 사본이라
    // 새로 새는 정보도 없다.
    let backup = if will_encrypt_memos(conn)? {
        Some(crate::commands::crypto::backup_db_file(
            conn,
            db_path_state,
            "-pre-upgrade",
        )?)
    } else {
        None
    };

    // 어느 버전에서 무엇을 암호화하는지는 ENCRYPTED_COLUMNS의 since_version 하나가
    // 정한다. 여기서 버전별로 분기하면 지식이 두 곳으로 갈라져, 다음에 컬럼을 추가한
    // 사람이 한쪽만 고치고 조용히 빠뜨릴 수 있다.
    let result = crate::db::migrate(conn, from, &|tx, to| {
        crate::commands::crypto::encrypt_columns_introduced_in(tx, key, to).map(|_| ())
    });

    if let Err(e) = result {
        // 단계마다 커밋하므로 여러 버전을 건너뛰는 파일은 중간 버전까지 적용됐을 수 있다.
        // "그대로입니다"라고 단정하면 사용자가 필요 없는 복구를 한다.
        let now: u32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap_or(from);
        let state = if now == from {
            "파일은 이전 상태 그대로입니다.".to_string()
        } else {
            format!("파일은 v{now}까지 적용된 상태입니다.")
        };
        let hint = if e.contains("외래키") {
            // 다시 열어도 같은 지점에서 멈춘다. 재시도를 권하면 안 된다.
            "파일 안의 연결 정보가 어긋나 있습니다. 다시 열어도 같은 결과이므로,              백업 파일로 되돌리거나 도움을 요청해주세요."
        } else {
            "디스크 여유 공간을 확인하고, 파일이 다른 프로그램에서 열려 있지 않은지              확인한 뒤 다시 열어주세요."
        };
        let backup_note = match &backup {
            Some(p) => format!("
복구용 백업이 남아 있습니다: {}", p.display()),
            None => String::new(),
        };
        return Err(format!(
            "파일 형식 업데이트(v{from} → v{}) 중 오류가 발생해 변경을 취소했습니다.              {state} {hint} ({e}){backup_note}",
            crate::db::SCHEMA_VERSION
        ));
    }

    // 평문 메모 사본을 남기지 않는다. 지우지 못하면 알린다 — 조용히 넘기면 사용자는
    // 사본이 없는 줄 알지만 실제로는 남아 있게 된다. 마이그레이션은 이미 커밋됐으므로
    // 다시 열면 정상 동작한다.
    if let Some(path) = backup {
        crate::commands::crypto::remove_backup_after_success(&path, "파일 형식 업데이트")?;
    }

    // 변환된 행이 있으면 마이그레이션 트랜잭션이 정리 표시를 남겼다. 옛 페이지에 남은
    // 평문 메모를 지운다. 표시가 없으면 아무것도 하지 않는다.
    //
    // 실패해도 마이그레이션 자체는 이미 커밋됐으므로 오류로 막지 않는다.
    // 조용히 넘기는 것은 아니다 — 표시가 그대로 남아 get_encryption_status가
    // purge_pending으로 알리고, 설정 화면에 "지금 정리" 버튼이 나온다.
    // (open_project_impl의 resume_pending_purge는 migrate보다 먼저 돌기 때문에
    //  이번 열기의 표시를 처리하지 못한다. 그래서 여기서 한 번 더 시도한다.)
    let _ = crate::commands::crypto::resume_pending_purge(conn);

    Ok(())
}

#[tauri::command]
pub fn migrate_schema(
    state: State<DbState>,
    crypto: State<CryptoStateHandle>,
    db_path: State<DbPathState>,
) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_mut().ok_or("DB not open")?;
    migrate_schema_impl(conn, &crypto, &db_path)
}
