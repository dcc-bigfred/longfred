//! Extras menu.

use longfred_proto::action::Action;
use longfred_proto::persist::RosterMode;

use super::helpers::{
    digit_key, height, next_throttle_count, overlay_prefixed_count, page_list, step_list,
};
use crate::context::ScreenCtx;
use crate::i18n::Strings;
use crate::intent::Intent;
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::screen::{KeyBindings, Screen};
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
                nav.emit(Intent::DeadManSwitchToggle);
                let on = !cx.drive.dead_man_switch_on;
                nav.overlay(if on {
                    cx.s.overlay_dead_man_on
                } else {
                    cx.s.overlay_dead_man_off
                });
                nav.root(ScreenId::Throttle);
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
                nav.emit(Intent::SetRosterMode(cx.drive.persist.roster_mode.next()));
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
    /// Unnumbered extras list (settings + toggles).
    #[must_use]
    pub fn new() -> Self {
        Self {
            list: PagedList::new(false),
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

    /// Title plus a paged extras list (no 1-based numbering).
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        g.set(0, cx.s.menu_extras, false);
        let labels = Self::labels(cx);
        crate::view::fill_list_page(&mut g, &labels, &self.list, height(cx));
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

    /// Digit jumps to that row and selects it.
    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let Some(d) = digit_key(c) else { return };
        let labels = Self::labels(cx);
        let h = height(cx);
        if self.list.select_label_digit(d, &labels, h).is_some()
            && let Some(item) = self.current_at(&labels, h)
        {
            item.activate(cx, nav);
        }
    }

    /// Open a settings screen, or emit a toggle and return to throttle.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if let Some(item) = self.current(cx) {
            item.activate(cx, nav);
        }
    }
}
