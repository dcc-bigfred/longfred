//! MarkWTech v1.1 pin map: Unexpected Maker TinyC6.
//!
//! Header GPIO only (17 lines). GPIO 4 is the onboard VBAT divider (ADC, not
//! a goldpin). GPIO 10 (VBUS sense) is read by the battery task. GPIO 12/13
//! (USB) and 22/23 (NeoPixel) are unused.
//!
//! GPIO 9 is an input-only extra button (onboard BOOT in parallel). Do not
//! drive it as a keypad row.
//!
//! Wiring tables: `docs/hardware/markwtech/v1.1.md`.

use crate::config::board::Gpio;

/// Keypad rows R0–R3: J4 USB-end run IO21…IO18.
pub const KEYPAD_ROW_PINS: [Gpio; 4] = [21, 20, 19, 18];
/// Keypad columns C0–C2: continue the J4 run IO7, IO6, IO5.
pub const KEYPAD_COL_PINS: [Gpio; 3] = [7, 6, 5];

/// Extra tact switches: left, Stop, right, Back, Menu.
///
/// Left/Stop/Right on IO15 + UART IO16/IO17 (RST+GND gap after IO15).
/// Menu = GPIO 8 (strapping ignored when GPIO 9 is high). Back = GPIO 9
/// (BOOT; holding at reset enters download).
pub const EXTRA_BUTTON_PINS: [Gpio; 5] = [15, 16, 17, 9, 8];

/// OLED I2C next to J3 `3V3` / `GND` (GPIO matrix, not LP_I2C 6/7).
pub const I2C_SDA: Gpio = 3;
pub const I2C_SCL: Gpio = 2;

pub const ENCODER_A: Gpio = 1;
pub const ENCODER_B: Gpio = 11;
/// Encoder push button; LP GPIO, deep-sleep wake source.
pub const ENCODER_BUTTON: Gpio = 0;
pub const WAKE_PIN: Gpio = ENCODER_BUTTON;

/// Onboard VBAT sense divider on GPIO 4 (`ADC1_CH4`). Not broken out.
pub const BATTERY_ADC: Gpio = 4;

/// Onboard VBUS sense (USB present). Not on the header.
pub const VBUS_PIN: Gpio = 10;

/// UM TinyC6 VBAT divider 442 kΩ / 160 kΩ → (442+160)/160 ≈ 3.76.
/// Calibrate from the `suggested_factor` battery log on a full cell.
pub const BATTERY_CONVERSION_FACTOR: f32 = 3.76;
