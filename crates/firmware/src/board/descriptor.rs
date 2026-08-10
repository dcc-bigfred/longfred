//! Hardware variant descriptors.

#[derive(Clone, Copy, Debug)]
pub struct DisplayGeometry {
    pub width: u16,
    pub height: u16,
    pub grid_rows: usize,
    pub grid_cols: usize,
    pub grid_lines: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct VariantDescriptor {
    pub id: &'static str,
    pub name: &'static str,
    pub mcu: &'static str,
    pub display: Option<DisplayGeometry>,
    pub has_expanders: bool,
    pub has_encoder: bool,
    pub has_keypad: bool,
    pub has_pot: bool,
    pub auto_pair_when_unconfigured: bool,
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
