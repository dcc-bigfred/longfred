//! Shared text-field engine: in-buffer caret `_`, T9, alphabet cycle, 2 s idle commit.

use embassy_time::Instant;
use heapless::String;

use crate::config::keyboard as kbd_cfg;
use crate::ui::view::{LINE_LEN, Line};

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
    pub fn load(&mut self, s: &str) {
        self.clear();
        for c in s.chars() {
            if self.buffer.len() >= self.max_len {
                break;
            }
            let _ = self.buffer.push(c);
        }
        self.cursor = self.buffer.len();
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn pending(&self) -> Option<char> {
        self.pending.map(|c| self.apply_case(c))
    }

    pub fn uppercase(&self) -> bool {
        self.uppercase
    }

    pub fn slot_char(&self) -> char {
        self.pending().unwrap_or('_')
    }

    fn cap(&self) -> usize {
        self.max_len.min(N)
    }

    fn can_insert(&self) -> bool {
        self.buffer.len() < self.cap()
    }

    fn note_edit(&mut self) {
        self.last_edit_ms = Some(Instant::now().as_millis());
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
        if !self.can_insert() {
            return false;
        }
        let i = self.cursor.min(self.buffer.len());
        let mut tmp = String::<N>::new();
        let _ = tmp.push_str(&self.buffer.as_str()[..i]);
        if tmp.push(c).is_err() {
            return false;
        }
        let _ = tmp.push_str(&self.buffer.as_str()[i..]);
        self.buffer = tmp;
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
    pub fn char_cycle(&mut self, delta: i8) -> KeyboardAction {
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
        let len = set.chars().count() as isize;
        let step = delta as isize;
        let idx = if let Some(c) = self.pending {
            set.chars()
                .position(|ch| ch == c.to_ascii_lowercase() || ch == c)
                .unwrap_or(self.charset_idx) as isize
        } else {
            self.charset_idx as isize
        };
        let next = (idx + step).rem_euclid(len) as usize;
        self.charset_idx = next;
        self.pending = kbd_cfg::charset_char(set, next);
        self.last_key = None;
        self.note_edit();
        KeyboardAction::Changed
    }

    pub fn case_toggle(&mut self) -> KeyboardAction {
        if self.mode != KeyboardMode::Text {
            return KeyboardAction::None;
        }
        self.uppercase = !self.uppercase;
        KeyboardAction::Changed
    }

    pub fn nav_up(&mut self) -> KeyboardAction {
        self.char_cycle(-1)
    }

    pub fn nav_down(&mut self) -> KeyboardAction {
        self.char_cycle(1)
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

    pub fn back(&mut self) -> KeyboardAction {
        self.nav_left()
    }

    /// Phone keypad digit 0–9 (T9 in Text, immediate insert in Digits).
    pub fn key_press(&mut self, key: u8) -> KeyboardAction {
        if key > 9 {
            return KeyboardAction::None;
        }
        match self.mode {
            KeyboardMode::Digits => self.insert_digit_immediate(key),
            KeyboardMode::Text => self.multitap(key),
        }
    }

    /// LongFred F-keys: digits only (F0–F9). No T9 on function keys.
    pub fn fn_press(&mut self, key: u8) -> KeyboardAction {
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

    fn multitap(&mut self, key: u8) -> KeyboardAction {
        let Some(group) = kbd_cfg::multitap_group(key) else {
            return KeyboardAction::None;
        };
        if group.len() == 1 {
            let _ = self.commit_pending();
            if let Some(c) = group.chars().next() {
                if self.insert_at_cursor(self.apply_case(c)) {
                    return KeyboardAction::Committed;
                }
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
        self.note_edit();
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
    pub fn value_preview(&self) -> String<N> {
        let mut s = String::new();
        let i = self.cursor.min(self.buffer.len());
        let _ = s.push_str(&self.buffer.as_str()[..i]);
        if let Some(c) = self.pending() {
            let _ = s.push(c);
        }
        let _ = s.push_str(&self.buffer.as_str()[i..]);
        s
    }

    /// Caret preview (`prefix + slot + suffix`), windowed to [`LINE_LEN`].
    pub fn preview(&self) -> Line {
        let mut full = heapless::String::<65>::new();
        let i = self.cursor.min(self.buffer.len());
        let _ = full.push_str(&self.buffer.as_str()[..i]);
        let focus = full.len();
        let _ = full.push(self.slot_char());
        let _ = full.push_str(&self.buffer.as_str()[i..]);
        window_around(full.as_str(), focus)
    }
}

/// Dotted IPv4, optionally `:port`, with `_` / pending at `cursor`.
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
        crate::ui::view::push_oled(&mut line, s);
        return line;
    }
    let max_start = s.len().saturating_sub(LINE_LEN);
    let start = focus.saturating_sub(LINE_LEN / 2).min(max_start);
    crate::ui::view::push_oled(&mut line, s.get(start..start + LINE_LEN).unwrap_or(s));
    line
}
