//! Heiko wiFred ControlSurface (headless: pot + expanders + LEDs).
//!
//! Hardware: 2× MCP23017 (F0–F8, yellow Shift1, red ESTOP), loco slot switches,
//! 3-position direction switch, speed pot on ADC. No display / encoder.
//! Programming chord: Shift1 + Stop (ESTOP) held for 8 s.
//! Only Shift1 layer: F0–F8 → F9–F16.

use embassy_time::Instant;
use longfred_proto::model::Direction;

use crate::board::ControlSurface;
use crate::board::chord::{ChordDetector, PROGRAMMING_CHORD_MS};
use crate::board::descriptor::VariantDescriptor;
use crate::board::raw::{AnalogId, ButtonId, RawEvent, SwitchId};
use crate::board::shift_layers::map_fn;
use crate::input::InputEvent;

pub const DESCRIPTOR: VariantDescriptor = VariantDescriptor {
    id: "heiko-wifred",
    name: "Heiko WiFred",
    mcu: "esp32c6",
    display: None,
    has_expanders: true,
    has_encoder: false,
    has_keypad: false,
    has_pot: true,
    auto_pair_when_unconfigured: true,
};

/// Maps wiFred raw events to domain `InputEvent`s.
pub struct HeikoWifredSurface {
    shift1: bool,
    stop: bool,
    chord: ChordDetector,
}

impl HeikoWifredSurface {
    pub const fn new() -> Self {
        Self {
            shift1: false,
            stop: false,
            chord: ChordDetector::new(),
        }
    }

    fn emit_fn(&self, key: u8, pressed: bool, out: &mut dyn FnMut(InputEvent)) {
        // Only shift1 → +9 (F9–F16); no shift2 on this hardware.
        let mapped = map_fn(key, self.shift1, false);
        if pressed {
            out(InputEvent::FnPress(mapped));
        } else {
            out(InputEvent::FnRelease(mapped));
        }
    }

    fn on_button(&mut self, id: ButtonId, pressed: bool, out: &mut dyn FnMut(InputEvent)) {
        match id {
            ButtonId::Shift1 => self.shift1 = pressed,
            ButtonId::Stop | ButtonId::EStop => {
                self.stop = pressed;
                if pressed {
                    out(InputEvent::EStop);
                }
            }
            ButtonId::F0 => self.emit_fn(0, pressed, out),
            ButtonId::F1 => self.emit_fn(1, pressed, out),
            ButtonId::F2 => self.emit_fn(2, pressed, out),
            ButtonId::F3 => self.emit_fn(3, pressed, out),
            ButtonId::F4 => self.emit_fn(4, pressed, out),
            ButtonId::F5 => self.emit_fn(5, pressed, out),
            ButtonId::F6 => self.emit_fn(6, pressed, out),
            ButtonId::F7 => self.emit_fn(7, pressed, out),
            ButtonId::F8 => self.emit_fn(8, pressed, out),
            _ => {}
        }
    }

    fn maybe_chord(&mut self, now: Instant, out: &mut dyn FnMut(InputEvent)) {
        let now_ms = now.as_millis();
        if self
            .chord
            .update(self.shift1, self.stop, now_ms, PROGRAMMING_CHORD_MS)
        {
            out(InputEvent::EnterProgrammingMode);
        }
    }
}

impl Default for HeikoWifredSurface {
    fn default() -> Self {
        Self::new()
    }
}

impl ControlSurface for HeikoWifredSurface {
    fn descriptor(&self) -> &'static VariantDescriptor {
        &DESCRIPTOR
    }

    fn on_raw(&mut self, ev: RawEvent, now: Instant, out: &mut dyn FnMut(InputEvent)) {
        match ev {
            RawEvent::Button(id, pressed) => self.on_button(id, pressed, out),
            RawEvent::Encoder(_) => {}
            RawEvent::Analog(AnalogId::SpeedPot, value) => {
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
        self.maybe_chord(now, out);
    }

    fn tick(&mut self, now: Instant, out: &mut dyn FnMut(InputEvent)) {
        self.maybe_chord(now, out);
    }
}
