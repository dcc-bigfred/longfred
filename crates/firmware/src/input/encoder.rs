//! Rotary encoder (Gray-code quadrature + detent) and encoder button.
//! Emits [`RawEvent`] for the board ControlSurface bridge.

use embassy_time::{Duration, Timer};
use esp_hal::gpio::{AnyPin, Input, InputConfig, Pull};

use crate::board::pins as board;
use crate::board::raw::{ButtonId, RawEvent, RawSender};
use crate::input::quadrature::QuadratureDecoder;

const BTN_DEBOUNCE_MS: u64 = 50;
const POLL_MS: u64 = 1;

pub struct Pins {
    pub a: Input<'static>,
    pub b: Input<'static>,
    pub button: Input<'static>,
}

/// Steal encoder GPIO pins (call once from `main`).
///
/// # Safety
///
/// Caller must guarantee these pins are not used elsewhere. Invoked exactly
/// once from `main` before encoder tasks run, so `steal` is sound by construction.
#[allow(unsafe_code)]
pub fn build() -> Pins {
    let cfg = InputConfig::default().with_pull(Pull::Up);
    Pins {
        // SAFETY: `ENCODER_A` is reserved for this driver; single owner from `main`.
        a: Input::new(unsafe { AnyPin::steal(board::ENCODER_A) }, cfg),
        // SAFETY: `ENCODER_B` is reserved for this driver; single owner from `main`.
        b: Input::new(unsafe { AnyPin::steal(board::ENCODER_B) }, cfg),
        // SAFETY: `ENCODER_BUTTON` is reserved for this driver; single owner from `main`.
        button: Input::new(unsafe { AnyPin::steal(board::ENCODER_BUTTON) }, cfg),
    }
}

#[embassy_executor::task]
pub async fn task(a: Input<'static>, b: Input<'static>, sender: RawSender) {
    let mut decoder = QuadratureDecoder::new(a.is_high(), b.is_high());
    loop {
        Timer::after(Duration::from_millis(POLL_MS)).await;
        if let Some(delta) = decoder.update(a.is_high(), b.is_high()) {
            log::info!("input: encoder {}", if delta > 0 { "CW" } else { "CCW" });
            let _ = sender.try_send(RawEvent::Encoder(delta));
        }
    }
}

#[embassy_executor::task]
pub async fn button_task(mut button: Input<'static>, sender: RawSender) {
    loop {
        button.wait_for_falling_edge().await;
        Timer::after(Duration::from_millis(BTN_DEBOUNCE_MS)).await;
        if button.is_low() {
            log::info!("input: encoder SW press");
            let _ = sender.try_send(RawEvent::Button(ButtonId::EncoderButton, true));
            button.wait_for_high().await;
            log::info!("input: encoder SW release");
            let _ = sender.try_send(RawEvent::Button(ButtonId::EncoderButton, false));
        }
    }
}
