//! Main menu (Function / Locos or Change DCC address / Speed / Power / Extras).

use longfred_proto::LocoSource;
use longfred_proto::action::Action;

use super::helpers::{
    digit_key, next_speed_multiplier, overlay_prefixed_count, page_list, set_list_hint,
};
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

    fn label(self, s: &Strings, source: LocoSource) -> &'static str {
        match self {
            Self::Function => s.menu_fn,
            Self::Locos => match source {
                LocoSource::AddressOnly => s.menu_change_addr,
                LocoSource::ServerRoster | LocoSource::StaticRoster => s.menu_locos,
            },
            Self::SpeedMult => s.menu_speed_mult,
            Self::Power => s.menu_power,
            Self::Extras => s.menu_extras,
        }
    }

    fn activate(self, cx: &ScreenCtx<'_>, nav: &mut Nav<'_>) {
        match self {
            Self::Function => nav.go(ScreenId::FunctionList),
            Self::Locos => match cx.drive.effective_loco_source {
                LocoSource::AddressOnly => nav.go(ScreenId::AddrEdit),
                LocoSource::ServerRoster | LocoSource::StaticRoster => nav.go(ScreenId::RosterList),
            },
            Self::SpeedMult => {
                nav.emit(Intent::Action(Action::SpeedMultiplier));
                let next = next_speed_multiplier(cx.drive.speed_multiplier);
                nav.overlay(overlay_prefixed_count(cx.s.overlay_speed, usize::from(next)).as_str());
                nav.root(ScreenId::Throttle);
            }
            Self::Power => {
                nav.emit(Intent::Action(Action::PowerToggle));
                let on = !matches!(cx.drive.track_power, longfred_proto::model::TrackPower::On);
                nav.overlay(if on {
                    cx.s.overlay_power_on
                } else {
                    cx.s.overlay_power_off
                });
                nav.root(ScreenId::Throttle);
            }
            Self::Extras => nav.go(ScreenId::Extras),
        }
    }
}

/// Main menu (Function / Locos / Speed / Power / Extras).
pub struct MenuScreen {
    list: PagedList,
}

impl MenuScreen {
    /// Numbered five-item main menu.
    #[must_use]
    pub fn new() -> Self {
        Self {
            list: PagedList::new(true).with_footer(true),
        }
    }

    fn labels(cx: &ScreenCtx<'_>) -> [&'static str; 5] {
        MenuItem::ALL.map(|item| item.label(cx.s, cx.drive.effective_loco_source))
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
        set_list_hint(&mut g, cx, cx.s.hint_menu);
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
            item.activate(cx, nav);
        }
    }

    /// Open Function/Roster/AddrEdit/Extras, or apply speed-mult / power and return to throttle.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if let Some(item) = self.current(cx) {
            item.activate(cx, nav);
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
