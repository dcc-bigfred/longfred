//! WIT / Z21 picker for manual server entry.

use longfred_proto::command::Protocol;

use crate::context::ScreenCtx;
use crate::nav::{Nav, ScreenId, Step};
use crate::screen::Screen;
use crate::view::UiView;
use crate::widgets::PagedList;

pub struct ServerProtoScreen {
    list: PagedList,
}

impl ServerProtoScreen {
    /// Two-row WIT / Z21 picker for manual entry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            list: PagedList::new(false),
        }
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

    /// WIT vs Z21; cursor highlights the current protocol.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        g.set(0, cx.s.msg_select_proto, false);
        g.set(1, cx.s.proto_wit, self.list.cursor == 0);
        g.set(2, cx.s.proto_z21, self.list.cursor == 1);
        g.set(5, cx.s.hint_proto, false);
        UiView::Grid(g)
    }

    /// Toggle between WIT and Z21.
    fn on_list_step(&mut self, d: Step, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        match d {
            Step::Prev => self.list.cursor = usize::from(self.list.cursor == 0),
            Step::Next => self.list.cursor = usize::from(self.list.cursor != 1),
        }
    }

    /// Remember protocol and open IP:port entry (not the from-list shortcut).
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        cx.session.manual_protocol = if self.list.cursor == 1 {
            Protocol::Z21
        } else {
            Protocol::WiThrottle
        };
        cx.session.server_entry_from_list = false;
        nav.go(ScreenId::ServerEntry);
    }
}
