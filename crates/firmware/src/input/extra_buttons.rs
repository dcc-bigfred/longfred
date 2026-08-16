//! Extra GPIO tact switches for MarkWTech (active-low, internal pull-up).
//!
//! Pin numbers come from [`crate::board::variants::markwtech`].

use embassy_time::{Duration, Timer};
use esp_hal::gpio::{AnyPin, Input, InputConfig, Pull};

use crate::board::raw::{RawEvent, RawSender};
use crate::board::variants::markwtech::{EXTRA_BUTTON_MAP, EXTRA_BUTTON_NAMES, EXTRA_BUTTON_PINS};

const POLL_MS: u64 = 20;
const DEBOUNCE_TICKS: u8 = 2;

pub struct Pins {
    pub buttons: [Input<'static>; 5],
}

/// Build extra-button GPIO from markwtech pin constants.
///
/// # Safety
///
/// Call once from `main`; pins must not overlap other drivers.
#[allow(unsafe_code)]
pub fn build() -> Pins {
    let cfg = InputConfig::default().with_pull(Pull::Up);
    // SAFETY: extra-button pins are reserved for this driver; single owner from `main`.
    Pins {
        buttons: [
            Input::new(unsafe { AnyPin::steal(EXTRA_BUTTON_PINS[0]) }, cfg),
            Input::new(unsafe { AnyPin::steal(EXTRA_BUTTON_PINS[1]) }, cfg),
            Input::new(unsafe { AnyPin::steal(EXTRA_BUTTON_PINS[2]) }, cfg),
            Input::new(unsafe { AnyPin::steal(EXTRA_BUTTON_PINS[3]) }, cfg),
            Input::new(unsafe { AnyPin::steal(EXTRA_BUTTON_PINS[4]) }, cfg),
        ],
    }
}

struct Btn {
    stable_high: bool,
    debounce: u8,
}

impl Btn {
    fn new(initial_high: bool) -> Self {
        Self {
            stable_high: initial_high,
            debounce: 0,
        }
    }

    /// Returns `Some(true)` on press (high→low), `Some(false)` on release.
    fn update(&mut self, raw_high: bool) -> Option<bool> {
        if raw_high == self.stable_high {
            self.debounce = 0;
            return None;
        }
        self.debounce = self.debounce.saturating_add(1);
        if self.debounce < DEBOUNCE_TICKS {
            return None;
        }
        let was_high = self.stable_high;
        self.stable_high = raw_high;
        self.debounce = 0;
        if was_high && !raw_high {
            Some(true)
        } else if !was_high && raw_high {
            Some(false)
        } else {
            None
        }
    }
}

#[embassy_executor::task]
pub async fn task(pins: Pins, sender: RawSender) {
    let mut state = [
        Btn::new(pins.buttons[0].is_high()),
        Btn::new(pins.buttons[1].is_high()),
        Btn::new(pins.buttons[2].is_high()),
        Btn::new(pins.buttons[3].is_high()),
        Btn::new(pins.buttons[4].is_high()),
    ];

    loop {
        for i in 0..5 {
            if let Some(pressed) = state[i].update(pins.buttons[i].is_high()) {
                let edge = if pressed { "press" } else { "release" };
                log::info!(
                    "input: {} GPIO {} {edge}",
                    EXTRA_BUTTON_NAMES[i],
                    EXTRA_BUTTON_PINS[i]
                );
                let _ = sender.try_send(RawEvent::Button(EXTRA_BUTTON_MAP[i], pressed));
            }
        }
        Timer::after(Duration::from_millis(POLL_MS)).await;
    }
}
