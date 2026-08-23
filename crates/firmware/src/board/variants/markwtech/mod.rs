//! MarkWTech / WiTcontroller-style ControlSurface.
//!
//! Hardware: 3×4 keypad matrix, extra tact cluster, KY-040 encoder, OLED 128×64
//! (SSD1309), no MCP expanders. Programming chord: Star (`*`) + Stop held for 8 s.
//!
//! GPIO numbers live in [`pins`] (`pins_v1_0` DevKitC-1 or `pins_v1_1` TinyC6).

pub mod pins;
#[cfg(not(feature = "variant-markwtech-v1-1"))]
mod pins_v1_0;
#[cfg(feature = "variant-markwtech-v1-1")]
mod pins_v1_1;

pub use pins::{
    BATTERY_ADC, BATTERY_CONVERSION_FACTOR, ENCODER_A, ENCODER_B, ENCODER_BUTTON,
    EXTRA_BUTTON_PINS, I2C_SCL, I2C_SDA, KEYPAD_COL_PINS, KEYPAD_ROW_PINS, WAKE_PIN,
};

use embassy_time::Instant;

use crate::board::ControlSurface;
use crate::board::chord::{ChordDetector, PROGRAMMING_CHORD_MS};
use crate::board::descriptor::{LAYOUT_128X64, VariantDescriptor};
use crate::board::raw::{AnalogId, ButtonId, RawEvent, SwitchId};
use crate::input::{InputEvent, NavDir};

/// Layout (row, col) → digit / star / hash:
/// ```text
///     C0   C1   C2
/// R0   1    2    3
/// R1   4    5    6
/// R2   7    8    9
/// R3   *    0    #
/// ```
pub const KEYPAD_MAP: [[ButtonId; 3]; 4] = [
    [
        ButtonId::KeypadDigit(1),
        ButtonId::KeypadDigit(2),
        ButtonId::KeypadDigit(3),
    ],
    [
        ButtonId::KeypadDigit(4),
        ButtonId::KeypadDigit(5),
        ButtonId::KeypadDigit(6),
    ],
    [
        ButtonId::KeypadDigit(7),
        ButtonId::KeypadDigit(8),
        ButtonId::KeypadDigit(9),
    ],
    [ButtonId::Star, ButtonId::KeypadDigit(0), ButtonId::Hash],
];

pub const EXTRA_BUTTON_MAP: [ButtonId; 5] = [
    ButtonId::JoyLeft,
    ButtonId::Stop,
    ButtonId::JoyRight,
    ButtonId::Back,
    ButtonId::Menu,
];
/// Silkscreen names matching `docs/hardware/markwtech/` (same order as [`EXTRA_BUTTON_PINS`]).
pub const EXTRA_BUTTON_NAMES: [&str; 5] = ["Menu left", "Stop", "Menu right", "Back", "Menu"];

#[cfg(not(feature = "variant-markwtech-v1-1"))]
const VARIANT_ID: &str = "markwtech";
#[cfg(feature = "variant-markwtech-v1-1")]
const VARIANT_ID: &str = "markwtech-v1.1";

#[cfg(not(feature = "variant-markwtech-v1-1"))]
const VARIANT_NAME: &str = "MarkWTech";
#[cfg(feature = "variant-markwtech-v1-1")]
const VARIANT_NAME: &str = "MarkWTech v1.1 (TinyC6)";

pub const DESCRIPTOR: VariantDescriptor = VariantDescriptor {
    id: VARIANT_ID,
    name: VARIANT_NAME,
    mcu: "esp32c6",
    display: Some(LAYOUT_128X64),
    has_expanders: false,
    has_encoder: true,
    has_keypad: true,
    has_pot: false,
    auto_pair_when_unconfigured: false,
};

/// Maps MarkWTech raw events to domain `InputEvent`s.
pub struct MarkwtechSurface {
    star: bool,
    stop: bool,
    chord: ChordDetector,
}

impl MarkwtechSurface {
    pub const fn new() -> Self {
        Self {
            star: false,
            stop: false,
            chord: ChordDetector::new(),
        }
    }

