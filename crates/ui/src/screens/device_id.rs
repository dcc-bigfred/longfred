//! Device numeric-id editor.

use longfred_proto::persist::{DEVICE_ID_MAX, DEVICE_ID_MIN};

use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, ScreenId};
use crate::screen::{KeyBindings, Screen};
use crate::view::UiView;
use crate::widgets::{KeyboardMode, TextKeyboard};

pub struct DeviceIdEditScreen {
    kbd: TextKeyboard<4>,
}

impl DeviceIdEditScreen {
    /// Four-digit numeric id.
    pub fn new() -> Self {
        Self {
            kbd: TextKeyboard::new(KeyboardMode::Digits),
        }
    }
}

impl Default for DeviceIdEditScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for DeviceIdEditScreen {
    fn id(&self) -> ScreenId {
        ScreenId::DeviceIdEdit
    }

    fn key_bindings(&self, _cx: &ScreenCtx<'_>) -> KeyBindings {
        KeyBindings::TEXT
    }

    /// Prefill a 4-digit id, or empty if none is assigned yet.
    fn on_enter(&mut self, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        self.kbd.clear();
        let id = cx.session.device.id;
        if id >= DEVICE_ID_MIN {
            let mut s = heapless::String::<4>::new();
            let _ = s.push((b'0' + ((id / 1000) % 10) as u8) as char);
            let _ = s.push((b'0' + ((id / 100) % 10) as u8) as char);
            let _ = s.push((b'0' + ((id / 10) % 10) as u8) as char);
            let _ = s.push((b'0' + (id % 10) as u8) as char);
            self.kbd.load(s.as_str());
        }
    }

    /// Four-digit id preview.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        g.set(0, cx.s.msg_device_id_edit, false);
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

    /// Type a digit into the id.
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

    /// Save a 4-digit id in range and replace back to the device summary.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let _ = self.kbd.ok();
        if self.kbd.buffer.len() == 4 {
            let mut id = 0u16;
            for b in self.kbd.buffer.as_bytes() {
                id = id.saturating_mul(10).saturating_add(u16::from(b - b'0'));
            }
            if (DEVICE_ID_MIN..=DEVICE_ID_MAX).contains(&id) {
                cx.session.device.id = id;
                nav.emit(Intent::SaveDevice(cx.session.device.clone()));
                nav.replace(ScreenId::Device);
            }
        }
    }
}
