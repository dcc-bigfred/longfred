//! LongFred v1 custom PCB (ESP32-C6 QFN-40 + MCP23017×2 + OLED 128×32).
//!
//! Pin map and MCP bit map: `longfred-hardware/plans/004-schematic.md`.
//! Programming chord: Shift1 + Stop (Back) held 8 s. Joy/Menu/Back on MCP, not GPIO.

use crate::config::board::Gpio;

/// I2C SDA (GPIO6, package pin 12).
pub const I2C_SDA: Gpio = 6;
/// I2C SCL (GPIO7, package pin 13).
pub const I2C_SCL: Gpio = 7;

/// Encoder A (GPIO1, package pin 7).
pub const ENCODER_A: Gpio = 1;
/// Encoder B (GPIO2, package pin 8).
pub const ENCODER_B: Gpio = 2;
/// Encoder SW / deep-sleep wake (GPIO0, package pin 6).
pub const ENCODER_BUTTON: Gpio = 0;
pub const WAKE_PIN: Gpio = ENCODER_BUTTON;

/// VBAT divider on GPIO4 (`ADC1_CH4`), TinyC6 442 kΩ / 160 kΩ.
pub const BATTERY_ADC: Gpio = 4;
/// VBUS sense (USB present) on GPIO10.
pub const VBUS_PIN: Gpio = 10;
/// MCP23017 U5 INTA (open-drain).
pub const MCP_INT: Gpio = 11;

/// E-ink SPI (J_DISP 5–10). Unused while OLED is fitted; do not steal as GPIO nav.
pub const EPD_MOSI: Gpio = 18;
pub const EPD_SCK: Gpio = 19;
pub const EPD_CS: Gpio = 20;
pub const EPD_DC: Gpio = 21;
pub const EPD_RST: Gpio = 22;
pub const EPD_BUSY: Gpio = 23;

/// ADC counts → pack millivolts. Same TinyC6 divider; empirical scale, not 3.76.
pub const BATTERY_CONVERSION_FACTOR: f32 = 1.7;
