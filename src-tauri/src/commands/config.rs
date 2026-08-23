use crate::state::DbState;
use rusqlite::Connection;
use std::collections::HashMap;
use tauri::State;

/// 반환값: None = 버전 동일(모달 표시 안 함), Some(old) = 이전 버전(모달 표시)
/// old가 빈 문자열이면 이전 레코드가 없었음(하위 호환) → 전체 노트 표시
pub fn check_and_update_app_version_impl(conn: &Connection, current_version: &str) -> Result<Option<String>, String> {
    let stored: Option<String> = conn
        .query_row(
            "SELECT config_value FROM APP_CONFIGS WHERE config_key = 'app_version'",
            [],
            |row| row.get(0),
        )
        .ok();

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

    Ok(stmt.query_row([key], |row| row.get::<_, String>(0)).ok())
}

/// 여러 키를 한 번에 조회한다. 저장된 값이 없는 키는 결과 맵에서 빠진다.
/// 전체 행을 반환하지 않고 요청한 키만 돌려주는 이유는, APP_CONFIGS에
/// 암호화 salt/verify token도 함께 저장되므로 프론트로 새어나가면 안 되기 때문이다.
pub fn get_configs_impl(
    conn: &Connection,
    keys: &[String],
) -> Result<HashMap<String, String>, String> {
    let mut stmt = conn
        .prepare("SELECT config_value FROM APP_CONFIGS WHERE config_key = ?1")
        .map_err(|e| e.to_string())?;

    let mut map = HashMap::new();
    for key in keys {
        if let Ok(value) = stmt.query_row([key], |row| row.get::<_, String>(0)) {
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
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("DB not open")?;
    get_config_impl(conn, &key)
}

#[tauri::command]
pub async fn get_configs(
    db: State<'_, DbState>,
    keys: Vec<String>,
) -> Result<HashMap<String, String>, String> {
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
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let conn = guard.as_ref().ok_or("DB not open")?;
    set_config_impl(conn, &key, &value)
}
