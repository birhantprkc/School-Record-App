//! 고정 암호화 벡터.
//!
//! 이 파일의 값들은 **배포된 사용자 DB에 실제로 들어 있는 형식**이다. 지금 코드로
//! 생성해 박아 둔 것이며, 앞으로 어떤 라이브러리를 올리든 이 값들이 그대로 풀려야
//! 기존 사용자의 암호화 파일이 열린다.
//!
//! 나머지 암호화 테스트는 전부 round-trip이다 — 같은 빌드로 암호화하고 같은 빌드로
//! 복호화한다. 그래서 라이브러리가 저장 형식(nonce 길이, 태그 위치, KDF 출력)을
//! 바꿔도 전부 통과하고, 깨지는 것은 이미 암호화를 켜 둔 사용자의 DB뿐이다.
//! 이 파일이 그 구멍을 막는다.
//!
//! **이 테스트가 실패하면 값을 다시 생성하지 말 것.** 실패는 저장 형식이 바뀌었다는
//! 뜻이고, 그대로 배포하면 기존 사용자가 자기 데이터를 영영 열 수 없게 된다.
//! `schema_lock_tests.rs`의 지문과 같은 성격이다.

use super::{insert_activity, setup_test_db};
use crate::commands::config::set_config_impl;
use crate::commands::crypto::{
    get_encryption_status_impl, resolve_data_key, unlock_encryption_impl,
};
use crate::commands::student::get_students_impl;
use crate::crypto::{decrypt, derive_key, encrypt, generate_salt, maybe_decrypt};
use crate::state::{CryptoState, CryptoStateHandle};
use base64::{engine::general_purpose::STANDARD as B64, Engine};

// ── 벡터 (현재 코드로 생성, 변경 금지) ────────────────────────

const PASSWORD: &str = "교사비밀번호123!";
const SALT: [u8; 16] = *b"SchoolRecordSalt";
const SALT_B64: &str = "U2Nob29sUmVjb3JkU2FsdA==";

/// PBKDF2-HMAC-SHA256, 600,000회, 32바이트. 반복 횟수가 바뀌면 이 값이 달라진다.
const KEY_HEX: &str = "dd8e32eac8f1a058e32b6512d3a4948e92cfb35cfbb63f30834292ff40ce2929";

const VERIFY_TOKEN: &str = "rUe5asL8cMdOi5pK:F0GhxnfZz+J9Wt6DfRiwrtft7XrultnOVJXoeX/UaYSWyVav";
const ENC_NAME: &str = "RWN4J//Algg0li2f:szyvP+MfJC8GoItIiaRCmio9QMII7cZyeQ==";
const ENC_CONTENT: &str = "NHkFZtC2mpjUypeK:EgaLyGKsbqYH82P7vIUHx66YlLrSK4oeVyH1NeJfjBNXP0dN4BobbG9BZutkZ9Bd4vXM+sgvwTUskko5Nb8Ydk7we27TcgGQVNJO0tVgB4QjlA==";
const ENC_SPACE: &str = "y8D3oU74hWl5P1jA:y9fe9yR52HBMtKQ4N2SmIX0=";
const ENC_MULTILINE: &str = "vrPCYVJsWb9UJcmR:jhNBinA1FKt/GmIzeBfQTF2aPYgT7so9dRMR8YnDhq1Jkg==";

const PLAIN_NAME: &str = "홍길동";
const PLAIN_CONTENT: &str = "수업에 적극적으로 참여하며 친구들을 잘 도왔음.";
const PLAIN_MULTILINE: &str = "첫 줄\n둘째 줄";
const VERIFY_PLAINTEXT: &str = "school-record-verify";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn fixed_key() -> [u8; 32] {
    derive_key(PASSWORD, &SALT)
}

// ── derive_key: 키 파생 파라미터 고정 ─────────────────────────

#[test]
fn test_derive_key_matches_fixed_vector() {
    // 반복 횟수·해시·출력 길이 중 하나라도 바뀌면 기존 비밀번호로 만든 키가 달라져
    // 사용자 데이터가 전부 열리지 않는다.
    assert_eq!(hex(&fixed_key()), KEY_HEX);
}

