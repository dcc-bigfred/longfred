//! Radio / roaming settings list: 802.11k/v/r toggles + roaming/IP-pin parameters.
//!
//! Boolean fields toggle in place (with an overlay); numeric fields open
//! [`crate::nav::ScreenId::RadioEdit`]. Backing out persists the whole draft
//! via [`crate::intent::Intent::SaveRadio`].

use core::fmt::Write as _;

use longfred_proto::persist::RadioConfig;

use super::helpers::{
    digit_key, height, list_digit, list_star_confirms, page_list, set_list_hint, step_list,
};
use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::screen::{KeyBindings, Screen};
use crate::session::RadioField;
use crate::view::UiView;
use crate::widgets::PagedList;

/// One row of the radio settings list.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RadioItem {
    Roam,
    Rrm,
    Btm,
    Ft,
    PowerSaveOff,
    Enable11ax,
    RssiThreshold,
    HysteresisDb,
    DebounceSamples,
    ScanIntervalS,
    SampleMs,
    IpPinning,
    PinMaxGapS,
    DhcpDiscoverTimeoutS,
}

impl RadioItem {
    const ALL: [Self; 14] = [
        Self::Roam,
        Self::Rrm,
        Self::Btm,
        Self::Ft,
        Self::PowerSaveOff,
        Self::Enable11ax,
        Self::RssiThreshold,
        Self::HysteresisDb,
        Self::DebounceSamples,
        Self::ScanIntervalS,
        Self::SampleMs,
        Self::IpPinning,
        Self::PinMaxGapS,
        Self::DhcpDiscoverTimeoutS,
    ];

    #[must_use]
    const fn name(self) -> &'static str {
        match self {
            Self::Roam => "Roam",
            Self::Rrm => "11k",
            Self::Btm => "11v",
            Self::Ft => "11r",
            Self::PowerSaveOff => "PS off",
            Self::Enable11ax => "11ax",
            Self::RssiThreshold => "RSSI",
            Self::HysteresisDb => "Hysteresis",
            Self::DebounceSamples => "Debounce",
            Self::ScanIntervalS => "Scan int",
            Self::SampleMs => "Sample",
            Self::IpPinning => "IP pin",
            Self::PinMaxGapS => "Pin gap",
            Self::DhcpDiscoverTimeoutS => "DHCP tmo",
        }
    }

    #[must_use]
    const fn is_bool(self) -> bool {
        matches!(
            self,
            Self::Roam
                | Self::Rrm
                | Self::Btm
                | Self::Ft
                | Self::PowerSaveOff
                | Self::Enable11ax
                | Self::IpPinning
        )
    }

    #[must_use]
    const fn field(self) -> Option<RadioField> {
        match self {
            Self::RssiThreshold => Some(RadioField::RssiThreshold),
            Self::HysteresisDb => Some(RadioField::HysteresisDb),
            Self::DebounceSamples => Some(RadioField::DebounceSamples),
            Self::ScanIntervalS => Some(RadioField::ScanIntervalS),
            Self::SampleMs => Some(RadioField::SampleMs),
            Self::PinMaxGapS => Some(RadioField::PinMaxGapS),
            Self::DhcpDiscoverTimeoutS => Some(RadioField::DhcpDiscoverTimeoutS),
            _ => None,
        }
    }

    /// Flip the underlying boolean and return the new state.
    fn toggle(self, cfg: &mut RadioConfig) -> bool {
        match self {
            Self::Roam => {
                cfg.roam_enabled = !cfg.roam_enabled;
                cfg.roam_enabled
            }
            Self::Rrm => {
                cfg.rrm_enabled = !cfg.rrm_enabled;
                cfg.rrm_enabled
            }
            Self::Btm => {
                cfg.btm_enabled = !cfg.btm_enabled;
                cfg.btm_enabled
            }
            Self::Ft => {
                cfg.ft_enabled = !cfg.ft_enabled;
                cfg.ft_enabled
            }
            Self::PowerSaveOff => {
                cfg.power_save_off = !cfg.power_save_off;
                cfg.power_save_off
            }
            Self::Enable11ax => {
                cfg.enable_11ax = !cfg.enable_11ax;
                cfg.enable_11ax
            }
            Self::IpPinning => {
                cfg.ip_pinning = !cfg.ip_pinning;
                cfg.ip_pinning
            }
            // Numeric rows are never toggled.
            _ => false,
        }
    }
}

const ON_OFF: [&str; 2] = ["off", "on"];

