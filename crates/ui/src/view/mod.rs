//! OLED rendering model — pure data structures (no logic).

/// Maximum lines in a [`GridView`] (128×64 uses 8 visible; extras are unused).
pub const GRID_LINES: usize = 12;
/// Characters per OLED line (`FONT_6X10` × 6 px on a 128 px panel).
pub const LINE_LEN: usize = 21;

/// One OLED text line (ASCII, folded by [`push_oled`]).
pub type Line = heapless::String<LINE_LEN>;

/// Full-screen grid of text lines (menus, wizards, diagnostics).
#[derive(Clone, PartialEq, Eq)]
pub struct GridView {
    /// Line contents; missing indices are blank.
    pub lines: heapless::Vec<Line, GRID_LINES>,
    /// Bit `i` set ⇒ line `i` is drawn inverted.
    pub invert: u16,
    /// Draw a rule under the title row.
    pub top_line: bool,
    /// Draw a rule above the footer row.
    pub foot_line: bool,
    /// Caps Lock indicator: `Some(true)` = uppercase (arrow up).
    pub caps: Option<bool>,
}

impl GridView {
    /// Empty grid, footer rule on.
    #[must_use]
    pub fn new() -> Self {
        Self {
            lines: heapless::Vec::new(),
            invert: 0,
            top_line: false,
            foot_line: true,
            caps: None,
        }
    }

    /// True when row `idx` is drawn inverted.
    #[must_use]
    pub fn inverted(&self, idx: usize) -> bool {
        idx < GRID_LINES && (self.invert & (1u16 << idx)) != 0
    }

    fn set_inverted(&mut self, idx: usize, inv: bool) {
        if idx >= GRID_LINES {
            return;
        }
        let bit = 1u16 << idx;
        if inv {
            self.invert |= bit;
        } else {
            self.invert &= !bit;
        }
    }

    /// Write `text` on row `idx` and set invert.
    pub fn set(&mut self, idx: usize, text: &str, inv: bool) {
        if idx >= GRID_LINES {
            return;
        }
        while self.lines.len() <= idx {
            let _ = self.lines.push(Line::new());
        }
        self.lines[idx].clear();
        push_oled(&mut self.lines[idx], text);
        self.set_inverted(idx, inv);
    }
}

/// `FONT_6X10` is ASCII-only. Fold Latin extras and stop at [`LINE_LEN`] bytes
/// so a long SSID cannot fail `push_str` and render as a blank line.
pub fn push_oled(line: &mut Line, s: &str) {
    for c in s.chars() {
        if line.push(oled_char(c)).is_err() {
            break;
        }
    }
}

/// Full-width line in characters (`FONT_6X10` × 6 px on a 128 px panel).
#[must_use]
pub fn col_chars() -> usize {
    LINE_LEN
}

/// Sequential content rows for paged lists (header index 0 unused).
///
/// When `footer` is set, the last visible content row is reserved for a hint
/// (row 6 on 128×64, row 3 on 128×32) and is not a list slot.
#[must_use]
pub fn list_slots_for(height: u16, footer: bool) -> &'static [usize] {
    if height <= 32 {
        if footer { &[1, 2] } else { &[1, 2, 3] }
    } else if footer {
        &[1, 2, 3, 4, 5]
    } else {
        &[1, 2, 3, 4, 5, 6]
    }
}

/// Last visible content row — used for list / overlay hints.
#[must_use]
pub fn list_hint_row(height: u16) -> usize {
    if height <= 32 { 3 } else { 6 }
}

/// 1-based number drawn next to a list item (`global` 0 → `1:`).
#[must_use]
pub fn item_display_num(global: usize) -> usize {
    global.saturating_add(1)
}

/// Keypad `1`–`9` → those display numbers; `0` → 10.
#[must_use]
pub fn digit_display_num(n: u8) -> usize {
    if n == 0 { 10 } else { n as usize }
}

