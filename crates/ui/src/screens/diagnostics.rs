//! Six-page diagnostics (battery / version / board / RF / Wi-Fi / ping).

use core::fmt::Write as _;

use longfred_proto::command::Protocol;
use longfred_proto::net_status::{PingStatus, ServerEndpoint};

use super::helpers::{write_ip_line, write_mac};
use crate::context::ScreenCtx;
use crate::nav::{Nav, PageDir, ScreenId};
use crate::screen::Screen;
use crate::view::{Line, UiView, fill_list_page};

const DIAG_PAGES: usize = 6;

pub struct DiagnosticsScreen {
    page: usize,
}

impl DiagnosticsScreen {
    /// Starts on the battery page.
    #[must_use]
    pub fn new() -> Self {
        Self { page: 0 }
    }
}

impl Default for DiagnosticsScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for DiagnosticsScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Diagnostics
    }

    /// Title plus the current diagnostics page (battery / version / board / RF / Wi-Fi / ping).
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        draw_diagnostics(&mut g, self.page, cx);
        UiView::Grid(g)
    }

    /// Next wraps 0..5. Prev on page 0 leaves diagnostics; otherwise goes back one page.
    fn on_page(&mut self, d: PageDir, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        match d {
            PageDir::Next => self.page = (self.page + 1) % DIAG_PAGES,
            PageDir::Prev if self.page == 0 => nav.back(),
            PageDir::Prev => self.page -= 1,
        }
    }

    /// Select is unused; paging is the only interaction.
    fn on_select(&mut self, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}
}

/// Fill `g` with one of six diagnostic pages. Title is row 0; body uses list layout.
#[allow(clippy::too_many_lines)]
fn draw_diagnostics(g: &mut crate::view::GridView, page: usize, cx: &ScreenCtx<'_>) {
    g.foot_line = false;
    let t = cx.s;
    let na = t.diag_na;
    let mut lines: heapless::Vec<Line, 8> = heapless::Vec::new();
    let title = match page {
        0 => t.diag_battery,
        1 => t.diag_version,
        2 => t.diag_software,
        3 => t.diag_range,
        4 => t.diag_wifi,
        _ => t.diag_ping,
    };
    g.set(0, title, false);

    match page {
        0 => {
            if let Some(b) = cx.battery {
                let mut l = Line::new();
                let _ = write!(l, "{}%", b.percent);
                let _ = lines.push(l);
                let mut l = Line::new();
                let _ = write!(l, "{} mV", b.millivolts);
                let _ = lines.push(l);
                let mut l = Line::new();
                let _ = write!(l, "ADC {}", b.raw);
                let _ = lines.push(l);
            } else {
                let mut l = Line::new();
                let _ = l.push_str(na);
                let _ = lines.push(l);
            }
            let mut l = Line::new();
            let _ = write!(l, "factor {:.1}", cx.env.battery_factor);
            let _ = lines.push(l);
            let mut l = Line::new();
            let _ = l.push_str("3.2-4.2 V");
            let _ = lines.push(l);
        }
        1 => {
            let mut l = Line::new();
            let _ = l.push_str(cx.env.app_name);
            let _ = lines.push(l);
            let mut l = Line::new();
            let _ = l.push_str(cx.env.fw_version);
            let _ = lines.push(l);
        }
        2 => {
            let mut l = Line::new();
            let _ = l.push_str(cx.env.board_id);
            let _ = lines.push(l);
            let mut l = Line::new();
            let _ = l.push_str(cx.env.board_mcu);
            let _ = lines.push(l);
            let proto = cx
                .net
                .server
                .map(|s| s.protocol)
                .or_else(|| cx.drive.persist.last_server.map(|s| s.protocol));
            let mut l = Line::new();
            match proto {
                Some(Protocol::WiThrottle) => {
                    let _ = l.push_str("WiThrottle");
                }
                Some(Protocol::Z21) => {
                    let _ = l.push_str("Z21");
                }
                None => {
                    let _ = l.push_str(na);
                }
            }
            let _ = lines.push(l);
        }
        3 => {
            if let Some(link) = cx.net.wifi_link.as_ref() {
                let mut l = Line::new();
                let _ = write!(l, "RSSI {} dB", link.rssi);
                let _ = lines.push(l);
                let mut l = Line::new();
                let _ = write!(l, "ch {}", link.channel);
                let _ = lines.push(l);
            } else {
                let mut l = Line::new();
                let _ = l.push_str(na);
                let _ = lines.push(l);
            }
        }
        4 => {
            if let Some(link) = cx.net.wifi_link.as_ref() {
                let mut l = Line::new();
                let _ = l.push_str(link.ssid.as_str());
                let _ = lines.push(l);
            } else {
                let mut l = Line::new();
                let _ = l.push_str(na);
                let _ = lines.push(l);
            }
            if let Some(net) = cx.net.sta_net {
                let mut l = Line::new();
                write_ip_line(&mut l, net.ip);
                let _ = write!(l, "/{}", net.prefix);
                let _ = lines.push(l);
                let mut l = Line::new();
                if let Some(gw) = net.gateway {
                    write_ip_line(&mut l, gw);
                } else {
                    let _ = l.push_str(na);
                }
                let _ = lines.push(l);
                let mut l = Line::new();
                let _ = l.push_str("STA ");
                write_mac(&mut l, net.mac);
                let _ = lines.push(l);
            } else {
                let mut l = Line::new();
                let _ = l.push_str(na);
                let _ = lines.push(l);
            }
            let mut l = Line::new();
            let _ = l.push_str("AP ");
            if let Some(link) = cx.net.wifi_link.as_ref() {
                write_mac(&mut l, link.bssid);
            } else {
                let _ = l.push_str(na);
            }
            let _ = lines.push(l);
        }
        _ => {
            let ep = cx.net.server.or_else(|| {
                cx.drive.persist.last_server.map(|s| ServerEndpoint {
                    ip: s.ip,
                    port: s.port,
                    protocol: s.protocol,
                })
            });
            if let Some(ep) = ep {
                let mut l = Line::new();
                write_ip_line(&mut l, ep.ip);
                let _ = l.push(':');
                let _ = write!(l, "{}", ep.port);
                let _ = lines.push(l);
            } else {
                let mut l = Line::new();
                let _ = l.push_str(na);
                let _ = lines.push(l);
            }
            let mut l = Line::new();
            match cx.net.ping {
                PingStatus::Ms(ms) => {
                    let _ = write!(l, "{ms} ms");
                }
                PingStatus::Timeout => {
                    let _ = l.push_str(t.diag_timeout);
                }
                PingStatus::Idle => {
                    let _ = l.push_str(na);
                }
            }
            let _ = lines.push(l);
        }
    }

    let mut refs: heapless::Vec<&str, 8> = heapless::Vec::new();
    for line in &lines {
        let _ = refs.push(line.as_str());
    }
    let list = crate::widgets::PagedList {
        page: 0,
        cursor: usize::MAX,
        numbered: false,
    };
    fill_list_page(g, &refs, &list, cx.env.geometry.height);
}
