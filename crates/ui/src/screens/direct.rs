//! Direct command list (function / next throttle / estop / …).

use longfred_proto::action::Action;

use super::helpers::{height, page_list, step_list};
use crate::context::ScreenCtx;
use crate::i18n::Strings;
use crate::intent::Intent;
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::screen::{KeyBindings, Screen};
use crate::view::{UiView, fill_list_page};
use crate::widgets::PagedList;

#[derive(Clone, Copy)]
enum DirectItem {
    Function,
    NextThrottle,
    SpeedMult,
    Reverse,
    EStop,
    Back,
}

impl DirectItem {
    const ALL: [Self; 6] = [
        Self::Function,
        Self::NextThrottle,
        Self::SpeedMult,
        Self::Reverse,
        Self::EStop,
        Self::Back,
    ];

    fn label(self, s: &Strings) -> &'static str {
        match self {
            Self::Function => s.direct_fn,
            Self::NextThrottle => s.direct_next_thr,
            Self::SpeedMult => s.direct_spd_mult,
            Self::Reverse => s.direct_rev,
            Self::EStop => s.direct_estop,
            Self::Back => s.direct_back,
        }
    }

    fn activate(self, nav: &mut Nav<'_>) {
        match self {
            Self::Function => nav.emit(Intent::Action(Action::Function(0))),
            Self::NextThrottle => nav.emit(Intent::Action(Action::NextThrottle)),
            Self::SpeedMult => nav.emit(Intent::Action(Action::SpeedMultiplier)),
            Self::Reverse => nav.emit(Intent::Action(Action::DirectionReverse)),
            Self::EStop => nav.emit(Intent::Action(Action::EStop)),
            Self::Back => nav.root(ScreenId::Throttle),
        }
    }
}

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

    fn labels(cx: &ScreenCtx<'_>) -> [&'static str; 6] {
        DirectItem::ALL.map(|item| item.label(cx.s))
    }

    fn current(&self, cx: &ScreenCtx<'_>) -> Option<DirectItem> {
        let labels = Self::labels(cx);
        let idx = self.list.global_index(&labels, false, height(cx));
        DirectItem::ALL.get(idx).copied()
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

    /// Run the highlighted command, or "Back" → throttle.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if let Some(item) = self.current(cx) {
            item.activate(nav);
        }
    }

    /// Skip Menu and return to throttle.
    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.root(ScreenId::Throttle);
    }
}
