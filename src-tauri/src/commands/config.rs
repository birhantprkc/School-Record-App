use crate::state::DbState;
use rusqlite::{Connection, OptionalExtension};
use std::collections::HashMap;
use tauri::State;

/// 프론트엔드가 읽거나 쓰면 안 되는 설정 키의 접두사.
///
/// APP_CONFIGS에는 암호화 salt와 검증 토큰이 함께 저장된다. 이 값이 새어나가면
/// 오프라인 비밀번호 대입이 가능해지고, 덮어쓰이면 올바른 비밀번호로도 키가 달라져
/// 데이터를 영구히 복구할 수 없다.
///
/// 차단은 커맨드 계층(get_config / get_configs / set_config)에서만 한다.
/// `*_impl` 함수는 crypto 모듈이 salt를 읽고 쓰는 정상 경로이므로 막지 않는다.
pub(crate) const PROTECTED_KEY_PREFIX: &str = "encryption_";

/// 프론트엔드에서 접근할 수 없는 키이면 오류를 반환한다.
pub(crate) fn ensure_not_protected(key: &str) -> Result<(), String> {
    if key.starts_with(PROTECTED_KEY_PREFIX) {
        return Err(format!("접근할 수 없는 설정 키입니다: {key}"));
    }
    Ok(())
}

/// 반환값: None = 버전 동일(모달 표시 안 함), Some(old) = 이전 버전(모달 표시)
/// old가 빈 문자열이면 이전 레코드가 없었음(하위 호환) → 전체 노트 표시
pub fn check_and_update_app_version_impl(conn: &Connection, current_version: &str) -> Result<Option<String>, String> {
    let stored: Option<String> = conn
        .query_row(
            "SELECT config_value FROM APP_CONFIGS WHERE config_key = 'app_version'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if stored.as_deref() == Some(current_version) {
        return Ok(None);
    }

    conn.execute(
        "INSERT OR REPLACE INTO APP_CONFIGS (config_key, config_value) VALUES ('app_version', ?1)",
        [current_version],
    )
    .map_err(|e| e.to_string())?;

    Ok(Some(stored.unwrap_or_default()))
}

pub fn get_config_impl(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    let mut stmt = conn
        .prepare("SELECT config_value FROM APP_CONFIGS WHERE config_key = ?1")
        .map_err(|e| e.to_string())?;

    // .ok()로 뭉개면 "키가 없음"과 "DB 읽기 실패"가 같은 None이 된다.
    // is_encryption_enabled가 이 함수를 쓰므로, 읽기 실패가 None이 되면
    // 암호화된 DB를 평문으로 취급해 평문을 기록하게 된다.
    stmt.query_row([key], |row| row.get::<_, String>(0))
        .optional()
        .map_err(|e| e.to_string())
}

/// 여러 키를 한 번에 조회한다. 저장된 값이 없는 키는 결과 맵에서 빠진다.
/// 전체 행을 반환하지 않고 요청한 키만 돌려주는 이유는, APP_CONFIGS에
/// 암호화 salt/verify token도 함께 저장되므로 프론트로 새어나가면 안 되기 때문이다.
///
/// 다만 이 함수 자체는 요청받은 키를 그대로 돌려준다. 프론트엔드가 salt를
/// 직접 지목해 요청하는 것은 커맨드 계층의 `ensure_not_protected`가 막는다.
pub fn get_configs_impl(
    conn: &Connection,
    keys: &[String],
) -> Result<HashMap<String, String>, String> {
    let mut stmt = conn
        .prepare("SELECT config_value FROM APP_CONFIGS WHERE config_key = ?1")
        .map_err(|e| e.to_string())?;

    let mut map = HashMap::new();
    for key in keys {
        if let Some(value) = stmt
            .query_row([key], |row| row.get::<_, String>(0))
            .optional()
            .map_err(|e| e.to_string())?
        {
            map.insert(key.clone(), value);
        }
    }
    Ok(map)
}

pub fn set_config_impl(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO APP_CONFIGS (config_key, config_value) VALUES (?1, ?2)",
        [key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_config(
    db: State<'_, DbState>,
    key: String,
) -> Result<Option<String>, String> {
    ensure_not_protected(&key)?;
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("DB not open")?;
    get_config_impl(conn, &key)
}

#[tauri::command]
pub async fn get_configs(
    db: State<'_, DbState>,
    keys: Vec<String>,
) -> Result<HashMap<String, String>, String> {
    for key in &keys {
        ensure_not_protected(key)?;
    }
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("DB not open")?;
    get_configs_impl(conn, &keys)
}

#[tauri::command]
pub async fn check_and_update_app_version(
    db: State<'_, DbState>,
    app: tauri::AppHandle,
) -> Result<Option<String>, String> {
    let version = app.package_info().version.to_string();
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("DB not open")?;
    check_and_update_app_version_impl(conn, &version)
}

#[tauri::command]
pub async fn set_config(
    db: State<'_, DbState>,
    key: String,
    value: String,
) -> Result<(), String> {
    ensure_not_protected(&key)?;
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("DB not open")?;
    set_config_impl(conn, &key, &value)
}