fn push_decimal_digit(buf: &mut heapless::String<64>, n: usize) {
    let d = u8::try_from(n % 10).unwrap_or(0);
    let _ = buf.push(char::from(b'0' + d));
}

fn push_item_num_prefix(buf: &mut heapless::String<64>, global: usize) {
    let n = item_display_num(global);
    if n >= 100 {
        push_decimal_digit(buf, n / 100);
    }
    if n >= 10 {
        push_decimal_digit(buf, n / 10);
    }
    push_decimal_digit(buf, n);
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
    if let Some(sp) = cur.rfind(' ')
        && sp > 0
        && sp + 1 < cur.len()
    {
        let mut next = Line::new();
        for ch in cur.chars().skip(sp + 1) {
            let _ = next.push(ch);
        }
        cur.truncate(sp);
        let _ = out.push(core::mem::replace(cur, next));
        return;
    }
    let _ = out.push(core::mem::replace(cur, Line::new()));
}

/// How many `items` starting at `start` fit on one list page.
#[must_use]
pub fn items_fitting(
    items: &[&str],
    start: usize,
    numbered: bool,
    height: u16,
    footer: bool,
) -> usize {
    let slots = list_slots_for(height, footer).len();
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
#[must_use]
pub fn page_start(items: &[&str], page: usize, numbered: bool, height: u16, footer: bool) -> usize {
    let mut idx = 0usize;
    for _ in 0..page {
        let n = items_fitting(items, idx, numbered, height, footer);
        if n == 0 {
            break;
        }
        idx += n;
    }
    idx
}

/// How many items fit on `page` given wrap and numbering.
#[must_use]
pub fn page_item_count(
    items: &[&str],
    page: usize,
    numbered: bool,
    height: u16,
    footer: bool,
) -> usize {
    let start = page_start(items, page, numbered, height, footer);
    items_fitting(items, start, numbered, height, footer)
}

/// True when another page exists after `page`.
#[must_use]
pub fn has_next_page(
    items: &[&str],
    page: usize,
    numbered: bool,
    height: u16,
    footer: bool,
) -> bool {
    let start = page_start(items, page, numbered, height, footer);
    start + items_fitting(items, start, numbered, height, footer) < items.len()
}

/// Place wrapped list items into grid slots using `list` page, cursor, and numbering.
pub fn fill_list_page(
    g: &mut GridView,
    items: &[&str],
    list: &crate::widgets::PagedList,
    height: u16,
) {
    fill_list_page_invert(g, items, list, height, |local, _global| {
        local == list.cursor
    });
}

/// Like [`fill_list_page`], with a custom invert predicate `(local, global) -> bool`.
pub fn fill_list_page_invert<F: Fn(usize, usize) -> bool>(
    g: &mut GridView,
    items: &[&str],
    list: &crate::widgets::PagedList,
    height: u16,
    invert: F,
) {
    g.foot_line = list.footer;
    let slots = list_slots_for(height, list.footer);
    let col = col_chars();
    let start = page_start(items, list.page, list.numbered, height, list.footer);
    let mut slot_i = 0usize;
    for (local, item) in items.iter().skip(start).enumerate() {
        let global = start + local;
        let mut folded = heapless::String::<64>::new();
        if list.numbered {
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

/// Drive HUD: speed, direction, loco name, battery.
#[derive(Clone, PartialEq, Eq)]
pub struct ThrottleView {
    /// 1-based roster / static-list position and catalogue length (`1/3`).
    /// `None` in address-only mode or when the current loco is not on the list.
    pub list_index: Option<(u8, u8)>,
    /// Speed step `0..=126`.
    pub speed: u8,
    /// `true` = forward.
    pub forward: bool,
    /// Consist size (1 = single loco).
    pub consist_len: u8,
    /// Command-station session is up (`ConnState::Connected`).
    pub server_connected: bool,
    /// Bitmask of DCC functions 0–31.
    pub functions: u32,
    /// Loco name / address line.
    pub loco: Line,
    /// Footer hints.
    pub footer: Line,
    /// Next-key hint (`MarkWTech`).
    pub next_hint: Line,
    /// Battery percent when known.
    pub battery: Option<u8>,
}

impl Default for ThrottleView {
    fn default() -> Self {
        Self {
            list_index: None,
            speed: 0,
            forward: true,
            consist_len: 0,
            server_connected: false,
            functions: 0,
            loco: Line::new(),
            footer: Line::new(),
            next_hint: Line::new(),
            battery: None,
        }
    }
}

/// Full-screen message covering whatever screen is underneath.
#[derive(Clone, PartialEq, Eq)]
pub struct OverlayView {
    /// Wrapped body plus footer, drawn like a [`GridView`].
    pub grid: GridView,
}

impl OverlayView {
    /// Wrap `text` and place `footer` on the last visible row.
    #[must_use]
    pub fn from_text(text: &str, footer: &str, height: u16) -> Self {
        let mut grid = GridView::new();
        grid.foot_line = height > 32;
        let chunks = wrap_text(text);
        let footer_row = list_hint_row(height);
        for (i, chunk) in chunks.iter().take(footer_row).enumerate() {
            grid.set(i, chunk.as_str(), false);
        }
        grid.set(footer_row, footer, false);
        Self { grid }
    }
}

/// Fold and wrap `text` at [`col_chars`] for overlay / multi-line bodies.
#[must_use]
pub fn wrap_text(text: &str) -> heapless::Vec<Line, 8> {
    let mut folded = heapless::String::<64>::new();
    for c in text.chars() {
        let _ = folded.push(oled_char(c));
    }
    wrap_chunks(folded.as_str(), col_chars())
}

/// What the firmware should paint this frame.
#[derive(Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum UiView {
    /// Drive HUD.
    Throttle(ThrottleView),
    /// Menu / wizard / diagnostics grid.
    Grid(GridView),
    /// Full-screen status / error overlay.
    Overlay(OverlayView),
    /// Boot splash bitmap / product name.
    Splash,
    /// Soft-AP wizard page 2 on 128×64: QR + HTTP URL.
    PairingQr,
}

impl Default for UiView {
    fn default() -> Self {
        UiView::Grid(GridView::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invert_bitmask_tracks_set_rows() {
        let mut g = GridView::new();
        g.set(0, "a", false);
        g.set(2, "c", true);
        assert!(!g.inverted(0));
        assert!(!g.inverted(1));
        assert!(g.inverted(2));
        g.set(2, "c", false);
        assert!(!g.inverted(2));
    }

    #[test]
    fn overlay_wraps_and_puts_footer_on_last_row() {
        let ov = OverlayView::from_text("vehicle_cap_exceeded extra", "EStop close", 64);
        assert!(ov.grid.lines[0].len() <= LINE_LEN);
        assert!(ov.grid.lines[0].as_str().starts_with("vehicle_cap"));
        assert_eq!(ov.grid.lines[6].as_str(), "EStop close");
    }

    #[test]
    fn overlay_footer_is_last_visible_row_on_32() {
        let ov = OverlayView::from_text("hello world extra", "OK", 32);
        assert_eq!(ov.grid.lines[3].as_str(), "OK");
        assert!(ov.grid.lines.get(3).is_some());
    }

    #[test]
    fn list_slots_reserve_last_row_when_footer() {
        assert_eq!(list_slots_for(64, false), &[1, 2, 3, 4, 5, 6]);
        assert_eq!(list_slots_for(64, true), &[1, 2, 3, 4, 5]);
        assert_eq!(list_slots_for(32, false), &[1, 2, 3]);
        assert_eq!(list_slots_for(32, true), &[1, 2]);
        assert_eq!(list_hint_row(64), 6);
        assert_eq!(list_hint_row(32), 3);
    }
}
