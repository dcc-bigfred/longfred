//! Power configuration: battery and deep sleep.

/// Enables ADC battery measurement and icon on the throttle screen.
pub const USE_BATTERY_TEST: bool = true;

/// ADC-to-voltage scaling factor (hardware calibration).
///
/// LongFred / Heiko / MarkWTech v1.0 use the 1:2 divider factor. MarkWTech v1.1
/// (TinyC6 onboard VBAT sense) overrides this from the variant pin map.
pub const BATTERY_CONVERSION_FACTOR: f32 = crate::board::pins::BATTERY_CONVERSION_FACTOR;

/// Default display mode: icon + percent (when `USE_BATTERY_TEST`).
pub const USE_BATTERY_PERCENT_WITH_ICON: bool = false;

/// Auto deep sleep when % < threshold; 0 = disabled.
pub const USE_BATTERY_SLEEP_AT_PERCENT: u8 = 5;

pub const BATTERY_POLL_S: u64 = 10;
pub const ADC_READS: usize = 20;

/// OLED panel off after this much input inactivity, ms.
/// 0 = disabled. Ignored on headless variants (`display: None`).
pub const DISPLAY_BLANK_INACTIVITY_MS: u64 = 30_000;

/// Deep sleep after this much input inactivity, ms, when no acquired loco
/// has speed > 0. 0 = disabled.
pub const AUTO_SLEEP_INACTIVITY_MS: u64 = 300_000;

/// Sleep screen delay before deep sleep, ms.
pub const SLEEP_SCREEN_DELAY_MS: u64 = 2_000;
