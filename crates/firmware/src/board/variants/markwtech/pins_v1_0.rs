//! MarkWTech v1.0 pin map: ESP32-C6-DevKitC-1, grouped by harness.
//!
//! Physical connector layout (see `docs/hardware/markwtech.md`):
//! keypad on J3-4…J3-10, OLED on J1-5/6, encoder DT/CLK on J1-3/4, SW on J1-7,
//! left/right/stop on J1-10…12, back/menu on J3-13/14.

use crate::config::board::Gpio;

/// Keypad matrix row GPIOs (driven, active-low scan): R0–R3.
pub const KEYPAD_ROW_PINS: [Gpio; 4] = [18, 19, 20, 21];
/// Keypad matrix column GPIOs (inputs with pull-up): C0, C1, C2.
///
/// C2 is GPIO 15 so the seven keypad lines sit on J3-4…J3-10.
pub const KEYPAD_COL_PINS: [Gpio; 3] = [22, 23, 15];

/// Extra tact switches (active-low, internal pull-up): left, Stop, right, Back, Menu.
pub const EXTRA_BUTTON_PINS: [Gpio; 5] = [10, 2, 11, 13, 12];

pub const I2C_SDA: Gpio = 6;
pub const I2C_SCL: Gpio = 7;

pub const ENCODER_A: Gpio = 4;
pub const ENCODER_B: Gpio = 5;
/// Encoder push button; LP GPIO, deep-sleep wake source.
pub const ENCODER_BUTTON: Gpio = 0;
pub const WAKE_PIN: Gpio = ENCODER_BUTTON;

/// External 47 kΩ / 47 kΩ divider into GPIO 1 (`ADC1_CH1`).
pub const BATTERY_ADC: Gpio = 1;

/// ADC-to-battery millivolt factor for the 1:2 divider (calibrate via log).
pub const BATTERY_CONVERSION_FACTOR: f32 = 1.7;
