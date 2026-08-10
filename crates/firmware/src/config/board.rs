//! Board Support Package: central, single board pinout.
//!
//! ALL physical GPIO numbers live here. Board changes / tuning for
//! ESP32-C6-DevKitC-1U = edit only this file.

/// GPIO pin number (raw index on the package).
pub type Gpio = u8;

// --- I2C bus (OLED SSD1306 + MCP23017 x2) ---
pub const I2C_SDA: Gpio = 6;
pub const I2C_SCL: Gpio = 7;
pub const I2C_FREQ_KHZ: u32 = 400;

// --- OLED display (I2C) ---
pub const OLED_I2C_ADDRESS: u8 = 0x3C;

// --- MCP23017 I2C expanders (Kamod IOEXP16) ---
pub const MCP0_I2C_ADDRESS: u8 = 0x20;
pub const MCP1_I2C_ADDRESS: u8 = 0x21;

// --- Rotary encoder (KY-040 / EC11) ---
pub const ENCODER_A: Gpio = 2;
pub const ENCODER_B: Gpio = 3;
pub const ENCODER_BUTTON: Gpio = 0;

// --- Nav cluster on ESP GPIO (active-low, internal pull-up) ---
// Header pins 18–23 on DevKitC-1 (avoid strapping 4/5/8/9/15 — weak/no effect in Wokwi).
pub const NAV_UP: Gpio = 18;
pub const NAV_DOWN: Gpio = 19;
pub const NAV_LEFT: Gpio = 20;
pub const NAV_RIGHT: Gpio = 21;
pub const NAV_OK: Gpio = 22;
pub const NAV_BACK: Gpio = 23;
pub const NAV_MENU: Gpio = 10;

// --- Battery measurement (ADC) ---
pub const BATTERY_ADC: Gpio = 1;

// --- Deep sleep wake (LP GPIO0) ---
pub const WAKE_PIN: Gpio = 0;

// --- Optional MCP23017 INTA (unused; Menu took GPIO10) ---
pub const MCP_INT: Gpio = 11;

// --- Heiko wiFred status LEDs (active-high; steal in LedPresenter) ---
pub const HEIKO_LED_STOP: Gpio = 18;
pub const HEIKO_LED_FORWARD: Gpio = 19;
pub const HEIKO_LED_REVERSE: Gpio = 20;

// Legacy aliases for display module.
pub const OLED_SDA: Gpio = I2C_SDA;
pub const OLED_SCL: Gpio = I2C_SCL;
pub const OLED_I2C_FREQ_KHZ: u32 = I2C_FREQ_KHZ;

/// Logical button read from MCP23017 expanders.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LogicalButton {
    JoyUp,
    JoyDown,
    JoyLeft,
    JoyRight,
    JoyOk,
    Menu,
    Back,
    EStop,
    F0,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    Direction,
}

/// (expander address, port A/B, bit) -> logical button. `None` = unused / moved to GPIO.
pub const BUTTON_MAP: [(u8, bool, u8, Option<LogicalButton>); 20] = [
    // MCP #0 port A — nav/back/menu are on ESP GPIO (see NAV_*)
    (MCP0_I2C_ADDRESS, true, 0, None),
    (MCP0_I2C_ADDRESS, true, 1, None),
    (MCP0_I2C_ADDRESS, true, 2, None),
    (MCP0_I2C_ADDRESS, true, 3, None),
    (MCP0_I2C_ADDRESS, true, 4, None),
    (MCP0_I2C_ADDRESS, true, 5, None),
    (MCP0_I2C_ADDRESS, true, 6, None),
    (MCP0_I2C_ADDRESS, true, 7, Some(LogicalButton::EStop)),
    // MCP #0 port B
    (MCP0_I2C_ADDRESS, false, 0, Some(LogicalButton::F0)),
    (MCP0_I2C_ADDRESS, false, 1, Some(LogicalButton::F1)),
    (MCP0_I2C_ADDRESS, false, 2, Some(LogicalButton::F2)),
    (MCP0_I2C_ADDRESS, false, 3, Some(LogicalButton::F3)),
    (MCP0_I2C_ADDRESS, false, 4, Some(LogicalButton::F4)),
    (MCP0_I2C_ADDRESS, false, 5, Some(LogicalButton::F5)),
    (MCP0_I2C_ADDRESS, false, 6, Some(LogicalButton::F6)),
    (MCP0_I2C_ADDRESS, false, 7, Some(LogicalButton::F7)),
    // MCP #1 port A
    (MCP1_I2C_ADDRESS, true, 0, Some(LogicalButton::F8)),
    (MCP1_I2C_ADDRESS, true, 1, Some(LogicalButton::F9)),
    (MCP1_I2C_ADDRESS, true, 2, Some(LogicalButton::F10)),
    (MCP1_I2C_ADDRESS, true, 3, Some(LogicalButton::Direction)),
];

pub const MCP_ADDRESSES: [u8; 2] = [MCP0_I2C_ADDRESS, MCP1_I2C_ADDRESS];
