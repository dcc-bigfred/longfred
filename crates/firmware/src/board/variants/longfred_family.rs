//! LongFred family ControlSurface (standard + mini).

use embassy_time::Instant;
use longfred_proto::model::Direction;

use crate::board::ControlSurface;
use crate::board::chord::{ChordDetector, PROGRAMMING_CHORD_MS};
use crate::board::descriptor::{LAYOUT_128X32, LAYOUT_128X64, VariantDescriptor};
use crate::board::raw::{AnalogId, ButtonId, RawEvent, SwitchId};
use crate::board::shift_layers::map_fn;
use crate::input::{InputEvent, NavDir};

pub const STANDARD: VariantDescriptor = VariantDescriptor {
    id: "longfred-standard",
    name: "LongFred Standard",
    mcu: "esp32c6",
    display: Some(LAYOUT_128X64),
    has_expanders: true,
    has_encoder: true,
    has_keypad: false,
    has_pot: false,
    auto_pair_when_unconfigured: false,
};

pub const MINI: VariantDescriptor = VariantDescriptor {
    id: "longfred-mini",
    name: "LongFred Mini",
    mcu: "esp32c6",
    display: Some(LAYOUT_128X32),
    has_expanders: true,
    has_encoder: true,
    has_keypad: false,
    has_pot: false,
    auto_pair_when_unconfigured: false,
};

/// Maps raw LongFred hardware events to domain `InputEvent`s.
pub struct LongFredSurface {
    descriptor: &'static VariantDescriptor,
    shift1: bool,
    shift2: bool,
    stop: bool,
    chord: ChordDetector,
}

impl LongFredSurface {
    pub const fn standard() -> Self {
        Self {
            descriptor: &STANDARD,
            shift1: false,
            shift2: false,
            stop: false,
            chord: ChordDetector::new(),
        }
    }

    pub const fn mini() -> Self {
        Self {
            descriptor: &MINI,
            shift1: false,
            shift2: false,
            stop: false,
            chord: ChordDetector::new(),
        }
    }

    fn emit_fn(&self, key: u8, pressed: bool, out: &mut dyn FnMut(InputEvent)) {
        let mapped = map_fn(key, self.shift1, self.shift2);
        if pressed {
            out(InputEvent::FnPress(mapped));
        } else {
            out(InputEvent::FnRelease(mapped));
        }
    }

    fn on_button(&mut self, id: ButtonId, pressed: bool, out: &mut dyn FnMut(InputEvent)) {
        match id {
            ButtonId::Shift1 => {
                let rising = pressed && !self.shift1;
                self.shift1 = pressed;
                if rising {
                    out(InputEvent::CaseToggle);
                }
            }
            ButtonId::Shift2 => self.shift2 = pressed,
            ButtonId::Stop => {
                self.stop = pressed;
                if pressed {
                    out(InputEvent::Stop);
                }
            }
            ButtonId::JoyUp if pressed => out(InputEvent::Nav(NavDir::Up)),
            ButtonId::JoyDown if pressed => out(InputEvent::Nav(NavDir::Down)),
            ButtonId::JoyLeft if pressed => out(InputEvent::Nav(NavDir::Left)),
            ButtonId::JoyRight if pressed => out(InputEvent::Nav(NavDir::Right)),
            // Center of 5-way = MenuEnter (Select when already in menu).
            ButtonId::JoyMenu if pressed => out(InputEvent::Menu),
            ButtonId::Menu if pressed => out(InputEvent::Menu),
            ButtonId::Direction if pressed => out(InputEvent::DirectionToggle),
            ButtonId::F0 => self.emit_fn(0, pressed, out),
            ButtonId::F1 => self.emit_fn(1, pressed, out),
            ButtonId::F2 => self.emit_fn(2, pressed, out),
            ButtonId::F3 => self.emit_fn(3, pressed, out),
            ButtonId::F4 => self.emit_fn(4, pressed, out),
            ButtonId::F5 => self.emit_fn(5, pressed, out),
            ButtonId::F6 => self.emit_fn(6, pressed, out),
            ButtonId::F7 => self.emit_fn(7, pressed, out),
            ButtonId::F8 => self.emit_fn(8, pressed, out),
            ButtonId::Extra(n) => {
                // Transitional F9/F10 (and other extras) bypass shift layers.
                if pressed {
                    out(InputEvent::FnPress(n));
                } else {
                    out(InputEvent::FnRelease(n));
                }
            }
            ButtonId::EncoderButton if pressed => out(InputEvent::EncoderButton),
            ButtonId::KeypadDigit(d) if pressed && d <= 9 => {
                out(InputEvent::Digit((b'0' + d) as char));
            }
            ButtonId::Hash if pressed => out(InputEvent::Digit('#')),
            ButtonId::Star if pressed => out(InputEvent::Digit('*')),
            ButtonId::JoyUp
            | ButtonId::JoyDown
            | ButtonId::JoyLeft
            | ButtonId::JoyRight
            | ButtonId::JoyMenu
            | ButtonId::Menu
            | ButtonId::Back
            | ButtonId::Direction
            | ButtonId::EncoderButton
            | ButtonId::KeypadDigit(_)
            | ButtonId::Hash
            | ButtonId::Star => {}
        }
    }
}

impl ControlSurface for LongFredSurface {
    fn descriptor(&self) -> &'static VariantDescriptor {
        self.descriptor
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
                // 12-bit-ish → 0..=126 speed step.
                let speed = ((u32::from(value) * 126) / 4095).min(126) as u8;
                out(InputEvent::SpeedAbsolute(speed));
            }
            RawEvent::Analog(AnalogId::Battery, _) => {}
            RawEvent::Switch(SwitchId::Direction, v) => {
                let dir = if v != 0 {
                    Direction::Forward
                } else {
                    Direction::Reverse
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

        let now_ms = now.as_millis();
        if self
            .chord
            .update(self.shift1, self.stop, now_ms, PROGRAMMING_CHORD_MS)
        {
            out(InputEvent::EnterProgrammingMode);
        }
    }

    fn tick(&mut self, now: Instant, out: &mut dyn FnMut(InputEvent)) {
        let now_ms = now.as_millis();
        if self
            .chord
            .update(self.shift1, self.stop, now_ms, PROGRAMMING_CHORD_MS)
        {
            out(InputEvent::EnterProgrammingMode);
        }
    }
}
