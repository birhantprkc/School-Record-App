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
/// 커밋은 끝났지만 정리(VACUUM)를 아직 못 끝냈다는 표시.
///
/// 값은 무엇을 하다 남았는지 나타내는 라벨("암호화" / "비밀번호 변경")이고,
/// 다시 시도할 때 그대로 오류 메시지에 쓰인다. `encryption_` 접두사이므로
/// 프론트엔드가 get_config/set_config로 건드릴 수 없다(config.rs 참고).
const KEY_PURGE_PENDING: &str = "encryption_purge_pending";

/// 새로 설정하는 비밀번호의 최소 길이.
///
/// 글자 수(char) 기준이다. UTF-8 바이트로 세면 한글 두 글자가 6바이트라 통과해,
/// 같은 규칙이 언어마다 다르게 적용된다.
const MIN_PASSWORD_LEN: usize = 4;

#[derive(serde::Serialize)]
pub struct EncryptionStatus {
    pub enabled: bool,
    pub unlocked: bool,
    /// 암호화 직후 파일 정리(VACUUM)가 끝나지 않은 상태.
    ///
    /// 파일 안에 이전 데이터의 흔적이 남아 있을 수 있다는 뜻이므로 화면에 알린다.
    /// 파일을 열 때 자동으로 다시 시도하지만, 그 시도까지 실패하면 이 값이 계속
    /// true로 남아 설정 화면에 경고와 재시도 버튼이 표시된다.
    pub purge_pending: bool,
}

#[derive(Clone, Copy)]
enum DataTransform {
    Encrypt,
    Decrypt,
}

pub(crate) struct EncryptedColumn {
    pub(crate) table: &'static str,
    pub(crate) column: &'static str,
    /// `WHERE col != ''`로 빈 값을 건너뛸지 여부.
    ///
    /// **nullable 컬럼은 반드시 true여야 한다.** SQLite에서 `NULL != ''`는 참이 아니라
    /// NULL이라 그 행이 조회에서 빠지는데, `fetch_id_text`는 `row.get::<String>()`을
    /// 하므로 false로 두면 NULL 행에서 타입 변환이 실패한다.
    pub(crate) skip_empty: bool,
    /// 이 컬럼이 암호화 대상이 된 스키마 버전.
    ///
    /// 마이그레이션은 **그 버전에 새로 추가된 컬럼만** 암호화한다
    /// (`encrypt_columns_introduced_in`). 이미 암호문인 컬럼을 다시 돌리면 이중
    /// 암호화가 되고, 그 파일은 한 번의 복호화로 평문이 나오지 않아 복구가 안 된다.
    ///
    /// 여기에 컬럼을 추가하는 것은 **DDL이 그대로여도 스키마 버전 bump 사유다.**
    /// 어느 파일이 이미 변환됐는지 구분할 표식이 user_version뿐이기 때문이다(db.rs 참고).
    ///
    /// **추가하는 경로만 있다.** 제거도 같은 이유로 bump 사유지만, 대응하는 복호화
    /// 훅이 없다. 그냥 빼면 그 컬럼은 암호문인 채로 남는데 읽기 경로에서
    /// `maybe_decrypt`가 빠지고 `decrypt_all_data` 대상에서도 사라져, 값을 되찾을
    /// 방법이 없어진다. 빼야 한다면 `decrypt_columns_removed_in`에 해당하는 훅을
    /// 먼저 만들고 그 버전의 `data_step`에 붙일 것.
    pub(crate) since_version: u32,
}

