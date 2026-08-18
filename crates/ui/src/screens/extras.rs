//! Extras menu.

use longfred_proto::action::Action;
use longfred_proto::persist::RosterMode;

use super::helpers::{
    digit_key, height, list_digit, list_star_confirms, next_throttle_count, overlay_prefixed_count,
    page_list, step_list,
};
use crate::context::ScreenCtx;
use crate::i18n::Strings;
use crate::intent::Intent;
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::screen::{KeyBindings, Screen};
use crate::session::ChoiceKind;
use crate::view::UiView;
use crate::widgets::PagedList;

#[derive(Clone, Copy)]
enum ExtrasItem {
    NetConfig,
    ServerManual,
    Device,
    HashFunctions,
    DeadManSwitch,
    ThrottlesPlus,
    ThrottlesMinus,
    Sleep,
    OneLoco,
    Language,
    RosterSource,
    Firmware,
    Diagnostics,
}

impl ExtrasItem {
    const ALL: [Self; 13] = [
        Self::NetConfig,
        Self::ServerManual,
        Self::Device,
        Self::HashFunctions,
        Self::DeadManSwitch,
        Self::ThrottlesPlus,
        Self::ThrottlesMinus,
        Self::Sleep,
        Self::OneLoco,
        Self::Language,
        Self::RosterSource,
        Self::Firmware,
        Self::Diagnostics,
    ];

    fn label(self, s: &Strings, roster_mode: RosterMode) -> &'static str {
        match self {
            Self::NetConfig => s.extras_net_config,
            Self::ServerManual => s.extras_server_manual,
            Self::Device => s.extras_device,
            Self::HashFunctions => s.extras_fnc_key_tgl,
            Self::DeadManSwitch => s.extras_dead_man_tgl,
            Self::ThrottlesPlus => s.extras_throttles_plus,
            Self::ThrottlesMinus => s.extras_throttles_minus,
            Self::Sleep => s.extras_off_sleep,
            Self::OneLoco => s.extras_one_loco_tgl,
            Self::Language => s.extras_language,
            Self::RosterSource => match roster_mode {
                RosterMode::Auto => s.extras_roster_auto,
                RosterMode::Static => s.extras_roster_static,
                RosterMode::AddressOnly => s.extras_roster_address,
            },
            Self::Firmware => s.extras_firmware,
            Self::Diagnostics => s.extras_diag,
        }
    }

    fn activate(self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        match self {
            Self::NetConfig => nav.go(ScreenId::IpConfig),
            Self::ServerManual => {
                cx.session.server_digits.clear();
                cx.session.server_entry_from_list = false;
                nav.go(ScreenId::ServerProto);
            }
            Self::Device => nav.go(ScreenId::Device),
            Self::HashFunctions => {
                nav.emit(Intent::HashFunctionsToggle);
                let on = !cx.session.hash_functions;
                nav.overlay(if on {
                    cx.s.overlay_fn_list
                } else {
                    cx.s.overlay_fn_direct
                });
                nav.root(ScreenId::Throttle);
            }
            Self::DeadManSwitch => {
                cx.session.choice = ChoiceKind::DeadMan;
                nav.go(ScreenId::Choice);
            }
            Self::ThrottlesPlus => {
                nav.emit(Intent::Action(Action::MaxThrottleIncrease));
                let n = next_throttle_count(cx.drive.max_throttles, true);
                nav.overlay(overlay_prefixed_count(cx.s.overlay_throttles, n).as_str());
                nav.root(ScreenId::Throttle);
            }
            Self::ThrottlesMinus => {
                nav.emit(Intent::Action(Action::MaxThrottleDecrease));
                let n = next_throttle_count(cx.drive.max_throttles, false);
                nav.overlay(overlay_prefixed_count(cx.s.overlay_throttles, n).as_str());
                nav.root(ScreenId::Throttle);
            }
            Self::Sleep => {
                nav.emit(Intent::Sleep);
                nav.root(ScreenId::Throttle);
            }
            Self::OneLoco => {
                nav.emit(Intent::DropBeforeAcquireToggle);
                let on = !cx.drive.drop_before_acquire;
                nav.overlay(if on {
                    cx.s.overlay_one_loco_on
                } else {
                    cx.s.overlay_one_loco_off
                });
                nav.root(ScreenId::Throttle);
            }
            Self::Language => nav.go(ScreenId::Language),
            Self::RosterSource => {
                cx.session.choice = ChoiceKind::RosterSource;
                nav.go(ScreenId::Choice);
            }
            Self::Firmware => nav.go(ScreenId::FirmwareUpdate),
            Self::Diagnostics => nav.go(ScreenId::Diagnostics),
        }
    }
}

/// Extras menu.
pub struct ExtrasScreen {
    list: PagedList,
}

impl ExtrasScreen {
    /// Numbered extras list (settings + toggles).
    #[must_use]
    pub fn new() -> Self {
        Self {
            list: PagedList::new(true),
        }
    }

    fn labels(cx: &ScreenCtx<'_>) -> [&'static str; 13] {
        ExtrasItem::ALL.map(|item| item.label(cx.s, cx.drive.persist.roster_mode))
    }

    fn current_at(&self, labels: &[&str], h: u16) -> Option<ExtrasItem> {
        ExtrasItem::ALL
            .get(self.list.global_index(labels, h))
            .copied()
    }

    fn current(&self, cx: &ScreenCtx<'_>) -> Option<ExtrasItem> {
        self.current_at(&Self::labels(cx), height(cx))
    }
}

impl Default for ExtrasScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for ExtrasScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Extras
    }

    fn key_bindings(&self, _cx: &ScreenCtx<'_>) -> KeyBindings {
        KeyBindings::NAVIGATION
    }

    /// Title plus a numbered paged extras list.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        let labels = Self::labels(cx);
        self.list
            .draw(&mut g, Some(cx.s.menu_extras), &labels, height(cx));
        UiView::Grid(g)
    }

    /// Move the highlighted extras row.
    fn on_list_step(&mut self, d: Step, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        step_list(&mut self.list, d, &labels, height(cx));
    }

    /// Page the extras list.
    fn on_page(&mut self, d: PageDir, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        page_list(&mut self.list, d, &labels, height(cx));
    }

    /// Digit jumps to that row and selects it; in `*` mode digits build a 1-based index.
    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let Some(d) = digit_key(c) else { return };
        let labels = Self::labels(cx);
        let h = height(cx);
        if list_digit(&mut self.list, d, &labels, h).is_some()
            && let Some(item) = self.current_at(&labels, h)
        {
            item.activate(cx, nav);
        }
    }

    /// `*` starts, cancels, or confirms a typed row number.
    fn on_star(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        let h = height(cx);
        if list_star_confirms(&mut self.list, &labels, h)
            && let Some(item) = self.current_at(&labels, h)
        {
            item.activate(cx, nav);
        }
    }

    /// Shift+F selects that 1-based row (`Shift`+F4 → 13).
    fn on_fn_key(&mut self, k: u8, down: bool, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if !down {
            return;
        }
        let labels = Self::labels(cx);
        let h = height(cx);
        if self.list.select_fn_key(k, &labels, h).is_some() {
            let _ = self.list.clear_index();
            if let Some(item) = self.current_at(&labels, h) {
                item.activate(cx, nav);
            }
        }
    }

    /// Open a settings screen, or emit a toggle and return to throttle.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let _ = self.list.clear_index();
        if let Some(item) = self.current(cx) {
            item.activate(cx, nav);
        }
    }

    /// Back leaves extras unless `*` index-entry is active.
    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if self.list.clear_index() {
            return;
        }
        nav.back();
    }
}
