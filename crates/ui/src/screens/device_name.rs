//! Device name editor.

use longfred_proto::persist::MAX_DEVICE_NAME_LEN;

use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, ScreenId};
use crate::screen::{KeyBindings, Screen};
use crate::view::UiView;
use crate::widgets::{KeyboardMode, TextKeyboard};

/// Device name editor.
pub struct DeviceNameEditScreen {
    kbd: TextKeyboard<MAX_DEVICE_NAME_LEN>,
}

impl DeviceNameEditScreen {
    /// Text keyboard capped at [`MAX_DEVICE_NAME_LEN`].
    #[must_use]
    pub fn new() -> Self {
        let mut kbd = TextKeyboard::new(KeyboardMode::Text);
        kbd.set_max_len(MAX_DEVICE_NAME_LEN);
        Self { kbd }
    }
}

impl Default for DeviceNameEditScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for DeviceNameEditScreen {
    fn id(&self) -> ScreenId {
        ScreenId::DeviceNameEdit
    }

    fn key_bindings(&self, _cx: &ScreenCtx<'_>) -> KeyBindings {
        KeyBindings::TEXT
    }

    /// Prefill from the session device draft.
    fn on_enter(&mut self, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        self.kbd.load(cx.session.device.name.as_str());
    }

    /// Name preview (with caps hint on keypad boards).
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        g.set(0, cx.s.msg_device_name_edit, false);
        g.set(2, self.kbd.preview().as_str(), false);
        g.set(5, cx.s.hint_device_name_edit, false);
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

    /// Toggle uppercase.
    fn on_case_toggle(&mut self, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let _ = self.kbd.case_toggle();
    }

    /// Multitap digit into the name.
    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        if c.is_ascii_digit() {
            let _ = self.kbd.key_press(c as u8 - b'0', cx.now_ms);
        }
    }

    /// Hardware Fn keys type into the name.
    fn on_fn_key(&mut self, k: u8, down: bool, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        if down {
            let _ = self.kbd.fn_press(k, cx.now_ms);
        }
    }

    /// Commit pending multitap.
    fn on_tick(&mut self, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        self.kbd.tick(cx.now_ms);
    }

    /// Save the name and replace back to the device summary.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let _ = self.kbd.ok();
        cx.session.device.name.clear();
        let _ = cx.session.device.name.push_str(self.kbd.buffer.as_str());
        nav.emit(Intent::SaveDevice(cx.session.device.clone()));
        nav.replace(ScreenId::Device);
    }
}
