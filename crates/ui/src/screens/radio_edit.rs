//! Numeric editor for one [`crate::session::RadioField`] (roaming/IP-pin params).
//!
//! RSSI threshold is entered as a magnitude and stored negative; all other
//! fields are unsigned. The draft lives in [`crate::session::UiSession::radio_cfg`]
//! and is persisted when the radio list is closed.

use core::fmt::Write as _;

use longfred_proto::persist::RadioConfig;

use crate::context::ScreenCtx;
use crate::nav::{Nav, ScreenId};
use crate::screen::{KeyBindings, Screen};
use crate::session::RadioField;
use crate::view::UiView;
use crate::widgets::{KeyboardMode, TextKeyboard};

/// Numeric editor for one radio field.
pub struct RadioEditScreen {
    kbd: TextKeyboard<4>,
}

impl RadioEditScreen {
    /// Numeric keyboard editor for the currently-selected radio field.
    #[must_use]
    pub fn new() -> Self {
        Self {
            kbd: TextKeyboard::new(KeyboardMode::Digits),
        }
    }
}

impl Default for RadioEditScreen {
    fn default() -> Self {
        Self::new()
    }
}

/// Current value of `field` as digit characters (RSSI as a magnitude).
fn load_field_digits(field: RadioField, cfg: &RadioConfig) -> heapless::String<4> {
    let mut s = heapless::String::new();
    match field {
        RadioField::RssiThreshold => {
            let _ = write!(s, "{}", cfg.roam_rssi_threshold.unsigned_abs());
        }
        RadioField::HysteresisDb => {
            let _ = write!(s, "{}", cfg.roam_hysteresis_db);
        }
        RadioField::DebounceSamples => {
            let _ = write!(s, "{}", cfg.roam_debounce_samples);
        }
        RadioField::ScanIntervalS => {
            let _ = write!(s, "{}", cfg.roam_scan_interval_s);
        }
        RadioField::SampleMs => {
            let _ = write!(s, "{}", cfg.roam_sample_ms);
        }
        RadioField::PinMaxGapS => {
            let _ = write!(s, "{}", cfg.ip_pin_max_gap_s);
        }
        RadioField::DhcpDiscoverTimeoutS => {
            let _ = write!(s, "{}", cfg.dhcp_discover_timeout_s);
        }
    }
    s
}

/// Parse `digits` into a `u32` (empty / non-digit → `None`).
fn parse_u32(digits: &str) -> Option<u32> {
    if digits.is_empty() {
        return None;
    }
    let mut n = 0u32;
    for b in digits.as_bytes() {
        if !b.is_ascii_digit() {
            return None;
        }
        n = n.saturating_mul(10).saturating_add(u32::from(b - b'0'));
    }
    Some(n)
}

/// Write the parsed (and clamped) value back into the draft config.
fn commit_field(field: RadioField, cfg: &mut RadioConfig, digits: &str) {
    let Some(n) = parse_u32(digits) else {
        return;
    };
    match field {
        RadioField::RssiThreshold => {
            let mag = n.clamp(50, 90).min(u8::MAX as u32) as u8;
            cfg.roam_rssi_threshold = -(mag as i8);
        }
        RadioField::HysteresisDb => {
            cfg.roam_hysteresis_db = n.clamp(3, 20).min(u8::MAX as u32) as u8;
        }
        RadioField::DebounceSamples => {
            cfg.roam_debounce_samples = n.clamp(1, 10).min(u8::MAX as u32) as u8;
        }
        RadioField::ScanIntervalS => {
            cfg.roam_scan_interval_s = n.clamp(1, 60).min(u8::MAX as u32) as u8;
        }
        RadioField::SampleMs => {
            cfg.roam_sample_ms = n.clamp(100, 2000).min(u16::MAX as u32) as u16;
        }
        RadioField::PinMaxGapS => {
            cfg.ip_pin_max_gap_s = n.clamp(5, 3600).min(u16::MAX as u32) as u16;
        }
        RadioField::DhcpDiscoverTimeoutS => {
            cfg.dhcp_discover_timeout_s = n.clamp(1, 30).min(u8::MAX as u32) as u8;
        }
    }
}

impl Screen for RadioEditScreen {
    fn id(&self) -> ScreenId {
        ScreenId::RadioEdit
    }

    fn key_bindings(&self, _cx: &ScreenCtx<'_>) -> KeyBindings {
        KeyBindings::TEXT
    }

    /// Prefill the current field value and limit the buffer to its width.
    fn on_enter(&mut self, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let field = cx.session.radio_field;
        self.kbd.clear();
        self.kbd.set_max_len(field.max_digits());
        let digits = load_field_digits(field, &cx.session.radio_cfg);
        self.kbd.load(digits.as_str());
    }

    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let field = cx.session.radio_field;
        let mut title = heapless::String::<16>::new();
        let _ = title.push_str(field.label());
        if field.is_signed() {
            // Hint that the stored value is negative.
            let _ = title.push_str(" -");
        }
        let mut g = crate::view::GridView::new();
        g.set(0, title.as_str(), false);
        g.set(2, self.kbd.preview().as_str(), false);
        g.set(5, cx.s.hint_radio_edit, false);
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

    /// Commit the parsed value to the draft and return to the radio list.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let _ = self.kbd.ok();
        let field = cx.session.radio_field;
        commit_field(field, &mut cx.session.radio_cfg, self.kbd.buffer.as_str());
        nav.back();
    }
}
