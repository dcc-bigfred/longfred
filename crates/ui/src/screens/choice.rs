//! Numbered option list for [`crate::session::ChoiceKind`].

use longfred_proto::persist::RosterMode;

use super::helpers::{digit_key, height, list_digit, list_star_confirms, page_list, step_list};
use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::screen::{KeyBindings, Screen};
use crate::session::ChoiceKind;
use crate::view::UiView;
use crate::widgets::PagedList;

/// Generic numbered picker (dead-man, roster source, how to connect).
pub struct ChoiceScreen {
    list: PagedList,
}

impl ChoiceScreen {
    /// Numbered choice list.
    #[must_use]
    pub fn new() -> Self {
        Self {
            list: PagedList::new(true),
        }
    }

    fn labels(cx: &ScreenCtx<'_>) -> heapless::Vec<&'static str, 3> {
        let mut v = heapless::Vec::new();
        match cx.session.choice {
            ChoiceKind::DeadMan => {
                let _ = v.push(cx.s.choice_dead_man_keep);
                let _ = v.push(cx.s.choice_dead_man_off);
            }
            ChoiceKind::RosterSource => {
                let _ = v.push(cx.s.extras_roster_auto);
                let _ = v.push(cx.s.extras_roster_static);
                let _ = v.push(cx.s.extras_roster_address);
            }
            ChoiceKind::ServerConnect => {
                let _ = v.push(cx.s.server_find);
                let _ = v.push(cx.s.server_manual);
            }
        }
        v
    }

    fn title(cx: &ScreenCtx<'_>) -> &'static str {
        match cx.session.choice {
            ChoiceKind::DeadMan => cx.s.extras_dead_man_tgl,
            ChoiceKind::RosterSource => cx.s.choice_roster,
            ChoiceKind::ServerConnect => cx.s.choice_connection,
        }
    }

    fn initial_index(cx: &ScreenCtx<'_>) -> usize {
        match cx.session.choice {
            ChoiceKind::DeadMan => usize::from(!cx.drive.dead_man_switch_on),
            ChoiceKind::RosterSource => match cx.drive.persist.roster_mode {
                RosterMode::Auto => 0,
                RosterMode::Static => 1,
                RosterMode::AddressOnly => 2,
            },
            ChoiceKind::ServerConnect => 0,
        }
    }

    fn activate(cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>, idx: usize) {
        match cx.session.choice {
            ChoiceKind::DeadMan => {
                let want_on = idx == 0;
                if want_on != cx.drive.dead_man_switch_on {
                    nav.emit(Intent::DeadManSwitchToggle);
                }
                nav.back();
            }
            ChoiceKind::RosterSource => {
                let mode = match idx {
                    1 => RosterMode::Static,
                    2 => RosterMode::AddressOnly,
                    _ => RosterMode::Auto,
                };
                if mode != cx.drive.persist.roster_mode {
                    nav.emit(Intent::SetRosterMode(mode));
                }
                nav.back();
            }
            ChoiceKind::ServerConnect => {
                if idx == 0 {
                    nav.emit(Intent::RequestMdns);
                    nav.go(ScreenId::ServerList);
                } else {
                    cx.session.server_digits.clear();
                    cx.session.server_entry_from_list = false;
                    nav.go(ScreenId::ServerProto);
                }
            }
        }
    }
}

impl Default for ChoiceScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for ChoiceScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Choice
    }

    fn key_bindings(&self, _cx: &ScreenCtx<'_>) -> KeyBindings {
        KeyBindings::NAVIGATION
    }

    fn on_enter(&mut self, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        let idx = Self::initial_index(cx).min(labels.len().saturating_sub(1));
        self.list.focus_global(idx, &labels, height(cx));
    }

    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        let labels = Self::labels(cx);
        self.list
            .draw(&mut g, Some(Self::title(cx)), &labels, height(cx));
        UiView::Grid(g)
    }

    fn on_list_step(&mut self, d: Step, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        step_list(&mut self.list, d, &labels, height(cx));
    }

    fn on_page(&mut self, d: PageDir, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        page_list(&mut self.list, d, &labels, height(cx));
    }

    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let Some(d) = digit_key(c) else { return };
        let labels = Self::labels(cx);
        let h = height(cx);
        if let Some(idx) = list_digit(&mut self.list, d, &labels, h) {
            Self::activate(cx, nav, idx);
        }
    }

    fn on_star(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        let h = height(cx);
        if list_star_confirms(&mut self.list, &labels, h) {
            let idx = self.list.global_index(&labels, h);
            Self::activate(cx, nav, idx);
        }
    }

    fn on_fn_key(&mut self, k: u8, down: bool, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if !down {
            return;
        }
        let labels = Self::labels(cx);
        let h = height(cx);
        if let Some(idx) = self.list.select_fn_key(k, &labels, h) {
            let _ = self.list.clear_index();
            Self::activate(cx, nav, idx);
        }
    }

    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let _ = self.list.clear_index();
        let labels = Self::labels(cx);
        let idx = self.list.global_index(&labels, height(cx));
        Self::activate(cx, nav, idx);
    }

    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if self.list.clear_index() {
            return;
        }
        nav.back();
    }
}
