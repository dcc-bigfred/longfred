//! OLED rendering model — pure data structures (no logic).

use longfred_proto::mdns::WitServer;

use crate::config::sizes;
use crate::domain::state::DomainState;
use crate::net::{ConnState, NetStatus, PingStatus, ServerEndpoint, SsidInfo, StaNet, WifiLink};
use crate::power::battery::BatterySample;

pub const GRID_LINES: usize = 12;
pub const LINE_LEN: usize = 21;

pub type Line = heapless::String<LINE_LEN>;

#[derive(Clone, PartialEq, Eq)]
pub struct GridView {
    pub lines: heapless::Vec<Line, GRID_LINES>,
    pub invert: heapless::Vec<bool, GRID_LINES>,
    pub top_line: bool,
    pub foot_line: bool,
    /// Caps Lock indicator: `Some(true)` = uppercase (arrow up).
    pub caps: Option<bool>,
}

impl GridView {
    pub fn new() -> Self {
        Self {
            lines: heapless::Vec::new(),
            invert: heapless::Vec::new(),
            top_line: false,
            foot_line: true,
            caps: None,
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
        push_oled(&mut self.lines[idx], text);
        if idx < self.invert.len() {
            self.invert[idx] = inv;
        }
    }
}

/// FONT_6X10 is ASCII-only. Fold Latin extras and stop at [`LINE_LEN`] bytes
/// so a long SSID cannot fail `push_str` and render as a blank line.
pub fn push_oled(line: &mut Line, s: &str) {
    for c in s.chars() {
        if line.push(oled_char(c)).is_err() {
            break;
        }
    }
}

/// Full-width line in characters (`FONT_6X10` × 6 px on a 128 px panel).
pub fn col_chars() -> usize {
    LINE_LEN
}

/// Sequential content rows for paged lists (header index 0 unused).
pub fn list_slots() -> &'static [usize] {
    if crate::board::active_variant()
        .display
        .is_some_and(|d| d.height <= 32)
    {
        &[1, 2, 3]
    } else {
        &[1, 2, 3, 4, 5, 6]
    }
}

/// 1-based number drawn next to a list item (`global` 0 → `1:`).
pub fn item_display_num(global: usize) -> usize {
    global.saturating_add(1)
}

/// Keypad `1`–`9` → those display numbers; `0` → 10.
pub fn digit_display_num(n: u8) -> usize {
    if n == 0 { 10 } else { n as usize }
}

fn push_item_num_prefix(buf: &mut heapless::String<64>, global: usize) {
    let n = item_display_num(global);
    if n >= 100 {
        let _ = buf.push((b'0' + (n / 100) as u8) as char);
    }
    if n >= 10 {
        let _ = buf.push((b'0' + ((n / 10) % 10) as u8) as char);
    }
    let _ = buf.push((b'0' + (n % 10) as u8) as char);
    let _ = buf.push(':');
}

fn item_line_count(s: &str, numbered: bool, global: usize) -> usize {
    wrap_item_chunks(s, numbered, global).len().max(1)
}

fn wrap_item_chunks(s: &str, numbered: bool, global: usize) -> heapless::Vec<Line, 8> {
    let mut folded = heapless::String::<64>::new();
    if numbered {
        push_item_num_prefix(&mut folded, global);
    }
    for c in s.chars() {
        let _ = folded.push(oled_char(c));
    }
    wrap_chunks(folded.as_str(), col_chars())
}

/// Wrap at [`col_chars`], preferring a space so sentences stay readable.
fn wrap_chunks(folded: &str, col: usize) -> heapless::Vec<Line, 8> {
    let col = col.clamp(1, LINE_LEN);
    let mut out: heapless::Vec<Line, 8> = heapless::Vec::new();
    let mut cur = Line::new();
    for c in folded.chars() {
        if cur.is_empty() && c == ' ' {
            continue;
        }
        if cur.len() + 1 > col {
            flush_line(&mut out, &mut cur);
            if c == ' ' {
                continue;
            }
        }
        if cur.push(c).is_err() {
            break;
        }
    }
    if cur.is_empty() && out.is_empty() {
        let _ = out.push(Line::new());
    } else if !cur.is_empty() {
        let _ = out.push(cur);
    }
    out
}

