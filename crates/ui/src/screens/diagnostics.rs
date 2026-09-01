//! Seven-page diagnostics (battery / version / board / RF+ping / Wi-Fi / RSSI chart / ping chart).

use core::fmt::Write as _;

use longfred_proto::network::{PingStatus, ServerEndpoint};

use super::helpers::{write_ip_line, write_mac};
use crate::chart::{
    ChartData, ChartScale, PING_THRESHOLD_MS, PING_TIMEOUT_MS, PING_Y_MAX_DEFAULT, RSSI_FIT_PAD,
    RSSI_Y_MAX_DEFAULT, RSSI_Y_MIN_DEFAULT, build_chart, push_sample,
};
use crate::context::ScreenCtx;
use crate::nav::{Nav, PageDir, ScreenId};
use crate::screen::Screen;
use crate::view::{CHART_HISTORY_LEN, Line, UiView, fill_list_page};

const DIAG_PAGES: usize = 7;
const PAGE_RSSI_CHART: usize = 5;
const PAGE_PING_CHART: usize = 6;
const RSSI_SAMPLE_MS: u64 = 1000;
const PING_SAMPLE_MS: u64 = 5000;

/// Seven-page diagnostics (text pages plus RSSI / ping charts).
pub struct DiagnosticsScreen {
    page: usize,
    rssi_hist: heapless::Vec<i16, CHART_HISTORY_LEN>,
    ping_hist: heapless::Vec<i16, CHART_HISTORY_LEN>,
    last_rssi_ms: u64,
    last_ping_ms: u64,
    last_ping_value: Option<i16>,
}

impl DiagnosticsScreen {
    /// Starts on the battery page.
    #[must_use]
    pub fn new() -> Self {
        Self {
            page: 0,
            rssi_hist: heapless::Vec::new(),
            ping_hist: heapless::Vec::new(),
            last_rssi_ms: 0,
            last_ping_ms: 0,
            last_ping_value: None,
        }
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

    /// Title plus the current diagnostics page (text grid or live chart).
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        match self.page {
            PAGE_RSSI_CHART => UiView::Chart(build_chart(ChartData {
                title: cx.s.diag_rssi_chart,
                samples: self.rssi_hist.as_slice(),
                y_min: RSSI_Y_MIN_DEFAULT,
                y_max: RSSI_Y_MAX_DEFAULT,
                threshold: None,
                unit: "dB",
                percentiles: true,
                extra_lines: &[],
                scale: ChartScale::FitPad { pad: RSSI_FIT_PAD },
            })),
            PAGE_PING_CHART => UiView::Chart(build_chart(ChartData {
                title: cx.s.diag_ping_chart,
                samples: self.ping_hist.as_slice(),
                y_min: 0,
                y_max: PING_Y_MAX_DEFAULT,
                threshold: Some(PING_THRESHOLD_MS),
                unit: "ms",
                percentiles: true,
                extra_lines: &[],
                scale: ChartScale::ExpandMax,
            })),
            page => {
                let mut g = crate::view::GridView::new();
                draw_diagnostics(&mut g, page, cx);
                UiView::Grid(g)
            }
        }
    }

    /// Next wraps 0..6. Prev on page 0 leaves diagnostics; otherwise goes back one page.
    fn on_page(&mut self, d: PageDir, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        match d {
            PageDir::Next => self.page = (self.page + 1) % DIAG_PAGES,
            PageDir::Prev if self.page == 0 => nav.back(),
            PageDir::Prev => self.page -= 1,
        }
    }

    /// Select is unused; paging is the only interaction.
    fn on_select(&mut self, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}

    /// Record RSSI / ping samples only while that chart page is visible.
    fn on_tick(&mut self, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        match self.page {
            PAGE_RSSI_CHART => self.sample_rssi(cx),
            PAGE_PING_CHART => self.sample_ping(cx),
            _ => {}
        }
    }
}

