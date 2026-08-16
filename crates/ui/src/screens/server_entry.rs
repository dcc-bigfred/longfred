//! Manual command-station IP:port entry.

use longfred_proto::command::Protocol;

use super::helpers::default_server_digits;
use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, ScreenId};
use crate::screen::{KeyBindings, Screen};
use crate::view::UiView;
use crate::widgets::{KeyboardMode, TextKeyboard, format_grouped_ip};

pub struct ServerEntryScreen {
    kbd: TextKeyboard<17>,
}

impl ServerEntryScreen {
    /// 12 IP digits + 5 port digits.
    pub fn new() -> Self {
        Self {
            kbd: TextKeyboard::new(KeyboardMode::Digits),
        }
    }
}

impl Default for ServerEntryScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for ServerEntryScreen {
    fn id(&self) -> ScreenId {
        ScreenId::ServerEntry
    }

    fn key_bindings(&self, _cx: &ScreenCtx<'_>) -> KeyBindings {
        KeyBindings::TEXT
    }

    /// Prefill session digits, else the default WIT or Z21 endpoint.
    ///
    /// The keyboard is always empty here: [`Router`] reconstructs the screen on enter.
    fn on_enter(&mut self, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        if !cx.session.server_digits.is_empty() {
            self.kbd.load(cx.session.server_digits.as_str());
            return;
        }
        let z21 = !cx.session.server_entry_from_list && cx.session.manual_protocol == Protocol::Z21;
        self.kbd.load(default_server_digits(cx, z21).as_str());
    }

    /// Grouped `aaa.bbb.ccc.ddd:ppppp` preview of the digit buffer.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        g.set(0, cx.s.msg_enter_server_ip, false);
        let formatted = format_grouped_ip(
            self.kbd.buffer.as_str(),
            self.kbd.cursor(),
            self.kbd.slot_char(),
            true,
        );
        g.set(2, formatted.as_str(), false);
        g.set(5, cx.s.hint_wit_entry, false);
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

    /// Type a digit into IP:port.
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

    /// Connect when all 17 digits are filled, then go to throttle.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let _ = self.kbd.ok();
        if self.kbd.buffer.len() == 17 {
            cx.session.server_digits.clear();
            let _ = cx.session.server_digits.push_str(self.kbd.buffer.as_str());
            nav.emit(Intent::ServerManual);
            nav.root(ScreenId::Throttle);
        }
    }

    /// Keep typed digits in the session and pop.
    fn on_cancel(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let _ = self.kbd.ok();
        cx.session.server_digits.clear();
        let _ = cx.session.server_digits.push_str(self.kbd.buffer.as_str());
        cx.session.server_entry_from_list = false;
        nav.back();
    }
}
