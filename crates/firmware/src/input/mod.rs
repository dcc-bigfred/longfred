//! Input events and the drivers every board shares (encoder, I2C).
//!
//! Keypad, extra buttons, and MCP expanders are compiled from
//! [`crate::board::variants`] so a variant feature is not named here.
//! Drivers emit [`crate::board::raw::RawEvent`] to `RAW_CHANNEL`;
//! the board bridge maps them to [`InputEvent`] on `INPUT_CHANNEL`.

pub mod encoder;
pub mod i2c_bus;
pub(crate) mod quadrature;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};
use longfred_proto::model::Direction;

/// Joystick navigation direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavDir {
    Up,
    Down,
    Left,
    Right,
}

/// Input event emitted to the domain / UI layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    Nav(NavDir),
    Ok,
    Back,
    Menu,
    EStop,
    /// Physical Stop — UiShell maps to EStop or Cancel/Back by screen.
    Stop,
    FnPress(u8),
    FnRelease(u8),
    DirectionSet(Direction),
    DirectionToggle,
    EncoderClockwise,
    EncoderCounterClockwise,
    EncoderButton,
    Digit(char),
    SpeedAbsolute(u8),
    LocoSlot(u8, bool),
    CharCycle(i8),
    CursorMove(i8),
    CaseToggle,
    EnterProgrammingMode,
}

pub const INPUT_CHANNEL_DEPTH: usize = 16;

pub type InputChannel = Channel<CriticalSectionRawMutex, InputEvent, INPUT_CHANNEL_DEPTH>;
pub type InputSender = Sender<'static, CriticalSectionRawMutex, InputEvent, INPUT_CHANNEL_DEPTH>;
pub type InputReceiver =
    Receiver<'static, CriticalSectionRawMutex, InputEvent, INPUT_CHANNEL_DEPTH>;

/// Sole input channel: board bridge -> domain.
pub static INPUT_CHANNEL: InputChannel = Channel::new();
