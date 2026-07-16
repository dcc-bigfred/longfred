//! Rotary encoder (quadrature on pin A edge) + encoder button.

use embassy_time::{Duration, Timer};
use esp_hal::gpio::{AnyPin, Input, InputConfig, Pull};

use crate::config::board;
use super::{InputEvent, InputSender};

const BTN_DEBOUNCE_MS: u64 = 50;

pub struct Pins {
    pub a: Input<'static>,
    pub b: Input<'static>,
    pub button: Input<'static>,
}

/// # Safety
///
/// Call once from `main`.
pub fn build() -> Pins {
    let cfg = InputConfig::default().with_pull(Pull::Up);
    Pins {
        a: Input::new(unsafe { AnyPin::steal(board::ENCODER_A) }, cfg),
        b: Input::new(unsafe { AnyPin::steal(board::ENCODER_B) }, cfg),
        button: Input::new(unsafe { AnyPin::steal(board::ENCODER_BUTTON) }, cfg),
    }
}

#[embassy_executor::task]
pub async fn task(mut a: Input<'static>, b: Input<'static>, sender: InputSender) {
    loop {
        a.wait_for_falling_edge().await;
        // Direction from B at the A edge (KY-040/EC11 detent).
        let cw = b.is_high();
        let ev = if cw {
            InputEvent::EncoderClockwise
        } else {
            InputEvent::EncoderCounterClockwise
        };
        let _ = sender.try_send(ev);
        Timer::after(Duration::from_millis(2)).await;
    }
}

#[embassy_executor::task]
pub async fn button_task(mut button: Input<'static>, sender: InputSender) {
    loop {
        button.wait_for_falling_edge().await;
        Timer::after(Duration::from_millis(BTN_DEBOUNCE_MS)).await;
        if button.is_low() {
            let _ = sender.try_send(InputEvent::EncoderButton);
            button.wait_for_high().await;
        }
    }
}
