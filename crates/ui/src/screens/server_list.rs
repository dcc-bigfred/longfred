//! mDNS command-station list.

use longfred_proto::command::Protocol;
use longfred_proto::model::MAX_FOUND_SERVERS;

use super::helpers::{height, step_list};
use crate::context::ScreenCtx;
use crate::intent::{AppEvent, Intent};
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::screen::Screen;
use crate::view::{Line, UiView};
use crate::widgets::PagedList;

pub struct ServerListScreen {
    list: PagedList,
}

impl ServerListScreen {
    /// Numbered mDNS server list.
    pub fn new() -> Self {
        Self {
            list: PagedList::new(true),
        }
    }

    /// `"label W|Z"` rows for discovered endpoints.
    fn labels(cx: &ScreenCtx<'_>) -> heapless::Vec<Line, MAX_FOUND_SERVERS> {
        let mut v = heapless::Vec::new();
        for s in cx.net.found_servers {
            let mut line = Line::new();
            let _ = line.push_str(s.label.as_str());
            let _ = line.push(' ');
            let _ = line.push(match s.protocol {
                Protocol::WiThrottle => 'W',
                Protocol::Z21 => 'Z',
            });
            let _ = v.push(line);
        }
        v
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

    /// Discovered services; page-right is protocol pick, page-left is WIT IP entry.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        let bufs = Self::labels(cx);
        let mut names: heapless::Vec<&str, MAX_FOUND_SERVERS> = heapless::Vec::new();
        for b in &bufs {
            let _ = names.push(b.as_str());
        }
        self.list.draw(
            &mut g,
            Some(cx.s.msg_services_found),
            &names,
            true,
            height(cx),
        );
        UiView::Grid(g)
    }

    /// Move the highlighted server row.
    fn on_list_step(&mut self, d: Step, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let bufs = Self::labels(cx);
        let mut names: heapless::Vec<&str, MAX_FOUND_SERVERS> = heapless::Vec::new();
        for b in &bufs {
            let _ = names.push(b.as_str());
        }
        step_list(&mut self.list, d, &names, true, height(cx));
    }

    /// Next → protocol picker. Prev → WIT manual IP (from-list flag set).
    fn on_page(&mut self, d: PageDir, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        match d {
            PageDir::Next => nav.go(ScreenId::ServerProto),
            PageDir::Prev => {
                cx.session.manual_protocol = Protocol::WiThrottle;
                cx.session.server_entry_from_list = true;
                nav.go(ScreenId::ServerEntry);
            }
        }
    }

    /// Digit jumps to that server and connects.
    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if let Some(d) = c.to_digit(10) {
            let hit = {
                let bufs = Self::labels(cx);
                let mut names: heapless::Vec<&str, MAX_FOUND_SERVERS> = heapless::Vec::new();
                for b in &bufs {
                    let _ = names.push(b.as_str());
                }
                self.list
                    .select_digit(d as u8, &names, true, height(cx))
                    .is_some()
            };
            if hit {
                self.on_select(cx, nav);
            }
        }
    }

    /// Connect to the highlighted discovered server and go to throttle.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let idx = {
            let bufs = Self::labels(cx);
            let mut names: heapless::Vec<&str, MAX_FOUND_SERVERS> = heapless::Vec::new();
            for b in &bufs {
                let _ = names.push(b.as_str());
            }
            self.list.global_index(&names, true, height(cx))
        };
        if idx < cx.net.found_servers.len() {
            nav.emit(Intent::ServerSelect(idx));
            nav.root(ScreenId::Throttle);
        }
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
