use crate::commands::area::{
    create_area_impl, delete_area_impl, get_areas_impl, update_area_impl, validate_byte_limit,
};
use super::{insert_activity, insert_area, insert_student, setup_test_db};

#[test]
fn test_create_area_returns_id() {
    let conn = setup_test_db();
    let id = create_area_impl(&conn, "국어", 500).unwrap();
    assert!(id > 0);
}

#[test]
fn test_create_area_duplicate_name_error() {
    let conn = setup_test_db();
    create_area_impl(&conn, "국어", 500).unwrap();
    let err = create_area_impl(&conn, "국어", 500).unwrap_err();
    assert!(err.contains("이미 같은 이름의 영역"), "에러 메시지: {err}");
}

#[test]
fn test_get_areas_empty_db() {
    let conn = setup_test_db();
    let areas = get_areas_impl(&conn).unwrap();
    assert!(areas.is_empty());
}

#[test]
fn test_get_areas_single_no_activities() {
    let conn = setup_test_db();
    insert_area(&conn, "수학", 400);
    let areas = get_areas_impl(&conn).unwrap();
    assert_eq!(areas.len(), 1);
    assert_eq!(areas[0].name, "수학");
    assert!(areas[0].activities.is_empty());
}

#[test]
fn test_get_areas_with_activities() {
    let conn = setup_test_db();
    let area_id = insert_area(&conn, "과학", 600);
    let act_id = insert_activity(&conn, "실험보고서");

    conn.execute(
        "INSERT INTO AreaActivity (area_id, activity_id) VALUES (?1, ?2)",
        rusqlite::params![area_id, act_id],
    )
    .unwrap();

    let areas = get_areas_impl(&conn).unwrap();
    assert_eq!(areas.len(), 1);
    assert_eq!(areas[0].activities.len(), 1);
    assert_eq!(areas[0].activities[0].name, "실험보고서");
}

#[test]
fn test_update_area_name_and_limit() {
    let conn = setup_test_db();
    let id = create_area_impl(&conn, "영어", 300).unwrap();
    update_area_impl(&conn, id, "영어(개정)", 600).unwrap();

    let areas = get_areas_impl(&conn).unwrap();
    assert_eq!(areas[0].name, "영어(개정)");
    assert_eq!(areas[0].byte_limit, 600);
}

#[test]
fn test_update_area_duplicate_name_error() {
    let conn = setup_test_db();
    let id1 = create_area_impl(&conn, "체육", 200).unwrap();
    let id2 = create_area_impl(&conn, "음악", 200).unwrap();
    let _ = id1;
    let err = update_area_impl(&conn, id2, "체육", 200).unwrap_err();
    assert!(err.contains("이미 같은 이름의 영역"), "에러 메시지: {err}");
}

#[test]
fn test_delete_area_removes_row() {
    let conn = setup_test_db();
    let id = create_area_impl(&conn, "미술", 250).unwrap();
    delete_area_impl(&conn, id).unwrap();

    let areas = get_areas_impl(&conn).unwrap();
    assert!(areas.is_empty());
}

#[test]
fn test_delete_area_cascades_area_activity() {
    let conn = setup_test_db();
    let area_id = insert_area(&conn, "기술", 300);
    let act_id = insert_activity(&conn, "설계도");
    conn.execute(
        "INSERT INTO AreaActivity (area_id, activity_id) VALUES (?1, ?2)",
        rusqlite::params![area_id, act_id],
    )
    .unwrap();

    delete_area_impl(&conn, area_id).unwrap();

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM AreaActivity WHERE area_id=?1",
            rusqlite::params![area_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

// ── byte_limit 범위 검증 ───────────────────────────────────────
// 스키마의 CHECK 제약과 같은 기준을 커맨드 진입부에서 먼저 거른다.
// 제약에 그대로 걸리면 영문 SQLite 원문이 사용자에게 노출된다.

#[test]
fn test_create_area_byte_limit_zero_rejected_in_korean() {
    let conn = setup_test_db();
    let err = create_area_impl(&conn, "영역", 0).unwrap_err();
    assert!(err.contains("글자 수"), "byte_limit=0 에러 메시지: {err}");
    assert!(!err.contains("CHECK constraint"), "영문 원문 노출: {err}");
}

#[test]
fn test_create_area_negative_byte_limit_rejected_in_korean() {
    let conn = setup_test_db();
    let err = create_area_impl(&conn, "영역", -100).unwrap_err();
    assert!(err.contains("글자 수"), "byte_limit=-100 에러 메시지: {err}");
    assert!(!err.contains("CHECK constraint"), "영문 원문 노출: {err}");
}

#[test]
fn test_update_area_byte_limit_zero_rejected_in_korean() {
    let conn = setup_test_db();
    let id = create_area_impl(&conn, "영역", 500).unwrap();
    let err = update_area_impl(&conn, id, "영역", 0).unwrap_err();
    assert!(err.contains("글자 수"), "update byte_limit=0 에러 메시지: {err}");
    assert!(!err.contains("CHECK constraint"), "영문 원문 노출: {err}");
}

#[test]
fn test_delete_area_cascades_area_student() {
    let conn = setup_test_db();
    let area_id = insert_area(&conn, "가정", 300);
    let student_id = insert_student(&conn, 1, 1, 1, "홍길동");
    conn.execute(
        "INSERT INTO AreaStudent (area_id, student_id) VALUES (?1, ?2)",
        rusqlite::params![area_id, student_id],
    )
    .unwrap();

    delete_area_impl(&conn, area_id).unwrap();

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM AreaStudent WHERE area_id=?1",
            rusqlite::params![area_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

// ── validate_byte_limit ──────────────────────────────────────

#[test]
fn test_validate_byte_limit_accepts_positive() {
    assert!(validate_byte_limit(1).is_ok());
    assert!(validate_byte_limit(1500).is_ok());
}