    fn on_button(&mut self, id: ButtonId, pressed: bool, out: &mut dyn FnMut(InputEvent)) {
        match id {
            ButtonId::Stop => {
                self.stop = pressed;
                if pressed {
                    out(InputEvent::Stop);
                }
            }
            ButtonId::Star => {
                self.star = pressed;
                if pressed {
                    // Surface emits Digit('*'); NavProfile / shell maps throttle * → MenuEnter.
                    out(InputEvent::Digit('*'));
                }
            }
            ButtonId::Hash if pressed => out(InputEvent::Digit('#')),
            ButtonId::KeypadDigit(d) if pressed && d <= 9 => {
                out(InputEvent::Digit((b'0' + d) as char));
            }
            ButtonId::JoyLeft if pressed => out(InputEvent::Nav(NavDir::Left)),
            ButtonId::JoyRight if pressed => out(InputEvent::Nav(NavDir::Right)),
            ButtonId::Back if pressed => out(InputEvent::Back),
            ButtonId::Extra(n) => {
                if pressed {
                    out(InputEvent::FnPress(n));
                } else {
                    out(InputEvent::FnRelease(n));
                }
            }
            ButtonId::EncoderButton if pressed => out(InputEvent::EncoderButton),
            ButtonId::Menu if pressed => out(InputEvent::Menu),
            ButtonId::F0
            | ButtonId::F1
            | ButtonId::F2
            | ButtonId::F3
            | ButtonId::F4
            | ButtonId::F5
            | ButtonId::F6
            | ButtonId::F7
            | ButtonId::F8 => {
                let key = match id {
                    ButtonId::F0 => 0,
                    ButtonId::F1 => 1,
                    ButtonId::F2 => 2,
                    ButtonId::F3 => 3,
                    ButtonId::F4 => 4,
                    ButtonId::F5 => 5,
                    ButtonId::F6 => 6,
                    ButtonId::F7 => 7,
                    ButtonId::F8 => 8,
                    _ => return,
                };
                if pressed {
                    out(InputEvent::FnPress(key));
                } else {
                    out(InputEvent::FnRelease(key));
                }
            }
            _ => {}
        }
    }

    fn maybe_chord(&mut self, now: Instant, out: &mut dyn FnMut(InputEvent)) {
        let now_ms = now.as_millis();
        if self
            .chord
            .update(self.star, self.stop, now_ms, PROGRAMMING_CHORD_MS)
        {
            out(InputEvent::EnterProgrammingMode);
        }
    }
}

impl Default for MarkwtechSurface {
    fn default() -> Self {
        Self::new()
    }
}

impl ControlSurface for MarkwtechSurface {
    fn descriptor(&self) -> &'static VariantDescriptor {
        &DESCRIPTOR
    }

    fn on_raw(&mut self, ev: RawEvent, now: Instant, out: &mut dyn FnMut(InputEvent)) {
        match ev {
            RawEvent::Button(id, pressed) => self.on_button(id, pressed, out),
            RawEvent::Encoder(delta) => {
                if delta > 0 {
                    out(InputEvent::EncoderClockwise);
                } else if delta < 0 {
                    out(InputEvent::EncoderCounterClockwise);
                }
            }
            RawEvent::Analog(AnalogId::SpeedPot, value) => {
                let speed = ((u32::from(value) * 126) / 4095).min(126) as u8;
                out(InputEvent::SpeedAbsolute(speed));
            }
            RawEvent::Analog(AnalogId::Battery, _) => {}
            RawEvent::Switch(SwitchId::Direction, v) => {
                let dir = if v != 0 {
                    longfred_proto::model::Direction::Forward
                } else {
                    longfred_proto::model::Direction::Reverse
                };
                out(InputEvent::DirectionSet(dir));
            }
            RawEvent::Switch(SwitchId::Loco(slot), v) => {
                // Throttle slots are 1-indexed; ignore stray slot 0 events.
                if slot >= 1 {
                    out(InputEvent::LocoSlot(slot, v != 0));
                }
            }
        }
        self.maybe_chord(now, out);
    }

    fn tick(&mut self, now: Instant, out: &mut dyn FnMut(InputEvent)) {
        self.maybe_chord(now, out);
    }
}
