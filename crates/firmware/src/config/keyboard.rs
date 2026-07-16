//! Multitap text-entry tables for F0-F10 keys.

/// Multitap character groups per function key (F0..F10).
pub const MULTITAP: [&str; 11] = [
    " 0",       // F0
    "1",        // F1
    "abc2",     // F2
    "def3",     // F3
    "ghi4",     // F4
    "jkl5",     // F5
    "mno6",     // F6
    "pqrs7",    // F7
    "tuv8",     // F8
    "wxyz9",    // F9
    " @.",      // F10: space, @, period
];

/// Full charset cycled by joystick Up/Down in text mode.
pub const TEXT_CHARSET: &str =
    " abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_.@";

/// Digits only (IP, device ID, loco address).
pub const DIGIT_CHARSET: &str = "0123456789";

pub fn multitap_char(key: u8, tap: u8) -> Option<char> {
    let group = MULTITAP.get(key as usize)?;
    if group.is_empty() {
        return None;
    }
    let idx = (tap as usize) % group.len();
    group.chars().nth(idx)
}

pub fn charset_char(set: &str, index: usize) -> Option<char> {
    if set.is_empty() {
        return None;
    }
    set.chars().nth(index % set.len())
}
