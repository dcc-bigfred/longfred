//! Extras menu.

use longfred_proto::action::Action;

use super::helpers::{height, page_list, step_list};
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
    Device,
    HashFunctions,
    Heartbeat,
    ThrottlesPlus,
    ThrottlesMinus,
    Sleep,
    OneLoco,
    Language,
    Firmware,
    Diagnostics,
}

impl ExtrasItem {
    const ALL: [Self; 11] = [
        Self::NetConfig,
        Self::Device,
        Self::HashFunctions,
        Self::Heartbeat,
        Self::ThrottlesPlus,
        Self::ThrottlesMinus,
        Self::Sleep,
        Self::OneLoco,
        Self::Language,
        Self::Firmware,
        Self::Diagnostics,
    ];

    fn label(self, s: &Strings) -> &'static str {
        match self {
            Self::NetConfig => s.extras_net_config,
            Self::Device => s.extras_device,
            Self::HashFunctions => s.extras_fnc_key_tgl,
            Self::Heartbeat => s.extras_heartbt_tgl,
            Self::ThrottlesPlus => s.extras_throttles_plus,
            Self::ThrottlesMinus => s.extras_throttles_minus,
            Self::Sleep => s.extras_off_sleep,
            Self::OneLoco => s.extras_one_loco_tgl,
            Self::Language => s.extras_language,
            Self::Firmware => s.extras_firmware,
            Self::Diagnostics => s.extras_diag,
        }
    }

    fn activate(self, nav: &mut Nav<'_>) {
        match self {
            Self::NetConfig => nav.go(ScreenId::IpConfig),
            Self::Device => nav.go(ScreenId::Device),
            Self::HashFunctions => {
                nav.emit(Intent::HashFunctionsToggle);
                nav.root(ScreenId::Throttle);
            }
            Self::Heartbeat => {
                nav.emit(Intent::HeartbeatToggle);
                nav.root(ScreenId::Throttle);
            }
            Self::ThrottlesPlus => {
                nav.emit(Intent::Action(Action::MaxThrottleIncrease));
                nav.root(ScreenId::Throttle);
            }
            Self::ThrottlesMinus => {
                nav.emit(Intent::Action(Action::MaxThrottleDecrease));
                nav.root(ScreenId::Throttle);
            }
            Self::Sleep => {
                nav.emit(Intent::Sleep);
                nav.root(ScreenId::Throttle);
            }
            Self::OneLoco => {
                nav.emit(Intent::DropBeforeAcquireToggle);
                nav.root(ScreenId::Throttle);
            }
            Self::Language => nav.go(ScreenId::Language),
            Self::Firmware => nav.go(ScreenId::FirmwareUpdate),
            Self::Diagnostics => nav.go(ScreenId::Diagnostics),
        }
    }
}

pub struct ExtrasScreen {
    list: PagedList,
}

impl ExtrasScreen {
    /// Unnumbered extras list (settings + toggles).
    pub fn new() -> Self {
        Self {
            list: PagedList::new(false),
        }
    }

    fn labels(cx: &ScreenCtx<'_>) -> [&'static str; 11] {
        ExtrasItem::ALL.map(|item| item.label(cx.s))
    }

    fn current(&self, cx: &ScreenCtx<'_>) -> Option<ExtrasItem> {
        let labels = Self::labels(cx);
        let idx = self.list.global_index(&labels, false, height(cx));
        ExtrasItem::ALL.get(idx).copied()
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
        crate::view::fill_list_page(
            &mut g,
            &labels,
            self.list.page,
            self.list.cursor,
            false,
            height(cx),
        );
        UiView::Grid(g)
    }

    /// Move the highlighted extras row.
    fn on_list_step(&mut self, d: Step, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        step_list(&mut self.list, d, &labels, false, height(cx));
    }

    /// Page the extras list.
    fn on_page(&mut self, d: PageDir, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        page_list(&mut self.list, d, &labels, false, height(cx));
    }

    /// Digit jumps to that row and selects it.
    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if let Some(d) = c.to_digit(10) {
            let labels = Self::labels(cx);
            if self
                .list
                .select_digit(d as u8, &labels, false, height(cx))
                .is_some()
            {
                self.on_select(cx, nav);
            }
        }
    }

    /// Open a settings screen, or emit a toggle and return to throttle.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if let Some(item) = self.current(cx) {
            item.activate(nav);
        }
    }
}
