//! 3×4 matrix keypad scanner (markwtech).
//!
//! Rows are driven low one at a time; columns are read with pull-ups (active-low).
//! Pin numbers come from [`crate::board::variants::markwtech`].

use embassy_time::{Duration, Timer};
use esp_hal::gpio::{AnyPin, Input, InputConfig, Level, Output, OutputConfig, Pull};

use crate::board::raw::{ButtonId, RawEvent, RawSender};
#[cfg(feature = "variant-markwtech")]
use crate::board::variants::markwtech::{KEYPAD_COL_PINS, KEYPAD_MAP, KEYPAD_ROW_PINS};

const POLL_MS: u64 = 15;
const DEBOUNCE_TICKS: u8 = 2;

pub struct Pins {
    pub rows: [Output<'static>; 4],
    pub cols: [Input<'static>; 3],
}

/// Build keypad GPIO from markwtech pin constants.
///
/// # Safety
///
/// Call once from `main`; pins must not overlap other drivers.
#[cfg(feature = "variant-markwtech")]
#[allow(unsafe_code)]
pub fn build() -> Pins {
    let out_cfg = OutputConfig::default();
    let in_cfg = InputConfig::default().with_pull(Pull::Up);
    // SAFETY: keypad row/col pins are reserved for this driver; single owner from `main`.
    let rows = [
        Output::new(
            unsafe { AnyPin::steal(KEYPAD_ROW_PINS[0]) },
            Level::High,
            out_cfg,
        ),
        Output::new(
            unsafe { AnyPin::steal(KEYPAD_ROW_PINS[1]) },
            Level::High,
            out_cfg,
        ),
        Output::new(
            unsafe { AnyPin::steal(KEYPAD_ROW_PINS[2]) },
            Level::High,
            out_cfg,
        ),
        Output::new(
            unsafe { AnyPin::steal(KEYPAD_ROW_PINS[3]) },
            Level::High,
            out_cfg,
        ),
    ];
    let cols = [
        Input::new(unsafe { AnyPin::steal(KEYPAD_COL_PINS[0]) }, in_cfg),
        Input::new(unsafe { AnyPin::steal(KEYPAD_COL_PINS[1]) }, in_cfg),
        Input::new(unsafe { AnyPin::steal(KEYPAD_COL_PINS[2]) }, in_cfg),
    ];
    Pins { rows, cols }
}

#[cfg(feature = "variant-markwtech")]
#[embassy_executor::task]
pub async fn task(mut pins: Pins, sender: RawSender) {
    // Debounced pressed state [row][col].
    let mut pressed = [[false; 3]; 4];
    let mut debounce = [[0u8; 3]; 4];

    loop {
        for r in 0..4 {
            // Idle: all rows high; scan one row low.
            for row in pins.rows.iter_mut() {
                row.set_high();
            }
            pins.rows[r].set_low();
            // Settle.
            Timer::after(Duration::from_micros(50)).await;

            for c in 0..3 {
                let raw_pressed = pins.cols[c].is_low();
                if raw_pressed == pressed[r][c] {
                    debounce[r][c] = 0;
                    continue;
                }
                debounce[r][c] = debounce[r][c].saturating_add(1);
                if debounce[r][c] < DEBOUNCE_TICKS {
                    continue;
                }
                pressed[r][c] = raw_pressed;
                debounce[r][c] = 0;
                let id = KEYPAD_MAP[r][c];
                let edge = if raw_pressed { "press" } else { "release" };
                match id {
                    ButtonId::Star => log::info!("input: keypad * {edge}"),
                    ButtonId::Hash => log::info!("input: keypad # {edge}"),
                    ButtonId::KeypadDigit(d) => log::info!("input: keypad {d} {edge}"),
                    other => log::info!("input: keypad {other:?} {edge}"),
                }
                let _ = sender.try_send(RawEvent::Button(id, raw_pressed));
            }
        }
        // Release drive.
        for row in pins.rows.iter_mut() {
            row.set_high();
        }
        Timer::after(Duration::from_millis(POLL_MS)).await;
    }
}
