use crate::commands::config::{ensure_not_protected, get_config_impl, get_configs_impl, set_config_impl, check_and_update_app_version_impl};
use super::setup_test_db;

#[test]
fn test_get_config_missing_key_returns_none() {
    let conn = setup_test_db();
    let result = get_config_impl(&conn, "nonexistent_key").unwrap();
    assert!(result.is_none());
}

#[test]
fn test_set_then_get_config() {
    let conn = setup_test_db();
    set_config_impl(&conn, "record_section_cell_text_size", "16").unwrap();
    let result = get_config_impl(&conn, "record_section_cell_text_size").unwrap();
    assert_eq!(result, Some("16".to_string()));
}

#[test]
fn test_set_config_overwrites_existing_value() {
    let conn = setup_test_db();
    set_config_impl(&conn, "record_section_cell_text_size", "14").unwrap();
    set_config_impl(&conn, "record_section_cell_text_size", "20").unwrap();
    let result = get_config_impl(&conn, "record_section_cell_text_size").unwrap();
    assert_eq!(result, Some("20".to_string()));
}

#[test]
fn test_multiple_keys_are_independent() {
    let conn = setup_test_db();
    set_config_impl(&conn, "key_a", "value_a").unwrap();
    set_config_impl(&conn, "key_b", "value_b").unwrap();
    assert_eq!(get_config_impl(&conn, "key_a").unwrap(), Some("value_a".to_string()));
    assert_eq!(get_config_impl(&conn, "key_b").unwrap(), Some("value_b".to_string()));
}

#[test]
fn test_set_config_empty_string_value() {
    let conn = setup_test_db();
    set_config_impl(&conn, "some_key", "").unwrap();
    let result = get_config_impl(&conn, "some_key").unwrap();
    assert_eq!(result, Some("".to_string()));
}

#[test]
fn test_get_config_returns_latest_after_multiple_sets() {
    let conn = setup_test_db();
    for val in ["10", "12", "14", "18", "22"] {
        set_config_impl(&conn, "record_section_cell_text_size", val).unwrap();
    }
    let result = get_config_impl(&conn, "record_section_cell_text_size").unwrap();
    assert_eq!(result, Some("22".to_string()));
}

// ── check_and_update_app_version_impl ───────────────────────────

#[test]
fn test_version_no_record_returns_empty_string() {
    // app_version 레코드가 없는 구버전 DB → Some("") 반환, 전체 노트 표시 신호
    let conn = setup_test_db();
    let result = check_and_update_app_version_impl(&conn, "0.2.12").unwrap();
    assert_eq!(result, Some("".to_string()));
}

#[test]
fn test_version_no_record_writes_current_version() {
    // app_version 레코드가 없을 때 현재 버전이 DB에 저장되어야 한다
    let conn = setup_test_db();
    check_and_update_app_version_impl(&conn, "0.2.12").unwrap();
    let stored = get_config_impl(&conn, "app_version").unwrap();
    assert_eq!(stored, Some("0.2.12".to_string()));
}

#[test]
fn test_version_same_returns_none() {
    // 저장된 버전 == 현재 버전 → None 반환 (모달 표시 안 함)
    let conn = setup_test_db();
    set_config_impl(&conn, "app_version", "0.2.12").unwrap();
    let result = check_and_update_app_version_impl(&conn, "0.2.12").unwrap();
    assert!(result.is_none());
}

#[test]
fn test_version_changed_returns_old_version() {
    // 저장된 버전 != 현재 버전 → Some(이전버전) 반환
    let conn = setup_test_db();
    set_config_impl(&conn, "app_version", "0.2.11").unwrap();
    let result = check_and_update_app_version_impl(&conn, "0.2.12").unwrap();
    assert_eq!(result, Some("0.2.11".to_string()));
}

#[test]
fn test_version_changed_updates_db() {
    // 버전 변경 후 DB의 app_version이 현재 버전으로 갱신되어야 한다
    let conn = setup_test_db();
    set_config_impl(&conn, "app_version", "0.2.11").unwrap();
    check_and_update_app_version_impl(&conn, "0.2.12").unwrap();
    let stored = get_config_impl(&conn, "app_version").unwrap();
    assert_eq!(stored, Some("0.2.12".to_string()));
}

#[test]
fn test_version_same_does_not_modify_db() {
    // 버전 동일 시 DB를 수정하지 않아야 한다 (재확인)
    let conn = setup_test_db();
    set_config_impl(&conn, "app_version", "0.2.12").unwrap();
    check_and_update_app_version_impl(&conn, "0.2.12").unwrap();
    let stored = get_config_impl(&conn, "app_version").unwrap();
    assert_eq!(stored, Some("0.2.12".to_string()));
}