pub(crate) const ENCRYPTED_COLUMNS: &[EncryptedColumn] = &[
    EncryptedColumn {
        table: "Student",
        column: "name",
        skip_empty: false,
        since_version: 1,
    },
    EncryptedColumn {
        table: "ActivityRecord",
        column: "content",
        skip_empty: true,
        since_version: 1,
    },
    EncryptedColumn {
        table: "ActivityRecordHistory",
        column: "content",
        skip_empty: true,
        since_version: 1,
    },
    // v2에서 추가. 사용자가 직접 쓰는 자유 텍스트라 학생 기록만큼 민감하다.
    //
    // 앱이 자동으로 넣는 고정 문자열("가져오기 전" 등)까지 함께 암호화된다.
    // 그 자체로 숨길 정보는 없지만, 한 컬럼에 평문과 암호문이 섞이는 순간
    // decrypt_all_data가 평문 행에서 실패해 **암호화 해제와 비밀번호 변경이
    // 영구히 막힌다.** 크기보다 컬럼 단위의 균일성이 중요하다.
    EncryptedColumn {
        table: "ActivityRecordHistory",
        column: "note",
        skip_empty: true,
        since_version: 2,
    },
    EncryptedColumn {
        table: "Snapshot",
        column: "memo",
        skip_empty: true,
        since_version: 2,
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

/// 지정한 스키마 버전에서 **새로** 암호화 대상이 된 컬럼만 암호화한다.
///
/// 마이그레이션 전용이다. 호출부(`db::migrate`의 data_step)가 이미 트랜잭션을 열어둔
/// 상태여야 하며, 여기서 BEGIN을 열면 중첩되어 실패한다.
///
/// `since_version`이 더 낮은 컬럼은 건드리지 않는다. 이미 암호문인 값을 다시 돌리면
/// 이중 암호화가 되기 때문이다.
///
/// 그 위에 한 겹 더 둔다 — **이미 이 키로 복호화되는 값은 건너뛴다.** 마이그레이션이
/// 실패해 파일이 이전 버전에 머무르면 그 파일은 닫히지 않고 작업 화면까지 열리므로,
/// 그 사이에 새 표현으로 쓰인 행이 다음 열기에서 한 번 더 변환될 수 있다. GCM 인증
/// 태그가 "이 키로 이미 암호화됨"을 사실상 오탐 없이 가려내므로, 이 검사로 이 단계가
/// 몇 번을 돌아도 같은 결과가 된다.
///
/// 실제로 바꾼 행이 있으면 **같은 트랜잭션 안에** 정리(VACUUM) 표시를 남긴다.
/// UPDATE는 옛 페이지를 freelist로 보내므로 그 자리에 평문 메모가 남는다.
/// 표시가 커밋에 포함되어야 커밋 직후 죽어도 다음에 이어받을 수 있다
/// (`with_purge_marked_transaction` 주석과 같은 이유. 다만 그 헬퍼는 자체 BEGIN을
/// 열기 때문에 여기서는 쓸 수 없고, set_config_impl을 직접 부른다).
///
/// 반환값은 바꾼 행 수다.
pub(crate) fn encrypt_columns_introduced_in(
    conn: &Connection,
    key: Option<[u8; 32]>,
    version: u32,
) -> Result<usize, String> {
    // 암호화를 쓰지 않는 파일은 바꿀 것이 없다. 버전만 오르면 된다.
    let Some(key) = key else {
        return Ok(0);
    };

    let mut changed = 0usize;
    for spec in ENCRYPTED_COLUMNS
        .iter()
        .filter(|c| c.since_version == version)
    {
        let rows = fetch_id_text(conn, &select_column_sql(spec))?;
        let update_sql = update_column_sql(spec);
        for (id, value) in rows {
            if decrypt(&value, &key).is_ok() {
                continue;
            }
            // maybe_encrypt를 쓴다. 빈 문자열을 그대로 두는 것이 enable/disable
            // 경로와 같은 표현이다. raw encrypt를 쓰면 skip_empty=false인 컬럼에서
            // 두 경로가 빈 값을 다르게 저장한다.
            let encrypted = maybe_encrypt(&value, Some(key))?;
            conn.execute(&update_sql, rusqlite::params![encrypted, id])
                .map_err(|e| e.to_string())?;
            changed += 1;
        }
    }

    if changed > 0 {
        set_config_impl(conn, KEY_PURGE_PENDING, "메모 암호화")?;
    }
    Ok(changed)
}

/// 새로 설정하는 비밀번호만 검사한다.
///
/// **잠금 해제(unlock)에는 절대 적용하지 않는다.** 이 하한이 생기기 전에 3자 이하로
/// 암호화한 파일이 이미 사용자 PC에 있을 수 있고, 여기서 막으면 올바른 비밀번호를
/// 알고 있는데도 자기 파일을 영영 열 수 없게 된다.
fn validate_new_password(password: &str) -> Result<(), String> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(format!(
            "비밀번호는 최소 {MIN_PASSWORD_LEN}자 이상이어야 합니다."
        ));
    }
    Ok(())
}