impl DiagnosticsScreen {
    fn sample_rssi(&mut self, cx: &ScreenCtx<'_>) {
        let Some(link) = cx.net.wifi_link.as_ref() else {
            return;
        };
        let due =
            self.last_rssi_ms == 0 || cx.now_ms.saturating_sub(self.last_rssi_ms) >= RSSI_SAMPLE_MS;
        if !due {
            return;
        }
        push_sample(&mut self.rssi_hist, i16::from(link.rssi));
        self.last_rssi_ms = cx.now_ms.max(1);
    }

    fn sample_ping(&mut self, cx: &ScreenCtx<'_>) {
        let value = match cx.net.ping {
            PingStatus::Ms(ms) => i16::try_from(ms).unwrap_or(i16::MAX),
            PingStatus::Timeout => PING_TIMEOUT_MS,
            PingStatus::Idle => return,
        };
        let changed = self.last_ping_value != Some(value);
        let due =
            self.last_ping_ms == 0 || cx.now_ms.saturating_sub(self.last_ping_ms) >= PING_SAMPLE_MS;
        if !changed && !due {
            return;
        }
        push_sample(&mut self.ping_hist, value);
        self.last_ping_ms = cx.now_ms.max(1);
        self.last_ping_value = Some(value);
    }
}

/// Fill `g` with one of the text diagnostic pages. Title is row 0; body uses list layout.
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
        _ => t.diag_wifi,
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
                let _ = write!(l, "pin {} mV", b.pin_mv);
                let _ = lines.push(l);
                let mut l = Line::new();
                let yn = if b.charging { t.diag_yes } else { t.diag_no };
                let _ = write!(l, "{} {}", t.diag_charging, yn);
                let _ = lines.push(l);
            } else {
                let mut l = Line::new();
                let _ = l.push_str(na);
                let _ = lines.push(l);
            }
            let mut l = Line::new();
            let _ = write!(l, "factor {:.1}", cx.env.battery_factor);
            let _ = lines.push(l);
            if let Some(b) = cx.battery.filter(|b| b.pin_mv > 0) {
                let mut l = Line::new();
                if b.charging {
                    let _ = write!(l, "{}-{}", b.pin_mv_min, b.pin_mv_max);
                } else {
                    let _ = write!(l, "sug {:.2}", 4200.0 / f32::from(b.pin_mv));
                }
                let _ = lines.push(l);
            } else {
                let mut l = Line::new();
                let _ = l.push_str("3.2-4.2 V");
                let _ = lines.push(l);
            }
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
                Some(p) => {
                    let _ = l.push_str(p.display_name());
                }
                None => {
                    let _ = l.push_str(na);
                }
            }
            let _ = lines.push(l);
            let mut l = Line::new();
            let _ = l.push_str("pref ");
            let _ = l.push_str(cx.drive.persist.roster_mode.as_source().label());
            let _ = lines.push(l);
            let mut l = Line::new();
            let _ = l.push_str("eff ");
            let _ = l.push_str(cx.drive.effective_loco_source.label());
            let _ = lines.push(l);
        }
        3 => {
            if let Some(link) = cx.net.wifi_link.as_ref() {
                let mut l = Line::new();
                let _ = write!(l, "RSSI {} dB", link.rssi);
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
            if let Some(link) = cx.net.wifi_link.as_ref() {
                let mut l = Line::new();
                let _ = write!(l, "ch {}", link.channel);
                let _ = lines.push(l);
            }
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
        }
        _ => {
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
    }

    let mut refs: heapless::Vec<&str, 8> = heapless::Vec::new();
    for line in &lines {
        let _ = refs.push(line.as_str());
    }
    let list = crate::widgets::PagedList {
        page: 0,
        cursor: usize::MAX,
        numbered: false,
        footer: false,
        index: None,
    };
    fill_list_page(g, &refs, &list, cx.env.geometry.height);
}
