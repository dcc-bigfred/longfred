//! mDNS command-station list.

use longfred_proto::command::Protocol;
use longfred_proto::model::MAX_FOUND_SERVERS;

use super::helpers::{digit_key, height, page_list, set_list_hint, step_list};
use crate::context::ScreenCtx;
use crate::intent::{AppEvent, Intent};
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::screen::Screen;
use crate::view::{LINE_LEN, Line, UiView, push_oled};
use crate::widgets::PagedList;

/// mDNS command-station list.
pub struct ServerListScreen {
    list: PagedList,
}

impl ServerListScreen {
    /// Numbered mDNS server list.
    #[must_use]
    pub fn new() -> Self {
        Self {
            list: PagedList::new(true).with_footer(true),
        }
    }

    /// `"label W|Z|B"` rows for discovered endpoints.
    fn labels(cx: &ScreenCtx<'_>) -> heapless::Vec<Line, MAX_FOUND_SERVERS> {
        let mut v = heapless::Vec::new();
        for s in cx.net.found_servers {
            let mut name = Line::new();
            push_oled(&mut name, s.label.as_str());
            while name.len() > LINE_LEN.saturating_sub(2) {
                let _ = name.pop();
            }
            let mut line = Line::new();
            push_oled(&mut line, name.as_str());
            let _ = line.push(' ');
            let _ = line.push(s.protocol.glyph());
            let _ = v.push(line);
        }
        v
    }

    fn name_refs(bufs: &[Line]) -> heapless::Vec<&str, MAX_FOUND_SERVERS> {
        let mut names = heapless::Vec::new();
        for b in bufs {
            let _ = names.push(b.as_str());
        }
        names
    }

    fn connect_at(cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>, idx: usize) {
        if idx < cx.net.found_servers.len() {
            nav.emit(Intent::ServerSelect(idx));
            nav.root(ScreenId::Throttle);
        }
    }

    fn open_manual_ip(cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        cx.session.manual_protocol = Protocol::WiThrottle;
        cx.session.server_entry_from_list = true;
        nav.go(ScreenId::ServerEntry);
    }

    fn open_proto(cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        cx.session.server_digits.clear();
        cx.session.server_entry_from_list = false;
        nav.go(ScreenId::ServerProto);
    }
}

impl Default for ServerListScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for ServerListScreen {
    fn id(&self) -> ScreenId {
        ScreenId::ServerList
    }

    /// Ask firmware to (re)start mDNS browsing.
    fn on_enter(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.emit(Intent::RequestMdns);
    }

    /// Discovered services; last content row shows key hints.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        let bufs = Self::labels(cx);
        let names = Self::name_refs(&bufs);
        self.list
            .draw(&mut g, Some(cx.s.msg_services_found), &names, height(cx));
        set_list_hint(&mut g, cx, cx.s.hint_server_list);
        UiView::Grid(g)
    }

    /// Move the highlighted server row.
    fn on_list_step(&mut self, d: Step, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let bufs = Self::labels(cx);
        let names = Self::name_refs(&bufs);
        step_list(&mut self.list, d, &names, height(cx));
    }

    /// Left / Right Menu pages the list.
    fn on_page(&mut self, d: PageDir, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let bufs = Self::labels(cx);
        let names = Self::name_refs(&bufs);
        page_list(&mut self.list, d, &names, height(cx));
    }

    /// Menu restarts mDNS without leaving the list.
    fn on_menu_key(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.emit(Intent::RequestMdns);
    }

    /// Stop: typed IP (keypad) or protocol picker (joystick).
    fn on_stop(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if cx.env.has_keypad {
            Self::open_manual_ip(cx, nav);
        } else {
            Self::open_proto(cx, nav);
        }
    }

    /// `*` opens the protocol picker.
    fn on_star(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        Self::open_proto(cx, nav);
    }

    /// Digit jumps to that server and connects.
    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let Some(d) = digit_key(c) else { return };
        let idx = {
            let bufs = Self::labels(cx);
            let names = Self::name_refs(&bufs);
            let h = height(cx);
            self.list
                .select_digit(d, &names, h)
                .is_some()
                .then(|| self.list.global_index(&names, h))
        };
        if let Some(idx) = idx {
            Self::connect_at(cx, nav, idx);
        }
    }

    /// Connect to the highlighted discovered server and go to throttle.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let idx = {
            let bufs = Self::labels(cx);
            let names = Self::name_refs(&bufs);
            self.list.global_index(&names, height(cx))
        };
        Self::connect_at(cx, nav, idx);
    }

    /// Already connected (e.g. last server) → throttle.
    fn on_app_event(&mut self, e: AppEvent, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if e == AppEvent::ServerConnected {
            nav.root(ScreenId::Throttle);
        }
    }

    /// Back to compiled SSIDs, or scan results if none are compiled in.
    fn on_cancel(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if cx.env.compiled_networks.is_empty() {
            nav.replace(ScreenId::SsidScan);
        } else {
            nav.replace(ScreenId::SsidList);
        }
    }
}
