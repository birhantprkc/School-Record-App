use crate::commands::config::{has_app_configs, set_config_impl};
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

    // **마이그레이션이 끝난 뒤에만 백업한다.**
    //
    // 메모 암호화로 넘어가는 파일을 변환 전에 뜨면 그 사본에 평문 메모가 담긴다.
    // 본 DB만 암호화되고, 앱은 백업을 스캔하지도 지우지도 않으므로(CLAUDE.md)
    // 비밀번호 없이 읽히는 파일이 그 옆에 영구히 남는다.
    //
    // 호출 순서를 프론트엔드의 약속으로만 두면 누군가 되돌렸을 때 아무것도
    // 걸리지 않는다. 그래서 여기서 막는다.
    crate::commands::crypto::ensure_migrated(conn)?;

    let path_guard = db_path_state.0.lock().map_err(|e| e.to_string())?;
    let src = path_guard.as_ref().ok_or("DB path not set")?;
    let dest = crate::engine::unique_backup_path(src, "")?;
    crate::commands::crypto::vacuum_into_backup(conn, &dest)
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
/// **이 함수는 백업을 만들지 않는다.** 열 때마다 만드는 백업(`backup_project_impl`)은
/// 프론트엔드가 이 함수 **뒤에** 호출한다. 순서가 그래야 하는 이유는 이렇다.
///
/// 변환 전에 사본을 뜨면 그 사본에는 **평문 메모**가 담긴다. 몇 초 뒤 본 DB는
/// 암호화되는데, 비밀번호 없이 읽히는 사본이 그 옆에 남는 꼴이다. 앱은 백업을
/// 스캔하지도 지우지도 않으므로(CLAUDE.md) 그 파일은 영구히 남는다.
///
/// 변환 전 사본이 막아주는 사고는 "암호화 코드 자체의 버그로 잘못된 암호문이
/// 커밋되는 것"뿐인데, 그건 **직전 열기의 백업**이 이미 커버한다. 전원이 나가거나
/// 강제 종료되는 경우는 마이그레이션의 **각 단계가 원자적**이라 그 단계의 변경이
/// 남지 않는다. (버전 단계마다 커밋하므로 전체가 하나의 트랜잭션인 것은 아니다 —
/// 여러 단계를 건너뛰는 파일은 중간 버전까지 적용된 채로 남을 수 있다.)
/// 확정적인 평문 노출과 맞바꿀 이유가 없다.
pub fn migrate_schema_impl(
    conn: &mut Connection,
    crypto: &CryptoStateHandle,
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
        // "외래키"로 매칭하면 db.rs의 "외래키 설정 복구에도 실패했습니다"까지 걸려,
        // 원인이 다른데 "다시 열어도 같은 결과"라는 잘못된 안내가 나간다.
        let hint = if e.contains("무결성 위반") {
            // 다시 열어도 같은 지점에서 멈춘다. 재시도를 권하면 안 된다.
            "파일 안의 연결 정보가 어긋나 있습니다. 다시 열어도 같은 결과입니다. 이 상태에서는 앱이 백업을 만들 수 없으니, 파일 탐색기로 파일을 복사해 두신 뒤 도움을 요청해주세요."
        } else {
            "디스크 여유 공간을 확인하고, 파일이 다른 프로그램에서 열려 있지 않은지 확인한 뒤 다시 열어주세요."
        };
        return Err(format!(
            "파일 형식 업데이트(v{from} → v{}) 중 오류가 발생해 변경을 취소했습니다. {state} {hint} ({e})",
            crate::db::SCHEMA_VERSION
        ));
    }

    // 변환된 행이 있으면 마이그레이션 트랜잭션이 정리 표시를 남겼다. 옛 페이지에 남은
    // 평문 메모를 지운다. 표시가 없으면 아무것도 하지 않는다.
    //
    // 실패해도 마이그레이션 자체는 이미 커밋됐으므로 오류로 막지 않는다.
    // 조용히 넘기는 것은 아니다 — 표시가 그대로 남아 get_encryption_status가
    // purge_pending으로 알리고, 설정 화면에 "지금 정리" 버튼이 나온다.
    // (open_project_impl의 resume_pending_purge는 migrate보다 먼저 돌기 때문에
    //  이번 열기의 표시를 처리하지 못한다. 그래서 여기서 한 번 더 시도한다.)
    //
    // 실패하면 파일 안에 평문 메모가 남는다. 그 사실은 get_encryption_status의
    // purge_pending으로 화면에 전달된다 — 프론트엔드가 마이그레이션 직후 그것을
    // 다시 읽어 사용자에게 알린다.
    //
    // 참고: 백업이 이 정리에 의존하지는 않는다. VACUUM INTO는 프리 페이지도
    // 페이지 슬랙도 복사하지 않으므로, 정리가 실패한 상태에서 뜬 사본에도 평문은
    // 담기지 않는다(실측 확인). migrate → backup 순서의 근거는 그것이 아니라
    // "변환 전 파일은 살아 있는 행 자체가 평문"이라는 것이다.
    let _ = crate::commands::crypto::resume_pending_purge(conn);

    Ok(())
}

#[tauri::command]
pub fn migrate_schema(
    state: State<DbState>,
    crypto: State<CryptoStateHandle>,
) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_mut().ok_or("DB not open")?;
    migrate_schema_impl(conn, &crypto)
}