#[test]
fn test_derive_key_is_deterministic_across_calls() {
    assert_eq!(derive_key(PASSWORD, &SALT), derive_key(PASSWORD, &SALT));
}

#[test]
fn test_derive_key_differs_for_other_password_and_salt() {
    assert_ne!(hex(&derive_key("다른비밀번호", &SALT)), KEY_HEX);
    assert_ne!(hex(&derive_key(PASSWORD, &[0u8; 16])), KEY_HEX);
}

// ── decrypt: 저장 형식 고정 ───────────────────────────────────

#[test]
fn test_decrypt_fixed_name() {
    assert_eq!(decrypt(ENC_NAME, &fixed_key()).unwrap(), PLAIN_NAME);
}

#[test]
fn test_decrypt_fixed_content() {
    assert_eq!(decrypt(ENC_CONTENT, &fixed_key()).unwrap(), PLAIN_CONTENT);
}

#[test]
fn test_decrypt_fixed_multiline_preserves_newline() {
    // 줄바꿈이 보존되지 않으면 글자 수 계산과 내보내기 결과가 달라진다.
    assert_eq!(
        decrypt(ENC_MULTILINE, &fixed_key()).unwrap(),
        PLAIN_MULTILINE
    );
}

#[test]
fn test_decrypt_fixed_single_space() {
    // 공백 한 칸은 빈 문자열이 아니므로 암호화 대상이다.
    assert_eq!(decrypt(ENC_SPACE, &fixed_key()).unwrap(), " ");
}

#[test]
fn test_decrypt_fixed_verify_token() {
    assert_eq!(
        decrypt(VERIFY_TOKEN, &fixed_key()).unwrap(),
        VERIFY_PLAINTEXT
    );
}

#[test]
fn test_maybe_decrypt_fixed_vector() {
    let out = maybe_decrypt(ENC_NAME.to_string(), Some(fixed_key())).unwrap();
    assert_eq!(out, PLAIN_NAME);
}

#[test]
fn test_fixed_vectors_reject_wrong_key() {
    let wrong = derive_key("틀린비밀번호", &SALT);
    for v in [VERIFY_TOKEN, ENC_NAME, ENC_CONTENT] {
        assert!(decrypt(v, &wrong).is_err(), "틀린 키로 풀리면 안 된다: {v}");
    }
}

// ── 저장 형식 불변식 ──────────────────────────────────────────

#[test]
fn test_stored_format_is_nonce_colon_ciphertext() {
    for v in [
        VERIFY_TOKEN,
        ENC_NAME,
        ENC_CONTENT,
        ENC_SPACE,
        ENC_MULTILINE,
    ] {
        let (nonce_b64, cipher_b64) = v.split_once(':').expect("구분자가 있어야 한다");
        assert_eq!(
            B64.decode(nonce_b64).unwrap().len(),
            12,
            "nonce는 96비트여야 한다: {v}"
        );
        assert!(!B64.decode(cipher_b64).unwrap().is_empty());
    }
}

#[test]
fn test_new_encryption_uses_same_format_as_fixed_vectors() {
    // 지금 만드는 값도 고정 벡터와 같은 형식이어야 한다. nonce는 매번 달라야 한다.
    let a = encrypt(PLAIN_NAME, &fixed_key()).unwrap();
    let b = encrypt(PLAIN_NAME, &fixed_key()).unwrap();
    assert_ne!(a, b, "같은 평문이라도 nonce가 달라 결과가 달라야 한다");

    let fixed_nonce_len = ENC_NAME.split_once(':').unwrap().0.len();
    for v in [a, b] {
        let (nonce_b64, _) = v.split_once(':').unwrap();
        assert_eq!(nonce_b64.len(), fixed_nonce_len);
        assert_eq!(decrypt(&v, &fixed_key()).unwrap(), PLAIN_NAME);
    }
}

