//! DCC function list (ON functions inverted).

use longfred_proto::model::MAX_FUNCTIONS;

use super::helpers::{digit_key, height, list_digit, list_star_confirms, page_list, step_list};
use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::screen::{KeyBindings, Screen};
use crate::view::{UiView, fill_list_page_invert};
use crate::widgets::PagedList;

/// DCC function list (ON functions inverted).
pub struct FunctionListScreen {
    list: PagedList,
}

impl FunctionListScreen {
    /// Numbered function picker for the current loco.
    #[must_use]
    pub fn new() -> Self {
        Self {
            list: PagedList::new(true),
        }
    }

    /// Function labels from the current throttle slot.
    fn names<'a>(cx: &'a ScreenCtx<'_>) -> heapless::Vec<&'a str, MAX_FUNCTIONS> {
        let mut v = heapless::Vec::new();
        if let Some(slot) = cx.drive.slots.get(cx.drive.current) {
            for label in &slot.labels {
                if v.push(label.as_str()).is_err() {
                    break;
                }
            }
        }
        v
    }
}

impl Default for FunctionListScreen {
    fn default() -> Self {
        Self::new()
    }
}

fn emit_function(nav: &mut Nav<'_>, idx: usize) {
    if let Ok(id) = u8::try_from(idx) {
        nav.emit(Intent::Function(id));
    }
}

impl Screen for FunctionListScreen {
    fn id(&self) -> ScreenId {
        ScreenId::FunctionList
    }

    fn key_bindings(&self, _cx: &ScreenCtx<'_>) -> KeyBindings {
        KeyBindings::NAVIGATION
    }

    /// Numbered labels; rows whose DCC function is ON are drawn inverted.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        g.set(0, self.list.title_with_index(cx.s.menu_fn).as_str(), false);
        let names = Self::names(cx);
        let ons = cx
            .drive
            .slots
            .get(cx.drive.current)
            .map_or([false; MAX_FUNCTIONS], |s| s.functions);
        fill_list_page_invert(&mut g, &names, &self.list, height(cx), |_local, global| {
            ons.get(global).copied().unwrap_or(false)
        });
        UiView::Grid(g)
    }

    /// Move the highlighted function row.
    fn on_list_step(&mut self, d: Step, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let names = Self::names(cx);
        step_list(&mut self.list, d, &names, height(cx));
    }

    /// Page the function list.
    fn on_page(&mut self, d: PageDir, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let names = Self::names(cx);
        page_list(&mut self.list, d, &names, height(cx));
    }

    /// Digit jumps to that row and toggles it; `*` builds a 1-based index.
    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let Some(d) = digit_key(c) else { return };
        let names = Self::names(cx);
        let h = height(cx);
        if list_digit(&mut self.list, d, &names, h).is_some() {
            let idx = self
                .list
                .global_index(&names, h)
                .min(MAX_FUNCTIONS.saturating_sub(1));
            emit_function(nav, idx);
        }
    }

    fn on_star(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let names = Self::names(cx);
        let h = height(cx);
        if list_star_confirms(&mut self.list, &names, h) {
            let idx = self
                .list
                .global_index(&names, h)
                .min(MAX_FUNCTIONS.saturating_sub(1));
            emit_function(nav, idx);
        }
    }

    fn on_fn_key(&mut self, k: u8, down: bool, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if !down {
            return;
        }
        let names = Self::names(cx);
        let h = height(cx);
        if let Some(idx) = self.list.select_fn_key(k, &names, h) {
            let _ = self.list.clear_index();
            emit_function(nav, idx.min(MAX_FUNCTIONS.saturating_sub(1)));
        }
    }

    /// Toggle the highlighted DCC function.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let _ = self.list.clear_index();
        let names = Self::names(cx);
        let idx = self
            .list
            .global_index(&names, height(cx))
            .min(MAX_FUNCTIONS.saturating_sub(1));
        emit_function(nav, idx);
    }

    /// Skip Menu and return to throttle.
    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if self.list.clear_index() {
            return;
        }
        nav.root(ScreenId::Throttle);
    }
}
