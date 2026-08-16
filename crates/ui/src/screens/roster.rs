//! Roster / locomotive picker.

use longfred_proto::model::MAX_ROSTER;

use super::helpers::{digit_key, height, page_list, step_list};
use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::screen::{KeyBindings, Screen};
use crate::view::{UiView, fill_list_page};
use crate::widgets::PagedList;

pub struct RosterListScreen {
    list: PagedList,
}

impl RosterListScreen {
    /// Numbered roster picker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            list: PagedList::new(true),
        }
    }

    /// Live WIT roster names, or persisted static roster (name, else address).
    fn names<'a>(cx: &'a ScreenCtx<'_>) -> heapless::Vec<&'a str, MAX_ROSTER> {
        let mut v = heapless::Vec::new();
        if cx.drive.roster.is_empty() {
            for e in &cx.drive.persist.static_roster {
                let s = if e.name.is_empty() {
                    e.addr.as_str()
                } else {
                    e.name.as_str()
                };
                if v.push(s).is_err() {
                    break;
                }
            }
        } else {
            for e in cx.drive.roster {
                if v.push(e.name.as_str()).is_err() {
                    break;
                }
            }
        }
        v
    }

    fn acquire_at(cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>, idx: usize) {
        if !cx.drive.roster.is_empty() {
            nav.emit(Intent::AcquireRoster(
                idx.min(cx.drive.roster.len().saturating_sub(1)),
            ));
        } else if let Some(e) = cx.drive.persist.static_roster.get(idx) {
            cx.session.addr.clear();
            let _ = cx.session.addr.push_str(e.addr.as_str());
            nav.emit(Intent::AcquireAddr);
        }
        nav.root(ScreenId::Throttle);
    }
}

impl Default for RosterListScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for RosterListScreen {
    fn id(&self) -> ScreenId {
        ScreenId::RosterList
    }

    fn key_bindings(&self, _cx: &ScreenCtx<'_>) -> KeyBindings {
        KeyBindings::NAVIGATION
    }

    /// Title plus numbered loco names for the current page.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        g.set(0, cx.s.menu_locos, false);
        let names = Self::names(cx);
        fill_list_page(&mut g, &names, &self.list, height(cx));
        UiView::Grid(g)
    }

    /// Move the highlighted roster row.
    fn on_list_step(&mut self, d: Step, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let names = Self::names(cx);
        step_list(&mut self.list, d, &names, height(cx));
    }

    /// Page the roster list.
    fn on_page(&mut self, d: PageDir, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let names = Self::names(cx);
        page_list(&mut self.list, d, &names, height(cx));
    }

    /// Digit jumps to that row and acquires it.
    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let Some(d) = digit_key(c) else { return };
        let idx = {
            let names = Self::names(cx);
            let h = height(cx);
            self.list
                .select_digit(d, &names, h)
                .is_some()
                .then(|| self.list.global_index(&names, h))
        };
        if let Some(idx) = idx {
            Self::acquire_at(cx, nav, idx);
        }
    }

    /// Acquire the highlighted WIT roster entry or static-roster address, then throttle.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let idx = {
            let names = Self::names(cx);
            self.list.global_index(&names, height(cx))
        };
        Self::acquire_at(cx, nav, idx);
    }

    /// Skip Menu and return to throttle.
    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.root(ScreenId::Throttle);
    }
}
