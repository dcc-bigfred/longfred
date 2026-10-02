//! Power configuration: battery and deep sleep.

/// Enables ADC battery measurement and icon on the throttle screen.
pub const USE_BATTERY_TEST: bool = true;

include!(concat!(env!("OUT_DIR"), "/battery_factor.rs"));

/// Default display mode: icon + percent (when `USE_BATTERY_TEST`).
pub const USE_BATTERY_PERCENT_WITH_ICON: bool = false;

/// Auto deep sleep when % < threshold; 0 = disabled.
pub const USE_BATTERY_SLEEP_AT_PERCENT: u8 = 5;

pub const BATTERY_POLL_S: u64 = 10;
/// Faster ADC while VBUS is present so Diagnostics can show a rising range.
pub const BATTERY_POLL_CHARGING_S: u64 = 2;
pub const ADC_READS: usize = 20;
/// Discarded conversions so the SAR hold cap can follow a high-Z divider.
pub const ADC_DUMMY_READS: usize = 8;
/// Pause between conversions (TinyC6 442 kΩ / 160 kΩ has no filter cap).
pub const ADC_SETTLE_MS: u64 = 5;

/// OLED panel off after this much input inactivity, ms, when no acquired
/// loco has speed > 0. 0 = disabled. Ignored on headless variants
/// (`display: None`).
pub const DISPLAY_BLANK_INACTIVITY_MS: u64 = 300_000;

/// Deep sleep after this much input inactivity, ms, when no acquired loco
/// has speed > 0. 0 = disabled.
pub const AUTO_SLEEP_INACTIVITY_MS: u64 = 900_000;

/// Sleep screen delay before deep sleep, ms.
pub const SLEEP_SCREEN_DELAY_MS: u64 = 2_000;