#[test]
fn test_version_idempotent_after_update() {
    // 업데이트 후 동일 버전으로 재호출 시 None 반환 (이중 모달 방지)
    let conn = setup_test_db();
    set_config_impl(&conn, "app_version", "0.2.11").unwrap();
    check_and_update_app_version_impl(&conn, "0.2.12").unwrap();
    let result = check_and_update_app_version_impl(&conn, "0.2.12").unwrap();
    assert!(result.is_none());
}

#[test]
fn test_version_empty_string_current_version() {
    // 현재 버전이 빈 문자열인 비정상 상황에서도 패닉 없이 처리
    let conn = setup_test_db();
    let result = check_and_update_app_version_impl(&conn, "");
    assert!(result.is_ok());
}

#[test]
fn test_version_multiple_upgrades_sequence() {
    // 0.2.10 → 0.2.11 → 0.2.12 순차 업그레이드 시 각 단계에서 올바른 이전 버전 반환
    let conn = setup_test_db();
    set_config_impl(&conn, "app_version", "0.2.10").unwrap();

    let r1 = check_and_update_app_version_impl(&conn, "0.2.11").unwrap();
    assert_eq!(r1, Some("0.2.10".to_string()));

    let r2 = check_and_update_app_version_impl(&conn, "0.2.12").unwrap();
    assert_eq!(r2, Some("0.2.11".to_string()));

    let r3 = check_and_update_app_version_impl(&conn, "0.2.12").unwrap();
    assert!(r3.is_none());
}

#[test]
fn test_version_downgrade_returns_old_and_updates_db() {
    // 다운그레이드(0.2.13 → 0.2.12)에서도 Some(이전버전) 반환 + DB 갱신
    let conn = setup_test_db();
    set_config_impl(&conn, "app_version", "0.2.13").unwrap();
    let result = check_and_update_app_version_impl(&conn, "0.2.12").unwrap();
    assert_eq!(result, Some("0.2.13".to_string()));
    let stored = get_config_impl(&conn, "app_version").unwrap();
    assert_eq!(stored, Some("0.2.12".to_string()));
}

#[test]
fn test_version_empty_string_stored_returns_empty_and_updates_db() {
    // app_version이 빈 문자열로 저장된 경우 → Some("") 반환 + DB 갱신
    let conn = setup_test_db();
    set_config_impl(&conn, "app_version", "").unwrap();
    let result = check_and_update_app_version_impl(&conn, "0.2.13").unwrap();
    assert_eq!(result, Some("".to_string()));
    let stored = get_config_impl(&conn, "app_version").unwrap();
    assert_eq!(stored, Some("0.2.13".to_string()));
}

// ── get_configs_impl ────────────────────────────────────────────

fn keys(list: &[&str]) -> Vec<String> {
    list.iter().map(|k| k.to_string()).collect()
}

#[test]
fn test_get_configs_returns_only_requested_keys() {
    let conn = setup_test_db();
    set_config_impl(&conn, "record_freeze_columns", "1").unwrap();
    set_config_impl(&conn, "record_show_preview", "0").unwrap();
    // 요청하지 않은 키(암호화 salt 등)는 절대 포함되면 안 된다.
    set_config_impl(&conn, "pbkdf2_salt", "secret").unwrap();

    let result = get_configs_impl(&conn, &keys(&["record_freeze_columns", "record_show_preview"])).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result.get("record_freeze_columns"), Some(&"1".to_string()));
    assert_eq!(result.get("record_show_preview"), Some(&"0".to_string()));
    assert!(result.get("pbkdf2_salt").is_none());
}

#[test]
fn test_get_configs_omits_missing_keys() {
    let conn = setup_test_db();
    set_config_impl(&conn, "record_smart_scroll", "0").unwrap();

    let result = get_configs_impl(&conn, &keys(&["record_smart_scroll", "record_compact_cell"])).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result.get("record_smart_scroll"), Some(&"0".to_string()));
    assert!(result.get("record_compact_cell").is_none());
}

