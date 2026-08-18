//! Roster / locomotive picker.

use longfred_proto::catalog::{Catalog, LocoCatalog};
use longfred_proto::model::MAX_ROSTER;

use super::helpers::{digit_key, height, list_digit, list_star_confirms, page_list, step_list};
use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::screen::{KeyBindings, Screen};
use crate::view::{UiView, fill_list_page};
use crate::widgets::PagedList;

/// Roster / locomotive picker.
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

    fn catalog<'a>(cx: &'a ScreenCtx<'_>) -> Catalog<'a> {
        Catalog::for_source(
            cx.drive.effective_loco_source,
            cx.drive.roster,
            &cx.drive.persist.static_roster,
        )
    }

    fn names<'a>(cx: &'a ScreenCtx<'_>) -> heapless::Vec<&'a str, MAX_ROSTER> {
        use longfred_proto::LocoSource;
        let source = Self::catalog(cx).source();
        let mut v = heapless::Vec::new();
        match source {
            LocoSource::ServerRoster => {
                for e in cx.drive.roster {
                    if v.push(e.name.as_str()).is_err() {
                        break;
                    }
                }
            }
            LocoSource::StaticRoster => {
                for e in &cx.drive.persist.static_roster {
                    if v.push(e.display_name()).is_err() {
                        break;
                    }
                }
            }
            LocoSource::AddressOnly => {}
        }
        v
    }

    fn acquire_at(cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>, idx: usize) {
        enum Pick {
            Roster(usize),
            Addr(heapless::String<8>),
        }
        let pick = {
            let cat = Self::catalog(cx);
            match cat {
                Catalog::Server(c) if c.allows_pick() => {
                    Some(Pick::Roster(idx.min(c.len().saturating_sub(1))))
                }
                Catalog::Static(c) => c.entry(idx).map(|e| Pick::Addr(e.addr)),
                Catalog::Server(_) | Catalog::Address(_) => None,
            }
        };
        match pick {
            Some(Pick::Roster(i)) => nav.emit(Intent::AcquireRoster(i)),
            Some(Pick::Addr(addr)) => {
                cx.session.addr.clear();
                let _ = cx.session.addr.push_str(addr.as_str());
                nav.emit(Intent::AcquireAddr);
            }
            None => {}
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
        g.set(
            0,
            self.list.title_with_index(cx.s.menu_locos).as_str(),
            false,
        );
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

    /// Digit jumps to that row and acquires it; `*` builds a 1-based index.
    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let Some(d) = digit_key(c) else { return };
        let idx = {
            let names = Self::names(cx);
            let h = height(cx);
            list_digit(&mut self.list, d, &names, h)
        };
        if let Some(idx) = idx {
            Self::acquire_at(cx, nav, idx);
        }
    }

    fn on_star(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let idx = {
            let names = Self::names(cx);
            let h = height(cx);
            list_star_confirms(&mut self.list, &names, h).then(|| self.list.global_index(&names, h))
        };
        if let Some(idx) = idx {
            Self::acquire_at(cx, nav, idx);
        }
    }

    fn on_fn_key(&mut self, k: u8, down: bool, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if !down {
            return;
        }
        let idx = {
            let names = Self::names(cx);
            let h = height(cx);
            self.list.select_fn_key(k, &names, h).inspect(|_| {
                let _ = self.list.clear_index();
            })
        };
        if let Some(idx) = idx {
            Self::acquire_at(cx, nav, idx);
        }
    }

    /// Acquire the highlighted WIT roster entry or static-roster address, then throttle.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let _ = self.list.clear_index();
        let idx = {
            let names = Self::names(cx);
            self.list.global_index(&names, height(cx))
        };
        Self::acquire_at(cx, nav, idx);
    }

    /// Skip Menu and return to throttle.
    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if self.list.clear_index() {
            return;
        }
        nav.root(ScreenId::Throttle);
    }
}
