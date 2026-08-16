//! Extras menu.

use longfred_proto::action::Action;

use super::helpers::{height, page_list, step_list};
use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::screen::{KeyBindings, Screen};
use crate::view::UiView;
use crate::widgets::PagedList;

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

    /// Localized labels for all extras rows.
    fn labels(cx: &ScreenCtx<'_>) -> [&'static str; 11] {
        [
            cx.s.extras_net_config,
            cx.s.extras_device,
            cx.s.extras_fnc_key_tgl,
            cx.s.extras_heartbt_tgl,
            cx.s.extras_throttles_plus,
            cx.s.extras_throttles_minus,
            cx.s.extras_off_sleep,
            cx.s.extras_one_loco_tgl,
            cx.s.extras_language,
            cx.s.extras_firmware,
            cx.s.extras_diag,
        ]
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
        let labels = Self::labels(cx);
        match self.list.global_index(&labels, false, height(cx)) {
            0 => nav.go(ScreenId::IpConfig),
            1 => nav.go(ScreenId::Device),
            2 => {
                nav.emit(Intent::HashFunctionsToggle);
                nav.root(ScreenId::Throttle);
            }
            3 => {
                nav.emit(Intent::HeartbeatToggle);
                nav.root(ScreenId::Throttle);
            }
            4 => {
                nav.emit(Intent::Action(Action::MaxThrottleIncrease));
                nav.root(ScreenId::Throttle);
            }
            5 => {
                nav.emit(Intent::Action(Action::MaxThrottleDecrease));
                nav.root(ScreenId::Throttle);
            }
            6 => {
                nav.emit(Intent::Sleep);
                nav.root(ScreenId::Throttle);
            }
            7 => {
                nav.emit(Intent::DropBeforeAcquireToggle);
                nav.root(ScreenId::Throttle);
            }
            8 => nav.go(ScreenId::Language),
            9 => nav.go(ScreenId::FirmwareUpdate),
            10 => nav.go(ScreenId::Diagnostics),
            _ => {}
        }
    }
}
