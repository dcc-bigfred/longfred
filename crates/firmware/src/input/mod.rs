//! Wejście: klawiatura 4x3, enkoder, przyciski dodatkowe (Etap 2).
//! Kontrakt kanału input -> domena: `InputEvent` + `INPUT_CHANNEL`.

pub mod encoder;
pub mod keypad;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};

/// Zdarzenie wejścia emitowane do warstwy domeny.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    KeyPress(char),
    KeyRelease(char),
    EncoderClockwise,
    EncoderCounterClockwise,
    EncoderButton,
}

pub const INPUT_CHANNEL_DEPTH: usize = 16;

pub type InputChannel = Channel<CriticalSectionRawMutex, InputEvent, INPUT_CHANNEL_DEPTH>;
pub type InputSender = Sender<'static, CriticalSectionRawMutex, InputEvent, INPUT_CHANNEL_DEPTH>;
pub type InputReceiver = Receiver<'static, CriticalSectionRawMutex, InputEvent, INPUT_CHANNEL_DEPTH>;

/// Jedyny kanał wejścia: drivery -> domena (Etap 8) / logger (Etap 2).
pub static INPUT_CHANNEL: InputChannel = Channel::new();
