//! Boot splash bitmap (1 bpp, host asset baked with `include_bytes!`).

/// Packed 1-bpp splash (128×56, MSB first, row-major). Black ink → bit set.
pub const SPLASH_RAW: &[u8] = include_bytes!("../../assets/splash_128x56.raw");
pub const SPLASH_WIDTH: u32 = 128;
pub const SPLASH_HEIGHT: u32 = 56;
pub const HINT: &str = "[STOP - Programming mode]";
