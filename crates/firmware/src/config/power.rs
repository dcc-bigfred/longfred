//! Power configuration: battery and deep sleep.

/// Enables ADC battery measurement and icon on the throttle screen.
pub const USE_BATTERY_TEST: bool = true;

/// ADC-to-voltage scaling factor (hardware calibration, stage 11).
pub const BATTERY_CONVERSION_FACTOR: f32 = 1.7;

/// Default display mode: icon + percent (when USE_BATTERY_TEST).
pub const USE_BATTERY_PERCENT_WITH_ICON: bool = false;

/// Auto deep sleep when % < threshold; 0 = disabled.
pub const USE_BATTERY_SLEEP_AT_PERCENT: u8 = 0;

pub const BATTERY_POLL_S: u64 = 10;
pub const ADC_READS: usize = 20;

/// Inactivity auto-off (no WiThrottle server connection), ms.
/// Set 0 to disable (recommended under Wokwi — deep sleep appears as reboot loop).
pub const AUTO_SLEEP_INACTIVITY_MS: u64 = 0;

/// Sleep screen delay before deep sleep, ms.
pub const SLEEP_SCREEN_DELAY_MS: u64 = 2_000;