#[test]
fn test_get_configs_empty_key_list_returns_empty_map() {
    let conn = setup_test_db();
    set_config_impl(&conn, "record_freeze_columns", "1").unwrap();
    let result = get_configs_impl(&conn, &[]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_get_configs_duplicate_keys_are_deduplicated() {
    let conn = setup_test_db();
    set_config_impl(&conn, "record_highlight_empty", "1").unwrap();
    let result = get_configs_impl(&conn, &keys(&["record_highlight_empty", "record_highlight_empty"])).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result.get("record_highlight_empty"), Some(&"1".to_string()));
}

#[test]
fn test_get_configs_matches_get_config_for_toolbar_keys() {
    let conn = setup_test_db();
    let toolbar = [
        ("record_freeze_columns", "1"),
        ("record_smart_scroll", "0"),
        ("record_compact_cell", "1"),
        ("record_highlight_empty", "0"),
        ("record_show_preview", "1"),
        ("record_collapse_personal_info", "0"),
    ];
    for (k, v) in toolbar {
        set_config_impl(&conn, k, v).unwrap();
    }

    let requested = keys(&toolbar.iter().map(|(k, _)| *k).collect::<Vec<_>>());
    let bulk = get_configs_impl(&conn, &requested).unwrap();
    assert_eq!(bulk.len(), toolbar.len());
    for (k, v) in toolbar {
        assert_eq!(bulk.get(k), Some(&v.to_string()));
        assert_eq!(get_config_impl(&conn, k).unwrap(), Some(v.to_string()));
    }
}

// ── ensure_not_protected ─────────────────────────────────────────

#[test]
fn test_ensure_not_protected_blocks_encryption_keys() {
    for key in [
        "encryption_enabled",
        "encryption_pbkdf2_salt",
        "encryption_verify_token",
    ] {
        assert!(ensure_not_protected(key).is_err(), "{key}는 차단되어야 한다");
    }
}

#[test]
fn test_ensure_not_protected_allows_preference_keys() {
    for key in [
        "app_version",
        "theme_mode",
        "record_freeze_columns",
        "export_c_separator",
    ] {
        assert!(ensure_not_protected(key).is_ok(), "{key}는 허용되어야 한다");
    }
}

#[test]
fn test_ensure_not_protected_error_message_names_key() {
    let err = ensure_not_protected("encryption_pbkdf2_salt").unwrap_err();
    assert!(err.contains("encryption_pbkdf2_salt"));
}

#[test]
fn test_impl_layer_still_reaches_protected_keys() {
    // 차단은 커맨드 계층 전용이다. crypto 모듈이 salt를 읽고 쓰는 정상 경로이므로
    // impl 계층까지 막으면 암호화 자체가 동작하지 않는다.
    let conn = setup_test_db();
    set_config_impl(&conn, "encryption_pbkdf2_salt", "c2FsdA==").unwrap();
    assert_eq!(
        get_config_impl(&conn, "encryption_pbkdf2_salt").unwrap(),
        Some("c2FsdA==".to_string())
    );
}

// ── 키 없음 vs 읽기 실패 ─────────────────────────────────────

#[test]
fn test_get_config_missing_key_is_none_not_error() {
    // "키가 없음"은 정상이고 None이다.
    let conn = setup_test_db();
    assert!(get_config_impl(&conn, "없는_키").unwrap().is_none());
}

/// 행은 있는데 값을 String으로 읽을 수 없는 상태를 만든다.
/// SQLite는 동적 타입이라 TEXT 컬럼에도 BLOB이 들어간다.
/// prepare는 성공하고 query_row만 실패하므로, 정확히 바뀐 경로를 짚는다.
fn insert_unreadable_value(conn: &rusqlite::Connection, key: &str) {
    conn.execute(
        "INSERT INTO APP_CONFIGS (config_key, config_value) VALUES (?1, ?2)",
        rusqlite::params![key, vec![0xFFu8, 0xFE, 0xFD]],
    )
    .unwrap();
}

#[test]
fn test_get_config_read_failure_is_error_not_none() {
    // 읽기 실패를 None으로 뭉개면 is_encryption_enabled가 false가 되어
    // 암호화된 DB를 평문으로 취급하고 평문을 기록하게 된다.
    let conn = setup_test_db();
    insert_unreadable_value(&conn, "encryption_enabled");

    let result = get_config_impl(&conn, "encryption_enabled");
    assert!(result.is_err(), "읽기 실패는 None이 아니라 오류여야 한다");
}

#[test]
fn test_get_configs_read_failure_is_error_not_empty_map() {
    let conn = setup_test_db();
    insert_unreadable_value(&conn, "theme_mode");

    let result = get_configs_impl(&conn, &keys(&["theme_mode"]));
    assert!(result.is_err(), "읽기 실패는 누락이 아니라 오류여야 한다");
}
