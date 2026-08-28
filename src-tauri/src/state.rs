use rusqlite::Connection;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use zeroize::{Zeroize, ZeroizeOnDrop};

pub struct DbState(pub Mutex<Option<Connection>>);
pub struct DbPathState(pub Mutex<Option<PathBuf>>);

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct CryptoState {
    pub key: Option<[u8; 32]>,
}

pub type CryptoStateHandle = Mutex<CryptoState>;

pub fn current_crypto_key(crypto: &CryptoStateHandle) -> Result<Option<[u8; 32]>, String> {
    let guard = crypto.lock().map_err(|e| e.to_string())?;
    Ok(guard.key)
}

pub fn set_crypto_state(
    crypto: &CryptoStateHandle,
    key: [u8; 32],
) -> Result<(), String> {
    let mut guard = crypto.lock().map_err(|e| e.to_string())?;
    guard.key = Some(key);
    Ok(())
}

pub fn clear_crypto_state(crypto: &CryptoStateHandle) -> Result<(), String> {
    let mut guard = crypto.lock().map_err(|e| e.to_string())?;
    if let Some(ref mut k) = guard.key { k.zeroize(); }
    guard.key = None;
    Ok(())
}

/// 치환 결과 캐시.
///
/// `entries`의 key와 value는 모두 **복호화된 평문**이다. 따라서 규칙이 바뀌거나
/// 프로젝트가 바뀌면 반드시 비워야 한다. 남겨두면 암호화 키를 zeroize한 뒤에도,
/// 심지어 다른 프로젝트를 연 뒤에도 이전 학생 기록의 평문이 메모리에 남는다.
pub struct ReplaceCache {
    pub ruleset_version: u64,
    pub entries: HashMap<String, (String, u64)>,
}

/// 캐시에 보관할 최대 항목 수. 넘으면 통째로 비운다.
/// 캐시는 성능 보조 수단일 뿐이라 비워도 정확성에는 영향이 없다.
pub const MAX_CACHE_ENTRIES: usize = 5_000;

impl ReplaceCache {
    /// 규칙 세트 버전을 올리고 보관 중인 평문을 모두 버린다.
    ///
    /// 버전만 올리면 옛 항목은 조회되지 않을 뿐 메모리에는 그대로 남는다.
    pub fn invalidate(&mut self) {
        self.ruleset_version += 1;
        self.entries.clear();
    }
}

pub type ReplaceCacheState = Mutex<ReplaceCache>;

/// SQLite 제약 위반을 한국어 메시지로 바꾼다.
///
/// 번역하지 않으면 "CHECK constraint failed: Student" 같은 영문 원문이 그대로
/// 교사에게 표시된다. CHECK 위반은 커맨드 진입부 검증이 먼저 막는 것이 원칙이고,
/// 이 번역은 검증이 놓친 경우를 위한 방어선이다.
pub fn constraint_err(e: &rusqlite::Error, conflict_msg: &str) -> String {
    let text = e.to_string();
    if text.contains("UNIQUE constraint failed") {
        conflict_msg.to_string()
    } else if text.contains("CHECK constraint failed") {
        "입력값이 허용 범위를 벗어났습니다.".to_string()
    } else {
        text
    }
}
