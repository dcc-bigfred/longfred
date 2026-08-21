//! OLED layout geometry (independent of the SSD1306 driver).

/// Pixel size and character grid for one OLED panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayGeometry {
    /// Panel width in pixels.
    pub width: u16,
    /// Panel height in pixels.
    pub height: u16,
    /// Character rows (`FONT_6X10`).
    pub grid_rows: usize,
    /// Character columns.
    pub grid_cols: usize,
    /// Logical lines in [`crate::view::GridView`] (may exceed visible rows).
    pub grid_lines: usize,
}

/// 128×64 SSD1306 used on `LongFred` / `MarkWTech`.
pub const LAYOUT_128X64: DisplayGeometry = DisplayGeometry {
    width: 128,
    height: 64,
    grid_rows: 8,
    grid_cols: 21,
    grid_lines: 12,
};

/// 128×32 SSD1306 used on the mini variant.
pub const LAYOUT_128X32: DisplayGeometry = DisplayGeometry {
    width: 128,
    height: 32,
    grid_rows: 4,
    grid_cols: 21,
    grid_lines: 6,
};