/// 이 파일이 암호화를 쓰는가.
///
/// APP_CONFIGS가 아예 없는 파일(v0)은 "쓰지 않음"이다. 암호화 기능이 생기기 전에
/// 만들어진 파일이라 설정을 저장한 적이 없다.
///
/// **여기서 오류를 내면 그 파일은 열리지도 않는다.** 프론트엔드는 잠금 해제 여부를
/// 정하려고 마이그레이션보다 **먼저** 이 값을 읽는다(그래야 키를 쥔 채로 변환할 수
/// 있다). 그래서 `migrate_schema_impl`이 `has_app_configs`로 v0 파일을 정식 지원해도,
/// 이 함수가 `no such table`로 먼저 실패하면 그 지원에 닿지 못한다.
pub(crate) fn is_encryption_enabled(conn: &Connection) -> Result<bool, String> {
    if !crate::commands::config::has_app_configs(conn)? {
        return Ok(false);
    }
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

/// 버전 검사를 하지 않고 키만 확인한다. **마이그레이션 전용.**
///
/// 마이그레이션 자체는 파일이 아직 옛 버전일 때 도는 것이 정상이므로, 아래
/// `resolve_data_key`의 가드를 그대로 쓰면 자기 자신이 막힌다.
pub(crate) fn resolve_data_key_unchecked(
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

/// 마이그레이션이 끝난 파일인지 확인한다.
///
/// 마이그레이션이 실패해도 파일은 닫히지 않는다. 그대로 두면 옛 버전 파일에 새 표현으로
/// 데이터를 쓰게 되고, 다음에 열 때 마이그레이션이 그 행을 한 번 더 변환한다.
/// `encrypt_columns_introduced_in`의 멱등 검사가 그것까지 막지만, 애초에 그런 파일이
/// 만들어지지 않게 여기서 먼저 끊는다.
///
/// 암호화 경로 셋(켜기·끄기·비밀번호 변경)과 데이터 키 조회에 모두 적용한다. 하나만
/// 열어두면 그 경로로 정확히 위 상태가 만들어진다.
pub(crate) fn ensure_migrated(conn: &Connection) -> Result<(), String> {
    let version: u32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if version != crate::db::SCHEMA_VERSION {
        return Err(format!(
            "파일 형식 업데이트가 끝나지 않았습니다 (파일 v{version}, 현재 v{}). 파일을 닫았다가 다시 열어주세요.",
            crate::db::SCHEMA_VERSION
        ));
    }
    Ok(())
}

/// 데이터를 읽거나 쓰기 전에 부르는 정상 경로.
///
/// **마이그레이션이 끝나지 않은 파일에서는 거부한다.** 마이그레이션이 실패해도 파일은
/// 닫히지 않는다 — 프론트엔드가 파일을 연 시점에 이미 열린 상태로 표시하고, 작업 화면
/// 진입도 그 상태만 본다. 그대로 두면 옛 버전 파일에 새 표현으로 데이터를 쓰게 되고,
/// 다음에 열 때 마이그레이션이 그 행을 한 번 더 변환한다.
/// `encrypt_columns_introduced_in`의 멱등 검사가 그것까지 막지만, 애초에 그런 파일이
/// 만들어지지 않게 여기서 먼저 끊는다.
pub(crate) fn resolve_data_key(
    conn: &Connection,
    crypto: &CryptoStateHandle,
) -> Result<Option<[u8; 32]>, String> {
    ensure_migrated(conn)?;
    resolve_data_key_unchecked(conn, crypto)
}

pub(crate) fn get_encryption_status_impl(
    conn: &Connection,
    crypto: &CryptoStateHandle,
) -> Result<EncryptionStatus, String> {
    let enabled = is_encryption_enabled(conn)?;
    let unlocked = enabled && current_crypto_key(crypto)?.is_some();
    Ok(EncryptionStatus {
        enabled,
        unlocked,
        purge_pending: is_purge_pending(conn)?,
    })
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

/// 암호화 전후 복구용 백업. 만든 경로를 반환한다.
///
/// 반환한 경로는 반드시 받아야 한다. `-pre-encrypt`와 `-pre-reencrypt` 백업은
/// 작업이 성공하면 지워야 하기 때문이다. 이유는 각 호출부 주석 참고.
///
/// 실패 시 사용자에게 "이 백업으로 되돌리라"고 안내하는 파일이므로, 열 때 만드는
/// 백업(backup_project_impl)보다 오히려 온전함이 중요하다. fs::copy는 SQLite 락을
/// 거치지 않아 다른 쓰기와 겹치면 반쯤 커밋된 페이지를 담은 파일이 나올 수 있었다.
///
/// **호출부는 반드시 트랜잭션 밖이어야 한다** — VACUUM은 트랜잭션 안에서 실행되지
/// 않는다. enable/disable/change 세 경로 모두 트랜잭션을 열기 전에 호출한다.
/// DbState 락은 커맨드 래퍼가 이미 잡고 있으므로(conn을 넘겨받는다) 여기서는
/// DbPathState만 잡아 DbState → DbPathState 순서를 유지한다.
/// `VACUUM INTO`로 사본을 만든다. **완성 전 이름(.part)으로 쓰고 성공해야 옮긴다.**
///
/// 바로 최종 이름으로 쓰면, 도중에 프로세스가 죽었을 때 크기만 작을 뿐 이름도
/// 확장자도 정상 백업과 똑같은 파일이 남는다. 앱은 백업을 스캔하지도 지우지도
/// 않으므로(CLAUDE.md) 그 파일은 영원히 남고, 나중에 복구하려고 열면 핫저널
/// 롤백으로 0바이트가 된다(`integrity_check`는 그래도 ok를 돌려준다).
///
/// 실패 시 지우는 것도 **자기가 만든 .part뿐**이다. 최종 이름을 지우면 같은 초에
/// 다른 인스턴스가 만든 정상 백업을 지울 수 있다. 그래서 임시 이름에 프로세스 번호를
/// 넣는다 — 넣지 않으면 같은 초에 같은 파일을 연 두 인스턴스가 같은 `.part`를
/// 노리고, 한쪽의 실패 정리가 **다른 쪽이 쓰는 중인 파일을 지운다.**
///
/// 다만 최종 이름으로의 `rename`은 대상이 있으면 덮어쓴다(Windows의 `MoveFileEx`
/// 동작이다). `unique_backup_path`가 없는 이름을 골라 주므로 남의 백업을 덮는 일은
/// 사실상 없고, 설령 같은 초에 겹치더라도 두 사본의 원본이 같은 DB라 내용이 같다.
pub(crate) fn vacuum_into_backup(conn: &Connection, dest: &Path) -> Result<(), String> {
    let mut part = dest.as_os_str().to_os_string();
    part.push(format!(".{}.part", std::process::id()));
    let part = PathBuf::from(part);
    let part_str = part
        .to_str()
        .ok_or("백업 경로를 문자열로 변환하지 못했습니다.")?;

    // 경로를 SQL에 직접 넣지 않고 바인딩한다 — 한글·역슬래시 이스케이프 문제를 피한다.
    if let Err(e) = conn.execute("VACUUM INTO ?1", rusqlite::params![part_str]) {
        std::fs::remove_file(&part).ok();
        return Err(format!("백업 생성 실패: {e}"));
    }
    std::fs::rename(&part, dest).map_err(|e| {
        std::fs::remove_file(&part).ok();
        format!("백업 파일 이름을 바꾸지 못했습니다: {e}")
    })
}

pub(crate) fn backup_db_file(
    conn: &Connection,
    db_path_state: &DbPathState,
    suffix: &str,
) -> Result<PathBuf, String> {
    let guard = db_path_state.0.lock().map_err(|e| e.to_string())?;
    let src = guard.as_ref().ok_or("열린 프로젝트가 없습니다.")?;
    let dest = crate::engine::unique_backup_path(src, suffix)?;
    vacuum_into_backup(conn, &dest)?;
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

/// 데이터 변경과 정리 표시를 **한 커밋으로 묶는다.**
///
/// 표시가 같은 커밋에 들어가야 커밋 직후 프로세스가 죽어도 표시가 파일에 남아,
/// 다음에 열 때 이어받을 수 있다. 표시를 트랜잭션 밖에서 남기면 커밋과 표시
/// 사이에 죽었을 때 잔재만 남고 표시는 없어, 다시 시도할 근거가 사라진다.
///
/// 그 창은 테스트로 잡을 수 없다 — 두 문장 사이에서 프로세스를 죽여야 보이기
/// 때문이다. 그래서 표시를 이 함수 안에 가둔다. 호출부는 표시를 남길지 고를 수
/// 없으므로, 실수로 트랜잭션 밖으로 옮기는 회귀 자체가 생기지 않는다.
pub(crate) fn with_purge_marked_transaction(
    conn: &Connection,
    what: &str,
    action: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    with_transaction(conn, || {
        action()?;
        set_config_impl(conn, KEY_PURGE_PENDING, what)
    })
}

/// 정리가 밀려 있는지 확인한다. 화면에 알리기 위한 조회다.
pub(crate) fn is_purge_pending(conn: &Connection) -> Result<bool, String> {
    // APP_CONFIGS가 없는 파일(v0)에는 표시를 남긴 적도 없다. `is_encryption_enabled`와
    // 같은 이유로, 여기서 오류를 내면 get_encryption_status가 실패해 그 파일이
    // 열리지 않는다.
    if !crate::commands::config::has_app_configs(conn)? {
        return Ok(false);
    }
    Ok(get_config_impl(conn, KEY_PURGE_PENDING)?.is_some())
}

/// VACUUM을 실행하고, **성공했을 때만** 표시를 지운다.
///
/// 순서를 뒤집어 표시를 먼저 지우면 VACUUM이 실패했을 때 재시도할 근거가 사라진다.
/// 실패하면 표시가 그대로 남아 다음에 파일을 열 때 이어서 시도된다.
fn purge_and_clear_pending(conn: &Connection, what: &str) -> Result<(), String> {
    purge_free_pages(conn, what)?;
    conn.execute(
        "DELETE FROM APP_CONFIGS WHERE config_key = ?1",
        rusqlite::params![KEY_PURGE_PENDING],
    )
    .map_err(|e| format!("{what} 정리는 끝났지만 완료 표시를 지우지 못했습니다: {e}"))?;
    Ok(())
}

/// 지난번에 끝내지 못한 정리를 이어서 실행한다. 표시가 없으면 아무것도 하지 않는다.
///
/// 암호화를 켜거나 비밀번호를 바꾸면 옛 평문·옛 암호문이 freelist에 남고, 그것을
/// 지우는 VACUUM은 커밋 **이후**에 실행된다. 그 사이에 프로세스가 죽으면 잔재가
/// 파일에 남은 채 끝나고, 예전에는 앱 안에 다시 시도할 방법이 없었다.
/// 표시는 커밋에 포함돼 있으므로 여기서 이어받는다.
pub(crate) fn resume_pending_purge(conn: &Connection) -> Result<(), String> {
    let Some(what) = get_config_impl(conn, KEY_PURGE_PENDING)? else {
        return Ok(());
    };
    purge_and_clear_pending(conn, &what)
}

/// 사용자가 설정 화면에서 직접 누르는 재시도.
///
/// 열 때의 자동 재시도가 실패한 뒤(디스크 공간 부족 등) 원인을 해결했을 때,
/// 파일을 닫았다 다시 열지 않고도 정리할 수 있게 한다.
pub(crate) fn retry_pending_purge_impl(conn: &Connection) -> Result<(), String> {
    if !is_purge_pending(conn)? {
        return Err("정리할 항목이 없습니다.".to_string());
    }
    resume_pending_purge(conn)
}

/// 작업이 성공한 뒤 백업을 지운다.
///
/// 삭제 실패를 조용히 넘기면 사용자는 백업이 사라진 줄 알지만 실제로는 남아 있게
/// 된다. 그 상태가 바로 이 수정이 없애려는 상황이므로 반드시 오류로 알린다.
pub(crate) fn remove_backup_after_success(path: &Path, what: &str) -> Result<(), String> {
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
    ensure_migrated(conn)?;
    validate_new_password(password)?;
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

    with_purge_marked_transaction(conn, "암호화", || {
        encrypt_all_data(conn, key)?;
        set_config_impl(conn, KEY_PBKDF2_SALT, &salt_b64)?;
        set_config_impl(conn, KEY_VERIFY_TOKEN, &verify_token)?;
        set_config_impl(conn, KEY_ENCRYPTION_ENABLED, "true")
    })
    .map_err(|e| format!("{e}\n복구용 평문 백업이 남아 있습니다: {}", backup.display()))?;

    combine_all([
        set_crypto_state(crypto, key),
        remove_backup_after_success(&backup, "암호화"),
        purge_and_clear_pending(conn, "암호화"),
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
        // KEY_PURGE_PENDING은 일부러 지우지 않는다. 암호화를 켜다 만 상태에서
        // 해제한 경우 파일에는 아직 정리하지 못한 잔재가 있을 수 있고, 표시를
        // 남겨두면 다음에 열 때 정리된다. 남겨서 손해 보는 것은 VACUUM 한 번뿐이다.
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
    ensure_migrated(conn)?;
    validate_new_password(new_password)?;
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

    with_purge_marked_transaction(conn, "비밀번호 변경", || {
        decrypt_all_data(conn, old_key)?;
        encrypt_all_data(conn, new_key)?;
        set_config_impl(conn, KEY_PBKDF2_SALT, &new_salt_b64)?;
        set_config_impl(conn, KEY_VERIFY_TOKEN, &new_verify_token)
    })
    .map_err(|e| format!("{e}\n복구용 백업이 남아 있습니다: {}", backup.display()))?;

    // 옛 키로 암호화된 페이지가 freelist에 남는다. 비밀번호를 바꾼 이유가 유출이라면
    // 그 잔재도 지워야 변경이 의미를 갖는다.
    combine_all([
        set_crypto_state(crypto, new_key),
        remove_backup_after_success(&backup, "비밀번호 변경"),
        purge_and_clear_pending(conn, "비밀번호 변경"),
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

/// 정리를 지금 다시 시도한다. 설정 화면의 "지금 정리" 버튼이 호출한다.
#[tauri::command]
pub fn retry_encryption_purge(db: State<DbState>) -> Result<(), String> {
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    retry_pending_purge_impl(db_conn(&guard)?)
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
