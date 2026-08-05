-- ================================================================
-- 스키마 버전 1 스냅샷 (정식 출시 시점)
--
-- ⚠️ 이 파일은 수정 금지. 이미 배포된 DB 파일의 구조를 기록한 사본이다.
--    스키마를 바꿀 때는 이 파일을 고치는 것이 아니라
--    db.rs의 SCHEMA_VERSION을 올리고 vN.sql을 새로 추가한다.
--    자세한 절차는 schema_lock_tests.rs 참고.
-- ================================================================

-- ================================================================
-- 기본 테이블
-- ================================================================

CREATE TABLE IF NOT EXISTS Student
(
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    grade     INTEGER NOT NULL,
    class_num INTEGER NOT NULL,
    number    INTEGER NOT NULL,
    name      TEXT    NOT NULL,
    UNIQUE (grade, class_num, number),
    CHECK (grade >= 1 AND class_num >= 1 AND number >= 1)
);

CREATE TABLE IF NOT EXISTS Area
(
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT    NOT NULL UNIQUE,
    byte_limit INTEGER NOT NULL CHECK (byte_limit > 0)
);

CREATE TABLE IF NOT EXISTS Activity
(
    id   INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);

-- ================================================================
-- 관계 테이블
-- ================================================================

CREATE TABLE IF NOT EXISTS AreaActivity
(
    area_id       INTEGER NOT NULL,
    activity_id   INTEGER NOT NULL,
    PRIMARY KEY (area_id, activity_id),
    FOREIGN KEY (area_id) REFERENCES Area (id) ON DELETE CASCADE,
    FOREIGN KEY (activity_id) REFERENCES Activity (id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS AreaStudent
(
    area_id             INTEGER NOT NULL,
    student_id          INTEGER NOT NULL,
    PRIMARY KEY (area_id, student_id),
    FOREIGN KEY (area_id) REFERENCES Area (id) ON DELETE CASCADE,
    FOREIGN KEY (student_id) REFERENCES Student (id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS ActivityRecord
(
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    activity_id INTEGER NOT NULL,
    student_id  INTEGER NOT NULL,
    content     TEXT    NOT NULL DEFAULT '',
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE (activity_id, student_id),
    FOREIGN KEY (activity_id) REFERENCES Activity (id) ON DELETE CASCADE,
    FOREIGN KEY (student_id) REFERENCES Student (id) ON DELETE CASCADE
);

-- ================================================================
-- 이력
-- ================================================================

CREATE TABLE IF NOT EXISTS ActivityRecordHistory
(
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    activity_record_id INTEGER NOT NULL,
    content            TEXT    NOT NULL,
    changed_at         TEXT    NOT NULL DEFAULT (datetime('now')),
    note               TEXT,
    FOREIGN KEY (activity_record_id) REFERENCES ActivityRecord (id) ON DELETE CASCADE
);

-- ================================================================
-- 인덱스
-- ================================================================

CREATE INDEX IF NOT EXISTS idx_history_record ON ActivityRecordHistory (activity_record_id, changed_at);
CREATE INDEX IF NOT EXISTS idx_record_student ON ActivityRecord (student_id);
CREATE INDEX IF NOT EXISTS idx_areastudent_student ON AreaStudent (student_id);
CREATE INDEX IF NOT EXISTS idx_areaactivity_activity ON AreaActivity (activity_id);

-- ================================================================
-- 스냅샷
-- ================================================================

CREATE TABLE IF NOT EXISTS Snapshot
(
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    memo       TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_snapshot_created_at ON Snapshot (created_at);

-- ================================================================
-- 치환 규칙
-- ================================================================

CREATE TABLE IF NOT EXISTS ReplaceRule
(
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    old_text   TEXT    NOT NULL,
    new_text   TEXT    NOT NULL,
    is_regex   INTEGER NOT NULL DEFAULT 0,
    enabled    INTEGER NOT NULL DEFAULT 1,
    priority   INTEGER NOT NULL DEFAULT 0 CHECK (priority >= 0),
    created_at TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE (old_text, new_text)
);

CREATE INDEX IF NOT EXISTS idx_replace_rule_priority ON ReplaceRule (priority);

-- ================================================================
-- 유의어 점검
-- ================================================================

CREATE TABLE IF NOT EXISTS SynonymGroup
(
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT    NOT NULL UNIQUE,
    created_at TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS SynonymItem
(
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    group_id INTEGER NOT NULL,
    word     TEXT    NOT NULL,
    UNIQUE (group_id, word),
    FOREIGN KEY (group_id) REFERENCES SynonymGroup (id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_synonym_item_group ON SynonymItem (group_id);

-- ================================================================
-- 앱 설정
-- ================================================================

CREATE TABLE IF NOT EXISTS APP_CONFIGS
(
    config_key   TEXT PRIMARY KEY,
    config_value TEXT NOT NULL
);
