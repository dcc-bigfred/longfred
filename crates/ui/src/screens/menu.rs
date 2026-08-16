//! Main menu (Function / Locos / Speed / Power / Extras).

use longfred_proto::action::Action;

use super::helpers::{digit_key, page_list};
use crate::context::ScreenCtx;
use crate::i18n::Strings;
use crate::intent::Intent;
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::screen::{KeyBindings, Screen};
use crate::view::UiView;
use crate::widgets::PagedList;

#[derive(Clone, Copy)]
enum MenuItem {
    Function,
    Locos,
    SpeedMult,
    Power,
    Extras,
}

impl MenuItem {
    const ALL: [Self; 5] = [
        Self::Function,
        Self::Locos,
        Self::SpeedMult,
        Self::Power,
        Self::Extras,
    ];

    fn label(self, s: &Strings) -> &'static str {
        match self {
            Self::Function => s.menu_fn,
            Self::Locos => s.menu_locos,
            Self::SpeedMult => s.menu_speed_mult,
            Self::Power => s.menu_power,
            Self::Extras => s.menu_extras,
        }
    }

    fn activate(self, nav: &mut Nav<'_>) {
        match self {
            Self::Function => nav.go(ScreenId::FunctionList),
            Self::Locos => nav.go(ScreenId::RosterList),
            Self::SpeedMult => {
                nav.emit(Intent::Action(Action::SpeedMultiplier));
                nav.root(ScreenId::Throttle);
            }
            Self::Power => {
                nav.emit(Intent::Action(Action::PowerToggle));
                nav.root(ScreenId::Throttle);
            }
            Self::Extras => nav.go(ScreenId::Extras),
        }
    }
}

pub struct MenuScreen {
    list: PagedList,
}

impl MenuScreen {
    /// Numbered five-item main menu.
    #[must_use]
    pub fn new() -> Self {
        Self {
            list: PagedList::new(true),
        }
    }

    fn labels(cx: &ScreenCtx<'_>) -> [&'static str; 5] {
        MenuItem::ALL.map(|item| item.label(cx.s))
    }

    fn height(cx: &ScreenCtx<'_>) -> u16 {
        cx.env.geometry.height
    }

    fn current_at(&self, labels: &[&str], h: u16) -> Option<MenuItem> {
        MenuItem::ALL
            .get(self.list.global_index(labels, h))
            .copied()
    }

    fn current(&self, cx: &ScreenCtx<'_>) -> Option<MenuItem> {
        self.current_at(&Self::labels(cx), Self::height(cx))
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
        self.list
            .draw(&mut g, Some(cx.env.app_name), &labels, Self::height(cx));
        g.set(5, cx.s.hint_menu, false);
        UiView::Grid(g)
    }

    /// Move the highlighted menu row.
    fn on_list_step(&mut self, d: Step, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        match d {
            Step::Prev => self.list.list_prev(&labels, Self::height(cx)),
            Step::Next => self.list.list_next(&labels, Self::height(cx)),
        }
    }

    /// Digit 1–5 jumps to that row and selects it.
    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let Some(d) = digit_key(c) else { return };
        let labels = Self::labels(cx);
        let h = Self::height(cx);
        if self.list.select_digit(d, &labels, h).is_some()
            && let Some(item) = self.current_at(&labels, h)
        {
            item.activate(nav);
        }
    }

    /// Open Function/Roster/Extras, or apply speed-mult / power and return to throttle.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if let Some(item) = self.current(cx) {
            item.activate(nav);
        }
    }

    /// Back returns to throttle (menu is not stacked under drive).
    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.root(ScreenId::Throttle);
    }

    /// Page the menu list if it overflows the display.
    fn on_page(&mut self, d: PageDir, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        page_list(&mut self.list, d, &labels, Self::height(cx));
    }
}
