//! Direct command list (function / next throttle / estop / …).

use longfred_proto::action::Action;

use super::helpers::{height, page_list, step_list};
use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::screen::{KeyBindings, Screen};
use crate::view::{UiView, fill_list_page};
use crate::widgets::PagedList;

pub struct DirectCommandsScreen {
    list: PagedList,
}

impl DirectCommandsScreen {
    /// Unnumbered direct-command list (hash-key path from throttle).
    pub fn new() -> Self {
        Self {
            list: PagedList::new(false),
        }
    }

    /// Localized labels for the six command rows.
    fn labels(cx: &ScreenCtx<'_>) -> [&'static str; 6] {
        [
            cx.s.direct_fn,
            cx.s.direct_next_thr,
            cx.s.direct_spd_mult,
            cx.s.direct_rev,
            cx.s.direct_estop,
            cx.s.direct_back,
        ]
    }
}

impl Default for DirectCommandsScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for DirectCommandsScreen {
    fn id(&self) -> ScreenId {
        ScreenId::DirectCommands
    }

    fn key_bindings(&self, _cx: &ScreenCtx<'_>) -> KeyBindings {
        KeyBindings::NAVIGATION
    }

    /// Paged command list without 1-based numbering.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        let labels = Self::labels(cx);
        fill_list_page(
            &mut g,
            &labels,
            self.list.page,
            self.list.cursor,
            false,
            height(cx),
        );
        UiView::Grid(g)
    }

    /// Move the highlighted command row.
    fn on_list_step(&mut self, d: Step, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        step_list(&mut self.list, d, &labels, false, height(cx));
    }

    /// Page the command list.
    fn on_page(&mut self, d: PageDir, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        page_list(&mut self.list, d, &labels, false, height(cx));
    }

    /// Digit jumps to that row and runs it.
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

    /// Run the highlighted command, or "Back" → throttle. Last row does not emit an action.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        let idx = self.list.global_index(&labels, false, height(cx));
        let actions = [
            Action::Function(0),
            Action::NextThrottle,
            Action::SpeedMultiplier,
            Action::DirectionReverse,
            Action::EStop,
            Action::None,
        ];
        if idx == 5 {
            nav.root(ScreenId::Throttle);
            return;
        }
        if let Some(a) = actions.get(idx) {
            if *a != Action::None {
                nav.emit(Intent::Action(*a));
            }
        }
    }

    /// Skip Menu and return to throttle.
    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.root(ScreenId::Throttle);
    }
}