fn flush_line(out: &mut heapless::Vec<Line, 8>, cur: &mut Line) {
    if let Some(sp) = cur.rfind(' ') {
        if sp > 0 && sp + 1 < cur.len() {
            let mut next = Line::new();
            for ch in cur.chars().skip(sp + 1) {
                let _ = next.push(ch);
            }
            cur.truncate(sp);
            let _ = out.push(core::mem::replace(cur, next));
            return;
        }
    }
    let _ = out.push(core::mem::replace(cur, Line::new()));
}

/// How many `items` starting at `start` fit on one list page.
pub fn items_fitting(items: &[&str], start: usize, numbered: bool) -> usize {
    let slots = list_slots().len();
    let mut used = 0usize;
    let mut n = 0usize;
    for (off, s) in items.iter().skip(start).enumerate() {
        let lines = item_line_count(s, numbered, start + off);
        if used + lines > slots {
            break;
        }
        used += lines;
        n += 1;
    }
    n
}

/// First item index of `page` in a wrapped list.
pub fn page_start(items: &[&str], page: usize, numbered: bool) -> usize {
    let mut idx = 0usize;
    for _ in 0..page {
        let n = items_fitting(items, idx, numbered);
        if n == 0 {
            break;
        }
        idx += n;
    }
    idx
}

pub fn page_item_count(items: &[&str], page: usize, numbered: bool) -> usize {
    let start = page_start(items, page, numbered);
    items_fitting(items, start, numbered)
}

pub fn has_next_page(items: &[&str], page: usize, numbered: bool) -> bool {
    let start = page_start(items, page, numbered);
    start + items_fitting(items, start, numbered) < items.len()
}

/// Place wrapped list items into grid slots. `cursor` is the index **on this page**.
pub fn fill_list_page(
    g: &mut GridView,
    items: &[&str],
    page: usize,
    cursor: usize,
    numbered: bool,
) {
    fill_list_page_invert(g, items, page, numbered, |local, _global| local == cursor);
}

/// Like [`fill_list_page`], with a custom invert predicate `(local, global) -> bool`.
pub fn fill_list_page_invert<F: Fn(usize, usize) -> bool>(
    g: &mut GridView,
    items: &[&str],
    page: usize,
    numbered: bool,
    invert: F,
) {
    g.foot_line = false;
    let slots = list_slots();
    let col = col_chars();
    let start = page_start(items, page, numbered);
    let mut slot_i = 0usize;
    let mut local = 0usize;
    for (off, item) in items.iter().skip(start).enumerate() {
        let global = start + off;
        let mut folded = heapless::String::<64>::new();
        if numbered {
            push_item_num_prefix(&mut folded, global);
        }
        for c in item.chars() {
            let _ = folded.push(oled_char(c));
        }
        let chunks = wrap_chunks(folded.as_str(), col);
        if slot_i + chunks.len() > slots.len() {
            break;
        }
        let inv = invert(local, global);
        for (ci, chunk) in chunks.iter().enumerate() {
            g.set(slots[slot_i], chunk.as_str(), ci == 0 && inv);
            slot_i += 1;
        }
        local += 1;
    }
}

fn oled_char(c: char) -> char {
    match c {
        'ą' | 'á' | 'à' | 'â' | 'ä' | 'ã' => 'a',
        'Ą' | 'Á' | 'À' | 'Â' | 'Ä' | 'Ã' => 'A',
        'ć' | 'č' | 'ç' => 'c',
        'Ć' | 'Č' | 'Ç' => 'C',
        'ę' | 'é' | 'è' | 'ê' | 'ë' => 'e',
        'Ę' | 'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'ł' => 'l',
        'Ł' => 'L',
        'ń' | 'ñ' => 'n',
        'Ń' | 'Ñ' => 'N',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
        'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Õ' => 'O',
        'ś' | 'š' => 's',
        'Ś' | 'Š' => 'S',
        'ź' | 'ż' | 'ž' => 'z',
        'Ź' | 'Ż' | 'Ž' => 'Z',
        'ü' => 'u',
        'Ü' => 'U',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        c if c.is_ascii() && !c.is_ascii_control() => c,
        _ => '?',
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
    Splash,
    /// Soft-AP wizard page 2 on 128×64: QR + HTTP URL.
    PairingQr,
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
    pub battery: Option<BatterySample>,
    pub wifi_link: Option<WifiLink>,
    pub sta_net: Option<StaNet>,
    pub ping: PingStatus,
    pub sta_ipv4: Option<[u8; 4]>,
    pub http_ota: bool,
    pub http_ota_busy: bool,
}
