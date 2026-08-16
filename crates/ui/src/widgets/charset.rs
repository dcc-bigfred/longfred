//! Multitap tables and charsets for the shared text-field engine.

/// Idle time after which a pending T9 / alphabet character is committed
/// and the cursor advances one slot to the right.
pub const IDLE_COMMIT_MS: u64 = 2_000;

/// Phone-keypad groups indexed by digit 0–9.
pub const MULTITAP: [&str; 10] = [
    "0",            // 0
    "1_-*#@%^&!+=", // 1 — `_` is a typed character, not the caret
    "2abc",         // 2
    "3def",         // 3
    "4ghi",         // 4
    "5jkl",         // 5
    "6mno",         // 6
    "7pqrs",        // 7
    "8tuv",         // 8
    "9wxyz",        // 9
];

/// Joystick / encoder charset (letters are cased via Caps Lock).
pub const TEXT_CHARSET: &str = " abcdefghijklmnopqrstuvwxyz0123456789-_.@";

/// Digits only (IP, device ID, net config, loco address).
pub const DIGIT_CHARSET: &str = "0123456789";

#[must_use]
pub fn multitap_group(key: u8) -> Option<&'static str> {
    MULTITAP.get(key as usize).copied()
}

#[must_use]
pub fn multitap_char(key: u8, tap: u8) -> Option<char> {
    let group = multitap_group(key)?;
    if group.is_empty() {
        return None;
    }
    let idx = (tap as usize) % group.len();
    group.chars().nth(idx)
}

#[must_use]
pub fn charset_char(set: &str, index: usize) -> Option<char> {
    if set.is_empty() {
        return None;
    }
    set.chars().nth(index % set.len())
}
