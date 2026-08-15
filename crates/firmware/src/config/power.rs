//! Power configuration: battery and deep sleep.

/// Enables ADC battery measurement and icon on the throttle screen.
pub const USE_BATTERY_TEST: bool = true;

/// ADC-to-voltage scaling factor (hardware calibration, stage 11).
pub const BATTERY_CONVERSION_FACTOR: f32 = 1.7;

/// Default display mode: icon + percent (when `USE_BATTERY_TEST`).
pub const USE_BATTERY_PERCENT_WITH_ICON: bool = false;

/// Auto deep sleep when % < threshold; 0 = disabled.
pub const USE_BATTERY_SLEEP_AT_PERCENT: u8 = 5;

pub const BATTERY_POLL_S: u64 = 10;
pub const ADC_READS: usize = 20;

/// Inactivity auto-off (no `WiThrottle` server connection), ms.
/// 0 = disabled. The `sim` build does not spawn `sleep::task`, so this is a
/// no-op under Wokwi regardless of the value.
pub const AUTO_SLEEP_INACTIVITY_MS: u64 = 240_000;

/// Sleep screen delay before deep sleep, ms.
pub const SLEEP_SCREEN_DELAY_MS: u64 = 2_000;
