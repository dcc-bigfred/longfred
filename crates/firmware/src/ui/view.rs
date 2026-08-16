//! OLED rendering model consumed by the SSD1306 presenter (no menu logic).

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

impl Default for GridView {
    fn default() -> Self {
        Self::new()
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
