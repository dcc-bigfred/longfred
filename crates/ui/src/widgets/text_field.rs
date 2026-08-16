//! Shared text-field engine: in-buffer caret `_`, T9, alphabet cycle, 2 s idle commit.

use heapless::String;

use crate::view::{LINE_LEN, Line};
use crate::widgets::charset as kbd_cfg;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyboardMode {
    Text,
    Digits,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyboardAction {
    None,
    Changed,
    Committed,
    Backspace,
    CancelPending,
}

pub struct TextKeyboard<const N: usize> {
    pub mode: KeyboardMode,
    pub buffer: String<N>,
    pending: Option<char>,
    cursor: usize,
    max_len: usize,
    charset_idx: usize,
    last_key: Option<u8>,
    multitap_tap: u8,
    uppercase: bool,
    last_edit_ms: Option<u64>,
}

impl<const N: usize> TextKeyboard<N> {
    #[must_use]
    pub fn new(mode: KeyboardMode) -> Self {
        Self {
            mode,
            buffer: String::new(),
            pending: None,
            cursor: 0,
            max_len: N,
            charset_idx: 0,
            last_key: None,
            multitap_tap: 0,
            uppercase: false,
            last_edit_ms: None,
        }
    }

    pub fn set_max_len(&mut self, max_len: usize) {
        self.max_len = max_len.min(N);
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.pending = None;
        self.cursor = 0;
        self.charset_idx = 0;
        self.last_key = None;
        self.multitap_tap = 0;
        self.uppercase = false;
        self.last_edit_ms = None;
    }

    /// Replace the buffer and park the caret at the end.
    ///
    /// Non-ASCII and control characters are dropped. The caret is a byte index,
    /// so the buffer must stay single-byte ASCII.
    pub fn load(&mut self, s: &str) {
        self.clear();
        for c in s.chars() {
            if !c.is_ascii() || c.is_ascii_control() {
                continue;
            }
            if self.buffer.len() >= self.max_len {
                break;
            }
            if self.buffer.push(c).is_err() {
                break;
            }
        }
        self.cursor = self.buffer.len();
        debug_assert!(self.buffer.is_ascii());
    }

    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    #[must_use]
    pub fn pending(&self) -> Option<char> {
        self.pending.map(|c| self.apply_case(c))
    }

    #[must_use]
    pub fn uppercase(&self) -> bool {
        self.uppercase
    }

    #[must_use]
    pub fn slot_char(&self) -> char {
        self.pending().unwrap_or('_')
    }

    fn cap(&self) -> usize {
        self.max_len.min(N)
    }

    fn can_insert(&self) -> bool {
        self.buffer.len() < self.cap()
    }

    fn note_edit(&mut self, now_ms: u64) {
        self.last_edit_ms = Some(now_ms);
    }

    fn apply_case(&self, c: char) -> char {
        if self.mode != KeyboardMode::Text || !c.is_ascii_alphabetic() {
            return c;
        }
        if self.uppercase {
            c.to_ascii_uppercase()
        } else {
            c.to_ascii_lowercase()
        }
    }

    fn insert_at_cursor(&mut self, c: char) -> bool {
        if !c.is_ascii() || c.is_ascii_control() || !self.can_insert() {
            return false;
        }
        let i = self.cursor.min(self.buffer.len());
        let mut tail: heapless::Vec<u8, N> = heapless::Vec::new();
        while self.buffer.len() > i {
            let Some(ch) = self.buffer.pop() else {
                break;
            };
            let _ = tail.push(ch as u8);
        }
        if self.buffer.push(c).is_err() {
            while let Some(b) = tail.pop() {
                let _ = self.buffer.push(b as char);
            }
            return false;
        }
        while let Some(b) = tail.pop() {
            let _ = self.buffer.push(b as char);
        }
        self.cursor = i + 1;
        true
    }

    fn commit_pending(&mut self) -> bool {
        let Some(c) = self.pending.take() else {
            return false;
        };
        let c = self.apply_case(c);
        self.last_key = None;
        self.multitap_tap = 0;
        self.last_edit_ms = None;
        self.insert_at_cursor(c)
    }

    fn clear_pending(&mut self) {
        self.pending = None;
        self.last_key = None;
        self.multitap_tap = 0;
        self.last_edit_ms = None;
    }

    /// Commit pending (if any) without advancing an extra empty slot.
    pub fn ok(&mut self) -> KeyboardAction {
        if self.commit_pending() {
            KeyboardAction::Committed
        } else {
            KeyboardAction::None
        }
    }

    /// Joystick / encoder: cycle the character in the caret slot.
    pub fn char_cycle(&mut self, delta: i8, now_ms: u64) -> KeyboardAction {
        if delta == 0 {
            return KeyboardAction::None;
        }
        let set = match self.mode {
            KeyboardMode::Text => kbd_cfg::TEXT_CHARSET,
            KeyboardMode::Digits => kbd_cfg::DIGIT_CHARSET,
        };
        if set.is_empty() {
            return KeyboardAction::None;
        }
        if self.pending.is_none() && !self.can_insert() {
            return KeyboardAction::None;
        }
        let len = set.chars().count().cast_signed();
        let step = isize::from(delta);
        let idx = if let Some(c) = self.pending {
            set.chars()
                .position(|ch| ch == c.to_ascii_lowercase() || ch == c)
                .unwrap_or(self.charset_idx)
                .cast_signed()
        } else {
            self.charset_idx.cast_signed()
        };
        let next = (idx + step).rem_euclid(len) as usize;
        self.charset_idx = next;
        self.pending = kbd_cfg::charset_char(set, next);
        self.last_key = None;
        self.note_edit(now_ms);
        KeyboardAction::Changed
    }

    pub fn case_toggle(&mut self) -> KeyboardAction {
        if self.mode != KeyboardMode::Text {
            return KeyboardAction::None;
        }
        self.uppercase = !self.uppercase;
        KeyboardAction::Changed
    }

    pub fn nav_right(&mut self) -> KeyboardAction {
        if self.pending.is_some() {
            let _ = self.commit_pending();
            return KeyboardAction::Committed;
        }
        if self.cursor < self.buffer.len() {
            self.cursor += 1;
            return KeyboardAction::Changed;
        }
        KeyboardAction::None
    }

    pub fn nav_left(&mut self) -> KeyboardAction {
        if self.pending.is_some() {
            self.clear_pending();
            return KeyboardAction::CancelPending;
        }
        if self.cursor == self.buffer.len() && !self.buffer.is_empty() {
            let _ = self.buffer.pop();
            self.cursor = self.buffer.len();
            return KeyboardAction::Backspace;
        }
        if self.cursor > 0 {
            self.cursor -= 1;
            return KeyboardAction::Changed;
        }
        KeyboardAction::None
    }

    /// Phone keypad digit 0–9 (T9 in Text, immediate insert in Digits).
    pub fn key_press(&mut self, key: u8, now_ms: u64) -> KeyboardAction {
        if key > 9 {
            return KeyboardAction::None;
        }
        match self.mode {
            KeyboardMode::Digits => self.insert_digit_immediate(key),
            KeyboardMode::Text => self.multitap(key, now_ms),
        }
    }

    /// `LongFred` F-keys: digits only (F0–F9). No T9 on function keys.
    pub fn fn_press(&mut self, key: u8, now_ms: u64) -> KeyboardAction {
        let _ = now_ms;
        if self.mode == KeyboardMode::Digits && key <= 9 {
            self.insert_digit_immediate(key)
        } else {
            KeyboardAction::None
        }
    }

    fn insert_digit_immediate(&mut self, key: u8) -> KeyboardAction {
        let _ = self.commit_pending();
        let c = (b'0' + key) as char;
        if self.insert_at_cursor(c) {
            KeyboardAction::Committed
        } else {
            KeyboardAction::None
        }
    }

    fn multitap(&mut self, key: u8, now_ms: u64) -> KeyboardAction {
        let Some(group) = kbd_cfg::multitap_group(key) else {
            return KeyboardAction::None;
        };
        if group.len() == 1 {
            let _ = self.commit_pending();
            if let Some(c) = group.chars().next()
                && self.insert_at_cursor(self.apply_case(c))
            {
                return KeyboardAction::Committed;
            }
            return KeyboardAction::None;
        }
        if self.last_key == Some(key) {
            self.multitap_tap = self.multitap_tap.saturating_add(1);
        } else {
            let _ = self.commit_pending();
            if !self.can_insert() {
                return KeyboardAction::None;
            }
            self.last_key = Some(key);
            self.multitap_tap = 0;
        }
        self.pending = kbd_cfg::multitap_char(key, self.multitap_tap);
        self.note_edit(now_ms);
        KeyboardAction::Changed
    }

    /// Commit pending after [`kbd_cfg::IDLE_COMMIT_MS`] of inactivity.
    pub fn tick(&mut self, now_ms: u64) {
        if self.pending.is_none() {
            return;
        }
        let Some(t) = self.last_edit_ms else {
            return;
        };
        if now_ms.saturating_sub(t) >= kbd_cfg::IDLE_COMMIT_MS {
            let _ = self.commit_pending();
        }
    }

    /// Buffer + pending, no caret — for the throttle loco-address line.
    #[must_use]
    pub fn value_preview(&self) -> String<N> {
        let mut s = String::new();
        let (head, tail) = split_at_cursor(self.buffer.as_str(), self.cursor);
        let _ = s.push_str(head);
        if let Some(c) = self.pending() {
            let _ = s.push(c);
        }
        let _ = s.push_str(tail);
        s
    }

    /// Caret preview (`prefix + slot + suffix`), windowed to [`LINE_LEN`].
    #[must_use]
    pub fn preview(&self) -> Line {
        let mut full = heapless::String::<65>::new();
        let (head, tail) = split_at_cursor(self.buffer.as_str(), self.cursor);
        let _ = full.push_str(head);
        let focus = full.len();
        let _ = full.push(self.slot_char());
        let _ = full.push_str(tail);
        window_around(full.as_str(), focus)
    }
}

/// Split `s` at byte `cursor`. A mid-character index degrades to `(s, "")`.
fn split_at_cursor(s: &str, cursor: usize) -> (&str, &str) {
    let i = cursor.min(s.len());
    match s.get(..i) {
        Some(head) => (head, s.get(i..).unwrap_or("")),
        None => (s, ""),
    }
}

/// Dotted IPv4, optionally `:port`, with `_` / pending at `cursor`.
#[must_use]
pub fn format_grouped_ip(digits: &str, cursor: usize, slot: char, with_port: bool) -> Line {
    let mut full = heapless::String::<32>::new();
    let mut shown = 0usize;
    let mut focus = 0usize;
    for (i, c) in digits.chars().enumerate() {
        if i == cursor {
            focus = full.len();
            push_digit_slot(&mut full, shown, slot, with_port);
            shown += 1;
        }
        push_digit_slot(&mut full, shown, c, with_port);
        shown += 1;
    }
    if cursor >= digits.len() {
        focus = full.len();
        push_digit_slot(&mut full, shown, slot, with_port);
    }
    window_around(full.as_str(), focus)
}

fn push_digit_slot(out: &mut heapless::String<32>, digit_index: usize, ch: char, with_port: bool) {
    if digit_index == 3 || digit_index == 6 || digit_index == 9 {
        let _ = out.push('.');
    } else if with_port && digit_index == 12 {
        let _ = out.push(':');
    }
    let _ = out.push(ch);
}

fn window_around(s: &str, focus: usize) -> Line {
    let mut line = Line::new();
    if s.len() <= LINE_LEN {
        crate::view::push_oled(&mut line, s);
        return line;
    }
    let max_start = s.len().saturating_sub(LINE_LEN);
    let start = focus.saturating_sub(LINE_LEN / 2).min(max_start);
    crate::view::push_oled(&mut line, s.get(start..start + LINE_LEN).unwrap_or(s));
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_drops_non_ascii_and_stays_insertable() {
        let mut kbd = TextKeyboard::<16>::new(KeyboardMode::Text);
        kbd.load("hasło");
        assert_eq!(kbd.buffer.as_str(), "haso");
        assert!(kbd.insert_at_cursor('x'));
        assert_eq!(kbd.buffer.as_str(), "hasox");
    }

    #[test]
    fn load_skips_control_chars() {
        let mut kbd = TextKeyboard::<8>::new(KeyboardMode::Text);
        kbd.load("ab\ncd");
        assert_eq!(kbd.buffer.as_str(), "abcd");
    }

    #[test]
    fn load_stops_at_max_len() {
        let mut kbd = TextKeyboard::<8>::new(KeyboardMode::Digits);
        kbd.set_max_len(4);
        kbd.load("123456");
        assert_eq!(kbd.buffer.as_str(), "1234");
        assert!(!kbd.insert_at_cursor('7'));
    }

    #[test]
    fn insert_in_the_middle() {
        let mut kbd = TextKeyboard::<8>::new(KeyboardMode::Digits);
        kbd.load("13");
        kbd.cursor = 1;
        assert!(kbd.insert_at_cursor('2'));
        assert_eq!(kbd.buffer.as_str(), "123");
        assert_eq!(kbd.cursor, 2);
    }

    #[test]
    fn split_at_cursor_degrades_on_mid_char_index() {
        assert_eq!(split_at_cursor("ab", 1), ("a", "b"));
        assert_eq!(split_at_cursor("ł", 1), ("ł", ""));
    }

    #[test]
    fn char_cycle_on_full_buffer_does_not_grow() {
        let mut kbd = TextKeyboard::<4>::new(KeyboardMode::Digits);
        kbd.load("1234");
        let _ = kbd.char_cycle(1, 0);
        assert_eq!(kbd.buffer.as_str(), "1234");
        assert_eq!(kbd.buffer.len(), 4);
    }

    #[test]
    fn exact_capacity_rejects_one_more() {
        let mut kbd = TextKeyboard::<3>::new(KeyboardMode::Digits);
        kbd.load("12");
        assert!(kbd.insert_at_cursor('3'));
        assert_eq!(kbd.buffer.as_str(), "123");
        assert!(!kbd.insert_at_cursor('4'));
    }
}
