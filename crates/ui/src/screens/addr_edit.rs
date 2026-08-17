//! DCC address editor (menu row when the effective source is address-only).

use super::helpers::has_loco;
use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, ScreenId};
use crate::screen::{KeyBindings, Screen};
use crate::view::UiView;
use crate::widgets::{KeyboardMode, TextKeyboard};

/// Five-digit DCC address keyboard, same path as the empty-slot HUD.
pub struct AddrEditScreen {
    kbd: TextKeyboard<5>,
}

impl AddrEditScreen {
    /// Digit keyboard for a short or long DCC address.
    #[must_use]
    pub fn new() -> Self {
        Self {
            kbd: TextKeyboard::new(KeyboardMode::Digits),
        }
    }
}

impl Default for AddrEditScreen {
    fn default() -> Self {
        Self::new()
    }
}

fn wire_digits(s: &str) -> &str {
    match s.as_bytes().first() {
        Some(b'S' | b's' | b'L' | b'l') => s.get(1..).unwrap_or(""),
        _ => s,
    }
}

impl Screen for AddrEditScreen {
    fn id(&self) -> ScreenId {
        ScreenId::AddrEdit
    }

    fn key_bindings(&self, _cx: &ScreenCtx<'_>) -> KeyBindings {
        KeyBindings::TEXT
    }

    /// Prefill from the session draft, else the loco already on this slot.
    fn on_enter(&mut self, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        self.kbd.clear();
        if !cx.session.addr.is_empty() {
            self.kbd.load(wire_digits(cx.session.addr.as_str()));
            return;
        }
        if has_loco(cx)
            && let Some(addr) = cx
                .drive
                .slots
                .get(cx.drive.current)
                .and_then(|s| s.consist.first())
        {
            self.kbd.load(wire_digits(addr.as_str()));
        }
    }

    /// Title plus address preview.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        g.set(0, cx.s.msg_addr_edit, false);
        g.set(2, self.kbd.preview().as_str(), false);
        g.set(5, cx.s.hint_device_id_edit, false);
        UiView::Grid(g)
    }

    /// Encoder cycles the digit under the cursor.
    fn on_char_cycle(&mut self, d: i8, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let _ = self.kbd.char_cycle(d, cx.now_ms);
    }

    /// Move the digit cursor.
    fn on_cursor_move(&mut self, d: i8, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        if d < 0 {
            let _ = self.kbd.nav_left();
        } else {
            let _ = self.kbd.nav_right();
        }
    }

    /// Type a digit into the address.
    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        if c.is_ascii_digit() {
            let _ = self.kbd.key_press(c as u8 - b'0', cx.now_ms);
        }
    }

    /// Hardware Fn keys type digits.
    fn on_fn_key(&mut self, k: u8, down: bool, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        if down {
            let _ = self.kbd.fn_press(k, cx.now_ms);
        }
    }

    /// Commit pending multitap.
    fn on_tick(&mut self, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        self.kbd.tick(cx.now_ms);
    }

    /// Acquire the typed address into the current throttle slot.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let _ = self.kbd.ok();
        if self.kbd.buffer.is_empty() {
            return;
        }
        cx.session.addr.clear();
        let _ = cx.session.addr.push_str(self.kbd.buffer.as_str());
        nav.emit(Intent::AcquireAddr);
        nav.root(ScreenId::Throttle);
    }
}
