//! Numeric editor for the number of active throttle slots (`1..=9`).

use longfred_proto::action::Action;
use longfred_proto::model::MAX_THROTTLES;

use super::helpers::overlay_count_message;
use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, ScreenId};
use crate::screen::{KeyBindings, Screen};
use crate::view::UiView;
use crate::widgets::{KeyboardMode, TextKeyboard};

/// One-digit slot-count editor opened from Extras.
pub struct SlotCountEditScreen {
    kbd: TextKeyboard<1>,
}

impl SlotCountEditScreen {
    /// Digit keyboard for `1..=MAX_THROTTLES`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            kbd: TextKeyboard::new(KeyboardMode::Digits),
        }
    }
}

impl Default for SlotCountEditScreen {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_slot_count(buf: &str) -> Option<u8> {
    let b = buf.as_bytes().first().copied()?;
    if (b'1'..=b'9').contains(&b) {
        let n = b - b'0';
        (usize::from(n) <= MAX_THROTTLES).then_some(n)
    } else {
        None
    }
}

impl Screen for SlotCountEditScreen {
    fn id(&self) -> ScreenId {
        ScreenId::SlotCountEdit
    }

    fn key_bindings(&self, _cx: &ScreenCtx<'_>) -> KeyBindings {
        KeyBindings::TEXT
    }

    /// Prefill the current slot count as a single digit.
    fn on_enter(&mut self, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        self.kbd.clear();
        const DIGITS: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];
        let n = cx.drive.max_throttles.clamp(1, MAX_THROTTLES);
        self.kbd.load(DIGITS[n - 1]);
    }

    /// Title plus one-digit preview.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        g.set(0, cx.s.msg_slot_count_edit, false);
        g.set(2, self.kbd.preview().as_str(), false);
        g.set(5, cx.s.hint_device_id_edit, false);
        UiView::Grid(g)
    }

    /// Encoder cycles a fresh digit (replaces the prefilled value).
    fn on_char_cycle(&mut self, d: i8, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        if !self.kbd.buffer.is_empty() {
            self.kbd.clear();
        }
        let _ = self.kbd.char_cycle(d, cx.now_ms);
    }

    /// Type a digit, replacing any existing value.
    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        if c.is_ascii_digit() {
            self.kbd.clear();
            let _ = self.kbd.key_press(c as u8 - b'0', cx.now_ms);
        }
    }

    /// Hardware Fn keys type digits, replacing any existing value.
    fn on_fn_key(&mut self, k: u8, down: bool, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        if down {
            self.kbd.clear();
            let _ = self.kbd.fn_press(k, cx.now_ms);
        }
    }

    /// Commit pending multitap.
    fn on_tick(&mut self, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        self.kbd.tick(cx.now_ms);
    }

    /// Save `1..=9` and return to Extras with a confirmation overlay.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let _ = self.kbd.ok();
        let Some(n) = parse_slot_count(self.kbd.buffer.as_str()) else {
            return;
        };
        nav.emit(Intent::Action(Action::SetMaxThrottles(n)));
        let msg = overlay_count_message(
            cx.s.overlay_slots_prefix,
            usize::from(n),
            cx.s.overlay_slots_suffix,
        );
        nav.overlay(msg.as_str());
        nav.back();
    }
}
