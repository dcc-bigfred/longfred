//! OLED rendering model — pure data structures (no logic).

use longfred_proto::mdns::WitServer;

use crate::config::sizes;
use crate::domain::state::DomainState;
use crate::net::{ConnState, NetStatus, ServerEndpoint, SsidInfo};

pub const GRID_LINES: usize = 12;
pub const LINE_LEN: usize = 21;

pub type Line = heapless::String<LINE_LEN>;

#[derive(Clone, PartialEq, Eq)]
pub struct GridView {
    pub lines: heapless::Vec<Line, GRID_LINES>,
    pub invert: heapless::Vec<bool, GRID_LINES>,
    pub top_line: bool,
    pub foot_line: bool,
}

impl GridView {
    pub fn new() -> Self {
        Self {
            lines: heapless::Vec::new(),
            invert: heapless::Vec::new(),
            top_line: false,
            foot_line: true,
        }
    }

    pub fn set(&mut self, idx: usize, text: &str, inv: bool) {
        if idx >= GRID_LINES {
            return;
        }
        while self.lines.len() <= idx {
            let _ = self.lines.push(Line::new());
            let _ = self.invert.push(false);
        }
        self.lines[idx].clear();
        let _ = self.lines[idx].push_str(text);
        if idx < self.invert.len() {
            self.invert[idx] = inv;
        }
    }
}

impl Default for GridView {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ThrottleView {
    pub current: u8,
    pub speed: u8,
    pub forward: bool,
    pub consist_len: u8,
    pub power_on: bool,
    pub heartbeat_on: bool,
    pub functions: u32,
    pub loco: Line,
    pub footer: Line,
    pub next_hint: Line,
    pub battery: Option<u8>,
    pub battery_show_percent: bool,
}

impl Default for ThrottleView {
    fn default() -> Self {
        Self {
            current: 0,
            speed: 0,
            forward: true,
            consist_len: 0,
            power_on: false,
            heartbeat_on: true,
            functions: 0,
            loco: Line::new(),
            footer: Line::new(),
            next_hint: Line::new(),
            battery: None,
            battery_show_percent: false,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum UiView {
    Throttle(ThrottleView),
    Grid(GridView),
}

impl Default for UiView {
    fn default() -> Self {
        UiView::Grid(GridView::new())
    }
}

/// Read-only context for building the view (domain + network).
pub struct ViewCtx<'a> {
    pub domain: &'a DomainState,
    pub net_status: NetStatus,
    pub conn: ConnState,
    pub server: Option<ServerEndpoint>,
    pub scanned_ssids: &'a heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
    pub found_servers: &'a heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    pub selected_ssid: &'a str,
    pub password_preview: &'a str,
    pub pw_picker_char: u8,
    pub ip_formatted: &'a str,
    pub broadcast: Option<&'a str>,
    pub battery: Option<u8>,
}
