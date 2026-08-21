//! WIT / Z21 picker for manual server entry.

use longfred_proto::command::Protocol;

use super::helpers::{
    digit_key, height, list_label_digit, list_star_confirms, page_list, step_list,
};
use crate::context::ScreenCtx;
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::screen::{KeyBindings, Screen};
use crate::view::UiView;
use crate::widgets::PagedList;

/// WIT / Z21 picker for manual server entry.
pub struct ServerProtoScreen {
    list: PagedList,
}

impl ServerProtoScreen {
    /// Two-row WIT / Z21 picker for manual entry (`0` / `1` in the labels).
    #[must_use]
    pub fn new() -> Self {
        Self {
            list: PagedList::new(false).with_footer(true),
        }
    }

    fn labels(cx: &ScreenCtx<'_>) -> [&'static str; 2] {
        [cx.s.proto_wit, cx.s.proto_z21]
    }

    fn confirm(&self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        let idx = self.list.global_index(&labels, height(cx));
        cx.session.manual_protocol = if idx == 1 {
            Protocol::Z21
        } else {
            Protocol::WiThrottle
        };
        cx.session.server_entry_from_list = false;
        nav.go(ScreenId::ServerEntry);
    }
}

impl Default for ServerProtoScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for ServerProtoScreen {
    fn id(&self) -> ScreenId {
        ScreenId::ServerProto
    }

    fn key_bindings(&self, _cx: &ScreenCtx<'_>) -> KeyBindings {
        KeyBindings::NAVIGATION
    }

    /// WIT vs Z21; cursor highlights the current protocol.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        let labels = Self::labels(cx);
        self.list
            .draw(&mut g, Some(cx.s.msg_select_proto), &labels, height(cx));
        super::helpers::set_list_hint(&mut g, cx, cx.s.hint_proto);
        UiView::Grid(g)
    }

    /// Toggle between WIT and Z21.
    fn on_list_step(&mut self, d: Step, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        step_list(&mut self.list, d, &labels, height(cx));
    }

    fn on_page(&mut self, d: PageDir, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        page_list(&mut self.list, d, &labels, height(cx));
    }

    /// Digit `0` / `1` matches the label prefix and opens IP:port entry.
    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let Some(d) = digit_key(c) else { return };
        let labels = Self::labels(cx);
        if list_label_digit(&mut self.list, d, &labels, height(cx)).is_some() {
            self.confirm(cx, nav);
        }
    }

    fn on_star(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        if list_star_confirms(&mut self.list, &labels, height(cx)) {
            self.confirm(cx, nav);
        }
    }

    fn on_fn_key(&mut self, k: u8, down: bool, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if !down {
            return;
        }
        let labels = Self::labels(cx);
        if self.list.select_fn_key(k, &labels, height(cx)).is_some() {
            let _ = self.list.clear_index();
            self.confirm(cx, nav);
        }
    }

    /// Remember protocol and open IP:port entry (not the from-list shortcut).
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let _ = self.list.clear_index();
        self.confirm(cx, nav);
    }

    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if self.list.clear_index() {
            return;
        }
        nav.back();
    }
}
