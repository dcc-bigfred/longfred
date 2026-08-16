//! Main menu (Function / Locos / Speed / Power / Extras).

use longfred_proto::action::Action;

use super::helpers::page_list;
use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::screen::{KeyBindings, Screen};
use crate::view::UiView;
use crate::widgets::PagedList;

pub struct MenuScreen {
    list: PagedList,
}

impl MenuScreen {
    /// Numbered five-item main menu.
    pub fn new() -> Self {
        Self {
            list: PagedList::new(true),
        }
    }

    /// Localized labels for the five menu rows.
    fn labels(cx: &ScreenCtx<'_>) -> [&'static str; 5] {
        [
            cx.s.menu_fn,
            cx.s.menu_locos,
            cx.s.menu_speed_mult,
            cx.s.menu_power,
            cx.s.menu_extras,
        ]
    }

    /// Display height used for paging.
    fn height(cx: &ScreenCtx<'_>) -> u16 {
        cx.env.geometry.height
    }
}

impl Default for MenuScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for MenuScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Menu
    }

    fn key_bindings(&self, _cx: &ScreenCtx<'_>) -> KeyBindings {
        KeyBindings::NAVIGATION
    }

    /// Numbered list with app name as title and a footer hint.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        let labels = Self::labels(cx);
        self.list.draw(
            &mut g,
            Some(cx.env.app_name),
            &labels,
            true,
            Self::height(cx),
        );
        g.set(5, cx.s.hint_menu, false);
        UiView::Grid(g)
    }

    /// Move the highlighted menu row.
    fn on_list_step(&mut self, d: Step, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        match d {
            Step::Prev => self.list.list_prev(&labels, true, Self::height(cx)),
            Step::Next => self.list.list_next(&labels, true, Self::height(cx)),
        }
    }

    /// Digit 1–5 jumps to that row and selects it.
    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if let Some(d) = c.to_digit(10) {
            let labels = Self::labels(cx);
            if self
                .list
                .select_digit(d as u8, &labels, true, Self::height(cx))
                .is_some()
            {
                self.on_select(cx, nav);
            }
        }
    }

    /// Open Function/Roster/Extras, or apply speed-mult / power and return to throttle.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        match self.list.global_index(&labels, true, Self::height(cx)) {
            0 => nav.go(ScreenId::FunctionList),
            1 => nav.go(ScreenId::RosterList),
            2 => {
                nav.emit(Intent::Action(Action::SpeedMultiplier));
                nav.root(ScreenId::Throttle);
            }
            3 => {
                nav.emit(Intent::Action(Action::PowerToggle));
                nav.root(ScreenId::Throttle);
            }
            4 => nav.go(ScreenId::Extras),
            _ => {}
        }
    }

    /// Back returns to throttle (menu is not stacked under drive).
    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.root(ScreenId::Throttle);
    }

    /// Page the menu list if it overflows the display.
    fn on_page(&mut self, d: PageDir, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        page_list(&mut self.list, d, &labels, true, Self::height(cx));
    }
}
