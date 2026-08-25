//! Confirm a discovered mDNS server before connecting.

use super::helpers::{format_found_server_addr, format_found_server_name, height, set_list_hint};
use crate::context::ScreenCtx;
use crate::intent::{AppEvent, Intent};
use crate::nav::{Nav, ScreenId};
use crate::screen::Screen;
use crate::view::UiView;

/// Confirm name, protocol, and DNS/IP of a discovered server.
pub struct ServerConfirmScreen;

impl ServerConfirmScreen {
    fn pending<'a>(cx: &'a ScreenCtx<'_>) -> Option<&'a longfred_proto::network::WitServer> {
        let idx = cx.session.pending_server_idx?;
        cx.net.found_servers.get(idx)
    }

    fn confirm(cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let Some(idx) = cx.session.pending_server_idx else {
            nav.back();
            return;
        };
        if idx >= cx.net.found_servers.len() {
            nav.back();
            return;
        }
        nav.emit(Intent::ServerSelect(idx));
        nav.root(ScreenId::Throttle);
    }
}

impl Screen for ServerConfirmScreen {
    fn id(&self) -> ScreenId {
        ScreenId::ServerConfirm
    }

    /// Leave if the highlighted server disappeared after a rescan.
    fn on_enter(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if Self::pending(cx).is_none() {
            nav.back();
        }
    }

    /// Name, protocol, and DNS hostname or IPv4:port.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        let Some(s) = Self::pending(cx) else {
            g.set(0, cx.s.msg_confirm_server, false);
            set_list_hint(&mut g, cx, cx.s.hint_server_confirm);
            return UiView::Grid(g);
        };
        let name = format_found_server_name(s);
        let proto = s.protocol.display_name();
        let addr = format_found_server_addr(s);
        if height(cx) <= 32 {
            g.set(0, name.as_str(), false);
            g.set(1, proto, false);
            g.set(2, addr.as_str(), false);
        } else {
            g.set(0, cx.s.msg_confirm_server, false);
            g.set(1, name.as_str(), false);
            g.set(2, proto, false);
            g.set(3, addr.as_str(), false);
        }
        set_list_hint(&mut g, cx, cx.s.hint_server_confirm);
        UiView::Grid(g)
    }

    /// Menu confirms and connects.
    fn on_menu_key(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        Self::confirm(cx, nav);
    }

    /// Back returns to the mDNS list without connecting.
    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.back();
    }

    fn on_app_event(&mut self, e: AppEvent, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if e == AppEvent::ServerConnected {
            nav.root(ScreenId::Throttle);
        }
    }
}
