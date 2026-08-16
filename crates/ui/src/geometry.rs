//! OLED layout geometry (independent of the SSD1306 driver).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayGeometry {
    pub width: u16,
    pub height: u16,
    pub grid_rows: usize,
    pub grid_cols: usize,
    pub grid_lines: usize,
}

pub const LAYOUT_128X64: DisplayGeometry = DisplayGeometry {
    width: 128,
    height: 64,
    grid_rows: 8,
    grid_cols: 21,
    grid_lines: 12,
};

pub const LAYOUT_128X32: DisplayGeometry = DisplayGeometry {
    width: 128,
    height: 32,
    grid_rows: 4,
    grid_cols: 21,
    grid_lines: 6,
};
