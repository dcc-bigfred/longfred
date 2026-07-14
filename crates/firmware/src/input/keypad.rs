//! Sterownik klawiatury matrycowej 4x3 (polling skan, async).
//! Piny z `config::board`, układ/timingi z `config::keypad`.

use embassy_time::{Duration, Timer};
use esp_hal::gpio::{AnyPin, Input, InputConfig, Level, Output, OutputConfig, Pull};

use crate::config::{board, keypad};
use super::{InputEvent, InputSender};

const SETTLE_US: u64 = 50;

/// Buduje piny (wiersze=Output, kolumny=Input pull-up) z BSP.
///
/// # Safety
///
/// Wywoływać raz, z `main`, bez równoległego użycia tych GPIO.
pub fn build() -> ([Output<'static>; keypad::ROWS], [Input<'static>; keypad::COLS]) {
    let out_cfg = OutputConfig::default();
    let in_cfg = InputConfig::default().with_pull(Pull::Up);
    let rows = core::array::from_fn(|i| {
        let pin = unsafe { AnyPin::steal(board::KEYPAD_ROW_PINS[i]) };
        Output::new(pin, Level::High, out_cfg)
    });
    let cols = core::array::from_fn(|i| {
        let pin = unsafe { AnyPin::steal(board::KEYPAD_COL_PINS[i]) };
        Input::new(pin, in_cfg)
    });
    (rows, cols)
}

#[embassy_executor::task]
pub async fn task(
    mut rows: [Output<'static>; keypad::ROWS],
    cols: [Input<'static>; keypad::COLS],
    sender: InputSender,
) {
    let mut state = [[false; keypad::COLS]; keypad::ROWS];
    loop {
        for r in 0..keypad::ROWS {
            rows[r].set_low();
            Timer::after(Duration::from_micros(SETTLE_US)).await;
            for c in 0..keypad::COLS {
                let now = cols[c].is_low();
                if now != state[r][c] {
                    state[r][c] = now;
                    let key = keypad::KEYMAP[r][c];
                    let ev = if now {
                        InputEvent::KeyPress(key)
                    } else {
                        InputEvent::KeyRelease(key)
                    };
                    let _ = sender.try_send(ev);
                }
            }
            rows[r].set_high();
        }
        Timer::after(Duration::from_millis(keypad::KEYPAD_DEBOUNCE_MS)).await;
    }
}
