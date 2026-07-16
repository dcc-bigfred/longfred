//! Text-entry engine: joystick Up/Down + multitap F0-F10.

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
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.pending = None;
        self.charset_idx = 0;
        self.last_fn = None;
        self.multitap_tap = 0;
    }

    pub fn preview(&self) -> String<N> {
        let mut s = String::new();
        let _ = s.push_str(self.buffer.as_str());
        if let Some(c) = self.pending {
            let _ = s.push(c);
        }
        s
    }

    pub fn nav_up(&mut self) -> KeyboardAction {
        self.cycle_pending(true)
    }

    pub fn nav_down(&mut self) -> KeyboardAction {
        self.cycle_pending(false)
    }

    fn cycle_pending(&mut self, up: bool) -> KeyboardAction {
        let set = match self.mode {
            KeyboardMode::Text => kbd_cfg::TEXT_CHARSET,
            KeyboardMode::Digits => kbd_cfg::DIGIT_CHARSET,
        };
        if set.is_empty() {
            return KeyboardAction::None;
        }
        let len = set.chars().count();
        if let Some(c) = self.pending {
            let idx = set.chars().position(|ch| ch == c).unwrap_or(0);
            let next = if up {
                (idx + len - 1) % len
            } else {
                (idx + 1) % len
            };
            self.pending = kbd_cfg::charset_char(set, next);
        } else {
            self.charset_idx = if up {
                (self.charset_idx + len - 1) % len
            } else {
                (self.charset_idx + 1) % len
            };
            self.pending = kbd_cfg::charset_char(set, self.charset_idx);
        }
        KeyboardAction::Changed
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
                self.pending = kbd_cfg::multitap_char(key, self.multitap_tap);
                KeyboardAction::Changed
            }
        }
    }

    fn commit_pending(&mut self) {
        if let Some(c) = self.pending {
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
