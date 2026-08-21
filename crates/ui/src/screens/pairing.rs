//! Manual six-digit `BigFred` pairing code editor.

use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, ScreenId};
use crate::screen::{KeyBindings, Screen};
use crate::view::UiView;
use crate::widgets::{KeyboardMode, TextKeyboard};

/// Six-digit code entered with keypad, Fn keys, or joystick.
pub struct PairingScreen {
    kbd: TextKeyboard<6>,
}

impl PairingScreen {
    /// Empty six-digit pairing keyboard.
    #[must_use]
    pub fn new() -> Self {
        Self {
            kbd: TextKeyboard::new(KeyboardMode::Digits),
        }
    }
}

impl Default for PairingScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for PairingScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Pairing
    }

    fn key_bindings(&self, _cx: &ScreenCtx<'_>) -> KeyBindings {
        KeyBindings::TEXT
    }

    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        g.set(0, cx.s.msg_pairing_code, false);
        g.set(2, self.kbd.preview().as_str(), false);
        g.set(5, cx.s.hint_device_id_edit, false);
        UiView::Grid(g)
    }

    fn on_char_cycle(&mut self, d: i8, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let _ = self.kbd.char_cycle(d, cx.now_ms);
    }

    fn on_cursor_move(&mut self, d: i8, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        if d < 0 {
            let _ = self.kbd.nav_left();
        } else {
            let _ = self.kbd.nav_right();
        }
    }

    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        if c.is_ascii_digit() {
            let _ = self.kbd.key_press(c as u8 - b'0', cx.now_ms);
        }
    }

    fn on_fn_key(&mut self, k: u8, down: bool, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        if down {
            let _ = self.kbd.fn_press(k, cx.now_ms);
        }
    }

    fn on_tick(&mut self, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        self.kbd.tick(cx.now_ms);
    }

    fn on_select(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let _ = self.kbd.ok();
        if self.kbd.buffer.len() != 6 {
            return;
        }
        nav.emit(Intent::Pair(self.kbd.buffer.clone()));
        nav.replace(ScreenId::PairingWait);
    }

    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.root(ScreenId::Throttle);
    }
}
