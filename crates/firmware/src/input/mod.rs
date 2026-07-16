//! Input: GPIO nav cluster, MCP23017 tact/F-keys, encoder.
//! Input channel contract -> domain: `InputEvent` + `INPUT_CHANNEL`.

pub mod encoder;
pub mod expander;
pub mod gpio_nav;
pub mod i2c_bus;

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

/// Input event emitted to the domain layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    Nav(NavDir),
    Ok,
    Back,
    Menu,
    EStop,
    FnPress(u8),
    FnRelease(u8),
    DirectionSet(Direction),
    EncoderClockwise,
    EncoderCounterClockwise,
    EncoderButton,
}

pub const INPUT_CHANNEL_DEPTH: usize = 16;

pub type InputChannel = Channel<CriticalSectionRawMutex, InputEvent, INPUT_CHANNEL_DEPTH>;
pub type InputSender = Sender<'static, CriticalSectionRawMutex, InputEvent, INPUT_CHANNEL_DEPTH>;
pub type InputReceiver = Receiver<'static, CriticalSectionRawMutex, InputEvent, INPUT_CHANNEL_DEPTH>;

/// Sole input channel: drivers -> domain.
pub static INPUT_CHANNEL: InputChannel = Channel::new();