/// Build the 14 dynamic row labels (`"<name> <value>"`) from the draft config.
fn radio_labels(cfg: &RadioConfig) -> [heapless::String<16>; 14] {
    let mut out: [heapless::String<16>; 14] = Default::default();
    let b = |v: bool| ON_OFF[usize::from(v)];
    let _ = write!(out[0], "Roam {}", b(cfg.roam_enabled));
    let _ = write!(out[1], "11k {}", b(cfg.rrm_enabled));
    let _ = write!(out[2], "11v {}", b(cfg.btm_enabled));
    let _ = write!(out[3], "11r {}", b(cfg.ft_enabled));
    let _ = write!(out[4], "PS {}", b(cfg.power_save_off));
    let _ = write!(out[5], "11ax {}", b(cfg.enable_11ax));
    let _ = write!(out[6], "RSSI {}", cfg.roam_rssi_threshold);
    let _ = write!(out[7], "Hyst {}", cfg.roam_hysteresis_db);
    let _ = write!(out[8], "Deb {}", cfg.roam_debounce_samples);
    let _ = write!(out[9], "Scan {}", cfg.roam_scan_interval_s);
    let _ = write!(out[10], "Samp {}", cfg.roam_sample_ms);
    let _ = write!(out[11], "IPpin {}", b(cfg.ip_pinning));
    let _ = write!(out[12], "Gap {}", cfg.ip_pin_max_gap_s);
    let _ = write!(out[13], "DHCP {}", cfg.dhcp_discover_timeout_s);
    out
}

/// Borrow the 14 labels as `&[&str]` for the paged-list API.
fn label_refs(buf: &[heapless::String<16>; 14]) -> [&str; 14] {
    core::array::from_fn(|i| buf[i].as_str())
}

/// Radio / roaming settings list opened from Wi-Fi settings.
pub struct RadioSettingsScreen {
    list: PagedList,
}

impl RadioSettingsScreen {
    /// Numbered 14-row radio / roaming settings list.
    #[must_use]
    pub fn new() -> Self {
        Self {
            list: PagedList::new(true).with_footer(true),
        }
    }

    fn current_at(&self, labels: &[&str], h: u16) -> Option<RadioItem> {
        RadioItem::ALL
            .get(self.list.global_index(labels, h))
            .copied()
    }

    fn current(&self, cx: &ScreenCtx<'_>) -> Option<RadioItem> {
        let buf = radio_labels(&cx.session.radio_cfg);
        let labels = label_refs(&buf);
        self.current_at(&labels, height(cx))
    }

    fn activate(item: RadioItem, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if item.is_bool() {
            let on = item.toggle(&mut cx.session.radio_cfg);
            let mut msg = heapless::String::<16>::new();
            let _ = write!(msg, "{} {}", item.name(), ON_OFF[usize::from(on)]);
            nav.overlay(msg.as_str());
        } else if let Some(field) = item.field() {
            cx.session.radio_field = field;
            nav.go(ScreenId::RadioEdit);
        }
    }
}

impl Default for RadioSettingsScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for RadioSettingsScreen {
    fn id(&self) -> ScreenId {
        ScreenId::RadioSettings
    }

    fn key_bindings(&self, _cx: &ScreenCtx<'_>) -> KeyBindings {
        KeyBindings::NAVIGATION
    }

    /// Reset the cursor; the draft is loaded by the Wi-Fi settings row that
    /// opens this screen, so it survives a round-trip through `RadioEdit`.
    fn on_enter(&mut self, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        self.list.reset();
    }

    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let buf = radio_labels(&cx.session.radio_cfg);
        let labels = label_refs(&buf);
        let mut g = crate::view::GridView::new();
        self.list
            .draw(&mut g, Some(cx.s.msg_radio), &labels, height(cx));
        set_list_hint(&mut g, cx, cx.s.hint_radio);
        UiView::Grid(g)
    }

    fn on_list_step(&mut self, d: Step, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let buf = radio_labels(&cx.session.radio_cfg);
        let labels = label_refs(&buf);
        step_list(&mut self.list, d, &labels, height(cx));
    }

    fn on_page(&mut self, d: PageDir, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let buf = radio_labels(&cx.session.radio_cfg);
        let labels = label_refs(&buf);
        page_list(&mut self.list, d, &labels, height(cx));
    }

    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let Some(d) = digit_key(c) else { return };
        let buf = radio_labels(&cx.session.radio_cfg);
        let labels = label_refs(&buf);
        let h = height(cx);
        if list_digit(&mut self.list, d, &labels, h).is_some()
            && let Some(item) = self.current_at(&labels, h)
        {
            Self::activate(item, cx, nav);
        }
    }

    fn on_star(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let buf = radio_labels(&cx.session.radio_cfg);
        let labels = label_refs(&buf);
        let h = height(cx);
        if list_star_confirms(&mut self.list, &labels, h)
            && let Some(item) = self.current_at(&labels, h)
        {
            Self::activate(item, cx, nav);
        }
    }

    fn on_fn_key(&mut self, k: u8, down: bool, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if !down {
            return;
        }
        let buf = radio_labels(&cx.session.radio_cfg);
        let labels = label_refs(&buf);
        let h = height(cx);
        if self.list.select_fn_key(k, &labels, h).is_some() {
            let _ = self.list.clear_index();
            if let Some(item) = self.current_at(&labels, h) {
                Self::activate(item, cx, nav);
            }
        }
    }

    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let _ = self.list.clear_index();
        if let Some(item) = self.current(cx) {
            Self::activate(item, cx, nav);
        }
    }

    /// Persist the draft and return to Wi-Fi settings.
    fn on_cancel(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if self.list.clear_index() {
            return;
        }
        nav.emit(Intent::SaveRadio(cx.session.radio_cfg.clamped()));
        nav.overlay(cx.s.saved_radio);
        nav.back();
    }
}
