use crate::commands::config::{get_config_impl, set_config_impl};
use crate::db::with_transaction;
use crate::crypto::{decrypt, derive_key, encrypt, generate_salt, maybe_decrypt, maybe_encrypt};
use crate::state::{
    clear_crypto_state, current_crypto_key, set_crypto_state, CryptoStateHandle, DbPathState,
    DbState,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use tauri::State;
use zeroize::Zeroizing;

const VERIFY_PLAINTEXT: &str = "school-record-verify";
const KEY_ENCRYPTION_ENABLED: &str = "encryption_enabled";
const KEY_PBKDF2_SALT: &str = "encryption_pbkdf2_salt";
const KEY_VERIFY_TOKEN: &str = "encryption_verify_token";

#[derive(serde::Serialize)]
pub struct EncryptionStatus {
    pub enabled: bool,
    pub unlocked: bool,
}

#[derive(Clone, Copy)]
enum DataTransform {
    Encrypt,
    Decrypt,
}

struct EncryptedColumn {
    table: &'static str,
    column: &'static str,
    skip_empty: bool,
}

const ENCRYPTED_COLUMNS: &[EncryptedColumn] = &[
    EncryptedColumn {
        table: "Student",
        column: "name",
        skip_empty: false,
    },
    EncryptedColumn {
        table: "ActivityRecord",
        column: "content",
        skip_empty: true,
    },
    EncryptedColumn {
        table: "ActivityRecordHistory",
        column: "content",
        skip_empty: true,
    },
];


fn fetch_id_text(conn: &Connection, sql: &str) -> Result<Vec<(i64, String)>, String> {
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

fn select_column_sql(spec: &EncryptedColumn) -> String {
    if spec.skip_empty {
        format!(
            "SELECT id, {} FROM {} WHERE {} != ''",
            spec.column, spec.table, spec.column
        )
    } else {
        format!("SELECT id, {} FROM {}", spec.column, spec.table)
    }
}

fn update_column_sql(spec: &EncryptedColumn) -> String {
    format!("UPDATE {} SET {}=?1 WHERE id=?2", spec.table, spec.column)
}

fn transform_all_data(
    conn: &Connection,
    key: [u8; 32],
    transform: DataTransform,
) -> Result<(), String> {
    for spec in ENCRYPTED_COLUMNS {
        let rows = fetch_id_text(conn, &select_column_sql(spec))?;
        let update_sql = update_column_sql(spec);
        for (id, value) in rows {
            let transformed = match transform {
                DataTransform::Encrypt => maybe_encrypt(&value, Some(key))?,
                DataTransform::Decrypt => maybe_decrypt(value, Some(key))?,
            };
            conn.execute(&update_sql, rusqlite::params![transformed, id])
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub(crate) fn encrypt_all_data(conn: &Connection, key: [u8; 32]) -> Result<(), String> {
    transform_all_data(conn, key, DataTransform::Encrypt)
}

pub(crate) fn decrypt_all_data(conn: &Connection, key: [u8; 32]) -> Result<(), String> {
    transform_all_data(conn, key, DataTransform::Decrypt)
}

pub(crate) fn is_encryption_enabled(conn: &Connection) -> Result<bool, String> {
    Ok(get_config_impl(conn, KEY_ENCRYPTION_ENABLED)?.as_deref() == Some("true"))
}

fn encryption_material(conn: &Connection) -> Result<(Vec<u8>, String), String> {
    let salt_b64 = get_config_impl(conn, KEY_PBKDF2_SALT)?.ok_or("암호화 설정이 없습니다.")?;
    let salt = B64
        .decode(&salt_b64)
        .map_err(|e| format!("salt 디코딩 실패: {e}"))?;
    let token = get_config_impl(conn, KEY_VERIFY_TOKEN)?.ok_or("검증 토큰이 없습니다.")?;
    Ok((salt, token))
}

fn verify_password(
    password: &str,
    salt: &[u8],
    verify_token: &str,
    error_message: &str,
) -> Result<[u8; 32], String> {
    let key = derive_key(password, salt);
    let verified = decrypt(verify_token, &key)
        .map(|s| s == VERIFY_PLAINTEXT)
        .unwrap_or(false);
    if verified {
        Ok(key)
    } else {
        Err(error_message.to_string())
    }
}

pub(crate) fn resolve_data_key(
    conn: &Connection,
    crypto: &CryptoStateHandle,
) -> Result<Option<[u8; 32]>, String> {
    if !is_encryption_enabled(conn)? {
        return Ok(None);
    }

    current_crypto_key(crypto)?
        .map(Some)
        .ok_or_else(|| "암호화가 잠금 상태입니다.".to_string())
}

pub(crate) fn get_encryption_status_impl(
    conn: &Connection,
    crypto: &CryptoStateHandle,
) -> Result<EncryptionStatus, String> {
    let enabled = is_encryption_enabled(conn)?;
    let unlocked = enabled && current_crypto_key(crypto)?.is_some();
    Ok(EncryptionStatus { enabled, unlocked })
}

pub(crate) fn unlock_encryption_impl(
    conn: &Connection,
    crypto: &CryptoStateHandle,
    password: &str,
) -> Result<(), String> {
    let (salt, verify_token) = encryption_material(conn)?;
    let key = verify_password(
        password,
        &salt,
        &verify_token,
        "비밀번호가 올바르지 않습니다.",
    )?;
    set_crypto_state(crypto, key)
}

/// DB 파일을 같은 폴더에 복사하고 그 경로를 반환한다.
///
/// 반환한 경로는 반드시 받아야 한다. `-pre-encrypt`와 `-pre-reencrypt` 백업은
/// 작업이 성공하면 지워야 하기 때문이다. 이유는 각 호출부 주석 참고.
/// 암호화 전후 복구용 백업.
///
/// 실패 시 사용자에게 "이 백업으로 되돌리라"고 안내하는 파일이므로, 열 때 만드는
/// 백업(backup_project_impl)보다 오히려 온전함이 중요하다. fs::copy는 SQLite 락을
/// 거치지 않아 다른 쓰기와 겹치면 반쯤 커밋된 페이지를 담은 파일이 나올 수 있었다.
///
/// **호출부는 반드시 트랜잭션 밖이어야 한다** — VACUUM은 트랜잭션 안에서 실행되지
/// 않는다. enable/disable/change 세 경로 모두 with_transaction 이전에 호출한다.
/// DbState 락은 커맨드 래퍼가 이미 잡고 있으므로(conn을 넘겨받는다) 여기서는
/// DbPathState만 잡아 DbState → DbPathState 순서를 유지한다.
fn backup_db_file(
    conn: &Connection,
    db_path_state: &DbPathState,
    suffix: &str,
) -> Result<PathBuf, String> {
    let guard = db_path_state.0.lock().map_err(|e| e.to_string())?;
    let src = guard.as_ref().ok_or("열린 프로젝트가 없습니다.")?;
    let dest = crate::engine::unique_backup_path(src, suffix)?;
    let dest_str = dest
        .to_str()
        .ok_or("백업 경로를 문자열로 변환하지 못했습니다.")?;
    conn.execute("VACUUM INTO ?1", rusqlite::params![dest_str])
        .map_err(|e| format!("백업 생성 실패: {e}"))?;
    Ok(dest)
}

/// 마무리 작업들을 모두 시도하고 오류를 합친다.
///
/// 하나가 실패했다고 다음을 건너뛰면, 키 설정 실패가 평문 백업을 남기게 된다.
/// 그 상태가 바로 이 마무리 작업들로 없애려던 상황이다.
/// 배열 리터럴로 넘기므로 호출 시점에 모든 작업이 이미 실행된다.
pub(crate) fn combine_all(
    results: impl IntoIterator<Item = Result<(), String>>,
) -> Result<(), String> {
    let errors: Vec<String> = results.into_iter().filter_map(Result::err).collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

/// 암호화 이전 평문이 남아 있는 free page를 실제로 덮어쓴다.
///
/// UPDATE는 행을 제자리에서 바꾸지 않고 옛 페이지를 freelist로 보낸다. 그 페이지에는
/// 암호화 전 평문이 그대로 남아, 사용자가 안전하다고 믿는 .db 파일 안에서 읽힌다.
/// 백업 파일은 지울 수 있지만 이건 파일 안에 있다.
///
/// VACUUM은 DB를 새로 써서 그 잔재를 없앤다. 트랜잭션 안에서는 실행할 수 없으므로
/// 반드시 커밋 이후에 호출한다.
pub(crate) fn purge_free_pages(conn: &Connection, what: &str) -> Result<(), String> {
    conn.execute_batch("VACUUM").map_err(|e| {
        format!(
            "{what}는 완료했지만 이전 데이터 정리(VACUUM)에 실패했습니다. \
             디스크 여유 공간을 확인해주세요. ({e})"
        )
    })
}

/// 작업이 성공한 뒤 백업을 지운다.
///
/// 삭제 실패를 조용히 넘기면 사용자는 백업이 사라진 줄 알지만 실제로는 남아 있게
/// 된다. 그 상태가 바로 이 수정이 없애려는 상황이므로 반드시 오류로 알린다.
fn remove_backup_after_success(path: &Path, what: &str) -> Result<(), String> {
    std::fs::remove_file(path).map_err(|e| {
        format!(
            "{what}는 완료했지만 백업 파일을 삭제하지 못했습니다. 직접 삭제해주세요: {} ({e})",
            path.display()
        )
    })
}

pub(crate) fn enable_encryption_impl(
    conn: &Connection,
    crypto: &CryptoStateHandle,
    db_path_state: &DbPathState,
    password: &str,
) -> Result<(), String> {
    if password.is_empty() {
        return Err("비밀번호를 입력해주세요.".to_string());
    }
    if is_encryption_enabled(conn)? {
        return Err("이미 암호화가 활성화되어 있습니다.".to_string());
    }

    // 암호화 도중 실패하면 되돌릴 수 있도록 평문 상태를 복사해 둔다.
    // 성공하면 반드시 지운다 — 평문 사본이 DB 옆에 남으면 암호화를 켠 의미가 없다.
    let backup = backup_db_file(conn, db_path_state, "-pre-encrypt")?;

    let salt = generate_salt();
    let key = derive_key(password, &salt);
    let salt_b64 = B64.encode(salt);
    let verify_token = encrypt(VERIFY_PLAINTEXT, &key)?;

    with_transaction(conn, || {
        encrypt_all_data(conn, key)?;
        set_config_impl(conn, KEY_PBKDF2_SALT, &salt_b64)?;
        set_config_impl(conn, KEY_VERIFY_TOKEN, &verify_token)?;
        set_config_impl(conn, KEY_ENCRYPTION_ENABLED, "true")?;
        Ok(())
    })
    .map_err(|e| format!("{e}\n복구용 평문 백업이 남아 있습니다: {}", backup.display()))?;

    combine_all([
        set_crypto_state(crypto, key),
        remove_backup_after_success(&backup, "암호화"),
        purge_free_pages(conn, "암호화"),
    ])
}

pub(crate) fn disable_encryption_impl(
    conn: &Connection,
    crypto: &CryptoStateHandle,
    db_path_state: &DbPathState,
) -> Result<(), String> {
    let key = resolve_data_key(conn, crypto)?.ok_or("암호화가 활성화되어 있지 않습니다.")?;

    // 이 백업은 지우지 않는다. 암호문 사본이고, 성공하면 본 DB가 평문이 되므로
    // 백업 쪽이 오히려 덜 위험하다. 실수로 암호화를 끈 경우의 안전망으로 남긴다.
    backup_db_file(conn, db_path_state, "-pre-decrypt")?;

    with_transaction(conn, || {
        decrypt_all_data(conn, key)?;
        conn.execute(
            "DELETE FROM APP_CONFIGS WHERE config_key IN (?1, ?2, ?3)",
            rusqlite::params![KEY_ENCRYPTION_ENABLED, KEY_PBKDF2_SALT, KEY_VERIFY_TOKEN],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })?;

    clear_crypto_state(crypto)
}

pub(crate) fn change_encryption_password_impl(
    conn: &Connection,
    crypto: &CryptoStateHandle,
    db_path_state: &DbPathState,
    old_password: &str,
    new_password: &str,
) -> Result<(), String> {
    if new_password.is_empty() {
        return Err("새 비밀번호를 입력해주세요.".to_string());
    }
    let (salt, verify_token) = encryption_material(conn)?;
    let old_key = verify_password(
        old_password,
        &salt,
        &verify_token,
        "현재 비밀번호가 올바르지 않습니다.",
    )?;

    // 이 백업은 옛 비밀번호로 계속 열린다. 비밀번호가 새어서 바꾼 경우라면
    // 백업을 남겨두는 것이 변경 자체를 무의미하게 만들므로 성공 시 지운다.
    let backup = backup_db_file(conn, db_path_state, "-pre-reencrypt")?;

    let new_salt = generate_salt();
    let new_key = derive_key(new_password, &new_salt);
    let new_salt_b64 = B64.encode(new_salt);
    let new_verify_token = encrypt(VERIFY_PLAINTEXT, &new_key)?;

    with_transaction(conn, || {
        decrypt_all_data(conn, old_key)?;
        encrypt_all_data(conn, new_key)?;
        set_config_impl(conn, KEY_PBKDF2_SALT, &new_salt_b64)?;
        set_config_impl(conn, KEY_VERIFY_TOKEN, &new_verify_token)?;
        Ok(())
    })
    .map_err(|e| format!("{e}\n복구용 백업이 남아 있습니다: {}", backup.display()))?;

    // 옛 키로 암호화된 페이지가 freelist에 남는다. 비밀번호를 바꾼 이유가 유출이라면
    // 그 잔재도 지워야 변경이 의미를 갖는다.
    combine_all([
        set_crypto_state(crypto, new_key),
        remove_backup_after_success(&backup, "비밀번호 변경"),
        purge_free_pages(conn, "비밀번호 변경"),
    ])
}

fn db_conn<'a>(guard: &'a Option<Connection>) -> Result<&'a Connection, String> {
    guard
        .as_ref()
        .ok_or_else(|| "DB가 열려있지 않습니다.".to_string())
}

#[tauri::command]
pub fn get_encryption_status(
    db: State<DbState>,
    crypto: State<CryptoStateHandle>,
) -> Result<EncryptionStatus, String> {
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    get_encryption_status_impl(db_conn(&guard)?, &crypto)
}

#[tauri::command]
pub fn unlock_encryption(
    password: String,
    db: State<DbState>,
    crypto: State<CryptoStateHandle>,
) -> Result<(), String> {
    let password = Zeroizing::new(password);
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    unlock_encryption_impl(db_conn(&guard)?, &crypto, &password)
}

#[tauri::command]
pub fn enable_encryption(
    password: String,
    db: State<DbState>,
    db_path: State<DbPathState>,
    crypto: State<CryptoStateHandle>,
) -> Result<(), String> {
    let password = Zeroizing::new(password);
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    enable_encryption_impl(db_conn(&guard)?, &crypto, &db_path, &password)
}

#[tauri::command]
pub fn disable_encryption(
    db: State<DbState>,
    db_path: State<DbPathState>,
    crypto: State<CryptoStateHandle>,
) -> Result<(), String> {
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    disable_encryption_impl(db_conn(&guard)?, &crypto, &db_path)
}

#[tauri::command]
pub fn change_encryption_password(
    old_password: String,
    new_password: String,
    db: State<DbState>,
    db_path: State<DbPathState>,
    crypto: State<CryptoStateHandle>,
) -> Result<(), String> {
    let old_password = Zeroizing::new(old_password);
    let new_password = Zeroizing::new(new_password);
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    change_encryption_password_impl(db_conn(&guard)?, &crypto, &db_path, &old_password, &new_password)
}
