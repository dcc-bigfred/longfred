//! MarkWTech / WiTcontroller-style ControlSurface.
//!
//! Hardware: 3×4 keypad matrix, extra tact cluster, KY-040 encoder, OLED 128×64
//! (SSD1309), no MCP expanders. Programming chord: Star (`*`) + Stop held for 8 s.

use embassy_time::Instant;

use crate::board::ControlSurface;
use crate::board::chord::{ChordDetector, PROGRAMMING_CHORD_MS};
use crate::board::descriptor::{LAYOUT_128X64, VariantDescriptor};
use crate::board::raw::{AnalogId, ButtonId, RawEvent, SwitchId};
use crate::config::board::Gpio;
use crate::input::{InputEvent, NavDir};

/// Keypad matrix row GPIOs (driven, active-low scan).
pub const KEYPAD_ROW_PINS: [Gpio; 4] = [18, 19, 20, 21];
/// Keypad matrix column GPIOs (inputs with pull-up).
pub const KEYPAD_COL_PINS: [Gpio; 3] = [22, 23, 10];

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

/// Extra tact switches (active-low, internal pull-up): left, Stop, right, Back, Menu.
///
/// ESP32-C6-WROOM-1 does not expose GPIO 14, and GPIO 1 is the battery ADC, so the
/// cluster uses 4/5 (strapping, harmless with default eFuses) and 12 (USB_D-).
/// UART0 (16/17) stays free for the serial console.
pub const EXTRA_BUTTON_PINS: [Gpio; 5] = [11, 12, 4, 5, 15];
pub const EXTRA_BUTTON_MAP: [ButtonId; 5] = [
    ButtonId::JoyLeft,
    ButtonId::Stop,
    ButtonId::JoyRight,
    ButtonId::Back,
    ButtonId::Menu,
];
/// Silkscreen names matching `docs/hardware/markwtech.md` (same order as [`EXTRA_BUTTON_PINS`]).
pub const EXTRA_BUTTON_NAMES: [&str; 5] = ["Menu left", "Stop", "Menu right", "Back", "Menu"];

pub const DESCRIPTOR: VariantDescriptor = VariantDescriptor {
    id: "markwtech",
    name: "MarkWTech",
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
