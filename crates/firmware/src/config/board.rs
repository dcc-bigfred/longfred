//! Shared board constants.
//!
//! LongFred v1 MCP map lives here. GPIO numbers for the active build come from
//! [`crate::board::pins`] (longfred-v1 or MarkWTech v1.1).

/// GPIO pin number (raw index on the package).
pub type Gpio = u8;

// --- I2C bus (OLED + expanders) ---
pub const I2C_FREQ_KHZ: u32 = 400;

// --- OLED display (I2C) ---
pub const OLED_I2C_ADDRESS: u8 = 0x3C;

// --- MCP23017 I2C expanders (Kamod IOEXP16) ---
pub const MCP0_I2C_ADDRESS: u8 = 0x20;
pub const MCP1_I2C_ADDRESS: u8 = 0x21;

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
    Direction,
    Shift1,
    Shift2,
}

/// GPA bit on MCP #1 used as the direction SPDT (active-low = Forward).
pub const MCP_DIR_PORT_A_BIT: u8 = 5;

/// MCP map for longfred-v1 PCB (`plans/004-schematic.md`). GPA7/GPB7 left unused (MCP23017 errata).
pub const BUTTON_MAP: [(u8, bool, u8, Option<LogicalButton>); 20] = [
    // U5 0x20 port A — F0–F6
    (MCP0_I2C_ADDRESS, true, 0, Some(LogicalButton::F0)),
    (MCP0_I2C_ADDRESS, true, 1, Some(LogicalButton::F1)),
    (MCP0_I2C_ADDRESS, true, 2, Some(LogicalButton::F2)),
    (MCP0_I2C_ADDRESS, true, 3, Some(LogicalButton::F3)),
    (MCP0_I2C_ADDRESS, true, 4, Some(LogicalButton::F4)),
    (MCP0_I2C_ADDRESS, true, 5, Some(LogicalButton::F5)),
    (MCP0_I2C_ADDRESS, true, 6, Some(LogicalButton::F6)),
    // U5 0x20 port B — F7, F8, Shift1/2, EStop, Menu, Back
    (MCP0_I2C_ADDRESS, false, 0, Some(LogicalButton::F7)),
    (MCP0_I2C_ADDRESS, false, 1, Some(LogicalButton::F8)),
    (MCP0_I2C_ADDRESS, false, 2, Some(LogicalButton::Shift1)),
    (MCP0_I2C_ADDRESS, false, 3, Some(LogicalButton::Shift2)),
    (MCP0_I2C_ADDRESS, false, 4, Some(LogicalButton::EStop)),
    (MCP0_I2C_ADDRESS, false, 5, Some(LogicalButton::Menu)),
    (MCP0_I2C_ADDRESS, false, 6, Some(LogicalButton::Back)),
    // U6 0x21 port A — joystick + direction (GPA5)
    (MCP1_I2C_ADDRESS, true, 0, Some(LogicalButton::JoyUp)),
    (MCP1_I2C_ADDRESS, true, 1, Some(LogicalButton::JoyDown)),
    (MCP1_I2C_ADDRESS, true, 2, Some(LogicalButton::JoyLeft)),
    (MCP1_I2C_ADDRESS, true, 3, Some(LogicalButton::JoyRight)),
    (MCP1_I2C_ADDRESS, true, 4, Some(LogicalButton::JoyOk)),
    (MCP1_I2C_ADDRESS, true, 5, Some(LogicalButton::Direction)),
];

pub const MCP_ADDRESSES: [u8; 2] = [MCP0_I2C_ADDRESS, MCP1_I2C_ADDRESS];
