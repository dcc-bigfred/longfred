//! Wi-Fi password entry.

use super::helpers::password_for_ssid;
use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, ScreenId};
use crate::screen::{KeyBindings, Screen};
use crate::view::UiView;
use crate::widgets::{KeyboardMode, TextKeyboard};

/// Wi-Fi password entry.
pub struct PasswordScreen {
    kbd: TextKeyboard<64>,
}

impl PasswordScreen {
    /// Text keyboard preloaded from NVS / compiled password on enter.
    #[must_use]
    pub fn new() -> Self {
        let mut kbd = TextKeyboard::new(KeyboardMode::Text);
        kbd.set_max_len(64);
        Self { kbd }
    }
}

impl Default for PasswordScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for PasswordScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Password
    }

    fn key_bindings(&self, _cx: &ScreenCtx<'_>) -> KeyBindings {
        KeyBindings::TEXT
    }

    /// Prefill from the session buffer, else stored/compiled password for this SSID.
    ///
    /// The keyboard is always empty here: [`Router`] reconstructs the screen on enter.
    fn on_enter(&mut self, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        if !cx.session.password.is_empty() {
            self.kbd.load(cx.session.password.as_str());
            return;
        }
        let stored = {
            let s = password_for_ssid(cx, cx.session.selected_ssid.as_str());
            let mut buf = heapless::String::<64>::new();
            let _ = buf.push_str(s);
            buf
        };
        self.kbd.load(stored.as_str());
        let _ = cx.session.password.push_str(stored.as_str());
    }

    /// Password preview (with caps hint on keypad boards).
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        g.set(0, cx.s.msg_enter_password, false);
        g.set(2, self.kbd.preview().as_str(), false);
        g.set(5, cx.s.hint_enter_password, false);
        if cx.env.has_keypad {
            g.caps = Some(self.kbd.uppercase());
        }
        UiView::Grid(g)
    }

    /// Encoder cycles the character under the cursor.
    fn on_char_cycle(&mut self, d: i8, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let _ = self.kbd.char_cycle(d, cx.now_ms);
    }

    /// Move the text cursor.
    fn on_cursor_move(&mut self, d: i8, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        if d < 0 {
            let _ = self.kbd.nav_left();
        } else {
            let _ = self.kbd.nav_right();
        }
    }

    /// Toggle uppercase on the text keyboard.
    fn on_case_toggle(&mut self, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let _ = self.kbd.case_toggle();
    }

    /// Multitap digit into the password.
    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        if c.is_ascii_digit() {
            let _ = self.kbd.key_press(c as u8 - b'0', cx.now_ms);
        }
    }

    /// Hardware Fn keys type into the password keyboard.
    fn on_fn_key(&mut self, k: u8, down: bool, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        if down {
            let _ = self.kbd.fn_press(k, cx.now_ms);
        }
    }

    /// Commit pending multitap.
    fn on_tick(&mut self, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        self.kbd.tick(cx.now_ms);
    }

    /// Save the password, mark NVS save if it came from a scan, then connect.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let _ = self.kbd.ok();
        cx.session.password.clear();
        let _ = cx.session.password.push_str(self.kbd.buffer.as_str());
        if cx.session.selected_from_scan && !cx.session.password.is_empty() {
            cx.session.pending_password_save = true;
        }
        nav.replace(ScreenId::Connecting);
        nav.emit(Intent::WifiConnect);
    }

    /// Keep the typed password in the session and go back.
    fn on_cancel(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let _ = self.kbd.ok();
        cx.session.password.clear();
        let _ = cx.session.password.push_str(self.kbd.buffer.as_str());
        nav.back();
    }
}
