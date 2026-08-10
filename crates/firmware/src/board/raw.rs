//! Raw hardware events (pre–ControlSurface mapping).

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonId {
    Stop,
    Shift1,
    Shift2,
    JoyUp,
    JoyDown,
    JoyLeft,
    JoyRight,
    JoyMenu,
    Direction,
    F0,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    /// Keypad digit 0–9 (markwtech / heiko).
    KeypadDigit(u8),
    Menu,
    Hash,
    Star,
    Extra(u8),
    EncoderButton,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AnalogId {
    SpeedPot,
    Battery,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SwitchId {
    Direction,
    Loco(u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RawEvent {
    /// `pressed = true` on press, `false` on release.
    Button(ButtonId, bool),
    Encoder(i8),
    Analog(AnalogId, u16),
    Switch(SwitchId, u8),
}

pub const RAW_CHANNEL_DEPTH: usize = 32;

pub type RawChannel = Channel<CriticalSectionRawMutex, RawEvent, RAW_CHANNEL_DEPTH>;
pub type RawSender = Sender<'static, CriticalSectionRawMutex, RawEvent, RAW_CHANNEL_DEPTH>;
pub type RawReceiver = Receiver<'static, CriticalSectionRawMutex, RawEvent, RAW_CHANNEL_DEPTH>;

/// Drivers -> board ControlSurface bridge.
pub static RAW_CHANNEL: RawChannel = Channel::new();