#[test]
fn test_ciphertext_carries_16_byte_tag() {
    // AES-GCM 태그가 빠지면 위변조를 잡지 못한다.
    let (_, cipher_b64) = ENC_NAME.split_once(':').unwrap();
    let len = B64.decode(cipher_b64).unwrap().len();
    assert_eq!(len, PLAIN_NAME.len() + 16);
}

#[test]
fn test_tampered_ciphertext_is_rejected() {
    let (nonce, cipher) = ENC_NAME.split_once(':').unwrap();
    let mut bytes = cipher.as_bytes().to_vec();
    bytes[0] = if bytes[0] == b'A' { b'B' } else { b'A' };
    let tampered = format!("{nonce}:{}", String::from_utf8(bytes).unwrap());
    assert!(
        decrypt(&tampered, &fixed_key()).is_err(),
        "변조를 잡아야 한다"
    );
}

// ── generate_salt: 고정할 수 없는 값의 계약 ───────────────────

#[test]
fn test_generate_salt_length_and_randomness() {
    let a = generate_salt();
    let b = generate_salt();
    assert_eq!(
        a.len(),
        SALT.len(),
        "salt 길이가 바뀌면 기존 설정을 읽을 수 없다"
    );
    assert_ne!(a, b, "salt가 매번 같으면 안 된다");
}

// ── 배포된 사용자 DB 재현: 잠금 해제부터 읽기까지 ─────────────

#[test]
fn test_existing_encrypted_database_still_opens() {
    // 고정 벡터를 그대로 넣어 '이미 암호화를 켜 둔 사용자의 DB'를 만든 뒤,
    // 평소 경로(잠금 해제 → 조회)로 읽힌다는 것을 확인한다.
    let conn = setup_test_db();
    let crypto: CryptoStateHandle = std::sync::Mutex::new(CryptoState { key: None });

    set_config_impl(&conn, "encryption_enabled", "true").unwrap();
    set_config_impl(&conn, "encryption_pbkdf2_salt", SALT_B64).unwrap();
    set_config_impl(&conn, "encryption_verify_token", VERIFY_TOKEN).unwrap();

    let act_id = insert_activity(&conn, "발표");
    conn.execute(
        "INSERT INTO Student (grade, class_num, number, name) VALUES (1, 1, 1, ?1)",
        rusqlite::params![ENC_NAME],
    )
    .unwrap();
    let stu_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO ActivityRecord (activity_id, student_id, content) VALUES (?1, ?2, ?3)",
        rusqlite::params![act_id, stu_id, ENC_CONTENT],
    )
    .unwrap();

    // 잠금 상태에서는 읽을 수 없어야 한다.
    assert!(resolve_data_key(&conn, &crypto).is_err());

    unlock_encryption_impl(&conn, &crypto, PASSWORD).unwrap();
    let status = get_encryption_status_impl(&conn, &crypto).unwrap();
    assert!(status.enabled && status.unlocked);

    let key = resolve_data_key(&conn, &crypto).unwrap();
    let students = get_students_impl(&conn, key).unwrap();
    assert_eq!(students[0].name, PLAIN_NAME);

    let raw: String = conn
        .query_row("SELECT content FROM ActivityRecord", [], |r| r.get(0))
        .unwrap();
    assert_eq!(maybe_decrypt(raw, key).unwrap(), PLAIN_CONTENT);
}

#[test]
fn test_existing_database_rejects_wrong_password() {
    let conn = setup_test_db();
    let crypto: CryptoStateHandle = std::sync::Mutex::new(CryptoState { key: None });
    set_config_impl(&conn, "encryption_enabled", "true").unwrap();
    set_config_impl(&conn, "encryption_pbkdf2_salt", SALT_B64).unwrap();
    set_config_impl(&conn, "encryption_verify_token", VERIFY_TOKEN).unwrap();

    let err = unlock_encryption_impl(&conn, &crypto, "틀린비밀번호").unwrap_err();
    assert!(err.contains("비밀번호"), "에러 메시지: {err}");
}
