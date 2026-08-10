//! Text-entry engine: joystick picker (CharCycle) + optional multitap F0-F10.

use heapless::String;

use crate::config::keyboard as kbd_cfg;

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
    pub pending: Option<char>,
    charset_idx: usize,
    last_fn: Option<u8>,
    multitap_tap: u8,
    uppercase: bool,
}

impl<const N: usize> TextKeyboard<N> {
    pub fn new(mode: KeyboardMode) -> Self {
        Self {
            mode,
            buffer: String::new(),
            pending: None,
            charset_idx: 0,
            last_fn: None,
            multitap_tap: 0,
            uppercase: false,
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.pending = None;
        self.charset_idx = 0;
        self.last_fn = None;
        self.multitap_tap = 0;
        self.uppercase = false;
    }

    pub fn preview(&self) -> String<N> {
        let mut s = String::new();
        let _ = s.push_str(self.buffer.as_str());
        if let Some(c) = self.pending {
            let _ = s.push(self.apply_case(c));
        }
        s
    }

    fn apply_case(&self, c: char) -> char {
        if self.mode != KeyboardMode::Text {
            return c;
        }
        if self.uppercase {
            c.to_ascii_uppercase()
        } else {
            c.to_ascii_lowercase()
        }
    }

    /// Joystick / NavProfile picker: cycle pending character by `delta`.
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
        let len = set.chars().count() as isize;
        let step = delta as isize;
        if let Some(c) = self.pending {
            let idx = set.chars().position(|ch| ch == c).unwrap_or(0) as isize;
            let next = (idx + step).rem_euclid(len) as usize;
            self.pending = kbd_cfg::charset_char(set, next);
            self.charset_idx = next;
        } else {
            let next = (self.charset_idx as isize + step).rem_euclid(len) as usize;
            self.charset_idx = next;
            self.pending = kbd_cfg::charset_char(set, next);
        }
        KeyboardAction::Changed
    }

    /// Toggle letter case for pending / future text characters.
    pub fn case_toggle(&mut self) -> KeyboardAction {
        if self.mode != KeyboardMode::Text {
            return KeyboardAction::None;
        }
        self.uppercase = !self.uppercase;
        if let Some(c) = self.pending {
            self.pending = Some(self.apply_case(c));
        }
        KeyboardAction::Changed
    }

    pub fn nav_up(&mut self) -> KeyboardAction {
        self.char_cycle(-1)
    }

    pub fn nav_down(&mut self) -> KeyboardAction {
        self.char_cycle(1)
    }

    pub fn nav_right(&mut self) -> KeyboardAction {
        self.commit_pending();
        KeyboardAction::Committed
    }

    pub fn nav_left(&mut self) -> KeyboardAction {
        if self.pending.is_some() {
            self.pending = None;
            self.last_fn = None;
            self.multitap_tap = 0;
            KeyboardAction::CancelPending
        } else if self.buffer.pop().is_some() {
            KeyboardAction::Backspace
        } else {
            KeyboardAction::None
        }
    }

    pub fn back(&mut self) -> KeyboardAction {
        self.nav_left()
    }

    pub fn ok(&mut self) -> KeyboardAction {
        self.commit_pending();
        KeyboardAction::Committed
    }

    /// Optional multitap path (markwtech / legacy F-key text entry).
    pub fn fn_press(&mut self, key: u8) -> KeyboardAction {
        match self.mode {
            KeyboardMode::Digits => {
                if key <= 9 {
                    let c = (b'0' + key) as char;
                    if self.buffer.len() < N {
                        let _ = self.buffer.push(c);
                        return KeyboardAction::Changed;
                    }
                }
                KeyboardAction::None
            }
            KeyboardMode::Text => {
                if key > 10 {
                    return KeyboardAction::None;
                }
                if self.last_fn == Some(key) {
                    self.multitap_tap = self.multitap_tap.saturating_add(1);
                } else {
                    self.commit_pending();
                    self.last_fn = Some(key);
                    self.multitap_tap = 0;
                }
                self.pending = kbd_cfg::multitap_char(key, self.multitap_tap).map(|c| self.apply_case(c));
                KeyboardAction::Changed
            }
        }
    }

    fn commit_pending(&mut self) {
        if let Some(c) = self.pending {
            let c = self.apply_case(c);
            if self.buffer.len() < N {
                let _ = self.buffer.push(c);
            }
            self.pending = None;
            self.charset_idx = 0;
            self.last_fn = None;
            self.multitap_tap = 0;
        }
    }
}
