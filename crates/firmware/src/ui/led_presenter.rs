//! 3-LED status presenter for heiko-wifred (STOP / Forward / Reverse).

use embassy_time::{Duration, Instant, Timer};
use esp_hal::gpio::{AnyPin, Level, Output, OutputConfig};

use crate::config::board;

/// Device / drive indication mode (set by domain or boot path).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LedMode {
    /// Boot / WiFi connecting — STOP blinks 1 Hz.
    Boot,
    /// Pairing active — greens alternate in antiphase, STOP off.
    Pairing,
    /// Driving forward — solid forward green.
    DriveForward,
    /// Driving reverse — solid reverse green.
    DriveReverse,
    /// Emergency stop — solid STOP + blink direction green.
    EStop,
    /// Lost server connection — STOP blinks 1 Hz (greens keep last dir).
    Disconnect,
}

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Watch;

/// Domain → LED presenter. Two subscribers allowed (presenter + debug).
pub static LED_MODE: Watch<CriticalSectionRawMutex, LedMode, 2> = Watch::new_with(LedMode::Boot);

struct Leds {
    stop: Output<'static>,
    forward: Output<'static>,
    reverse: Output<'static>,
}

impl Leds {
    fn set(&mut self, stop: bool, forward: bool, reverse: bool) {
        if stop {
            self.stop.set_high();
        } else {
            self.stop.set_low();
        }
        if forward {
            self.forward.set_high();
        } else {
            self.forward.set_low();
        }
        if reverse {
            self.reverse.set_high();
        } else {
            self.reverse.set_low();
        }
    }
}

/// Steal status LED pins (call once from `main` under heiko feature).
///
/// # Safety
///
/// Caller must guarantee these pins are not used elsewhere. In practice this
/// is invoked exactly once from `main` before any other task touches the
/// heiko-wifred GPIOs, so the `steal` is sound by construction.
pub fn build() -> (Output<'static>, Output<'static>, Output<'static>) {
    let cfg = OutputConfig::default();
    // SAFETY: `HEIKO_LED_STOP` is reserved for this presenter; `main` calls
    // `build()` once before spawning the LED task, so no aliasing occurs.
    let stop = Output::new(
        unsafe { AnyPin::steal(board::HEIKO_LED_STOP) },
        Level::Low,
        cfg,
    );
    // SAFETY: `HEIKO_LED_FORWARD` is reserved for this presenter; single
    // owner established in `main` before any other task runs.
    let forward = Output::new(
        unsafe { AnyPin::steal(board::HEIKO_LED_FORWARD) },
        Level::Low,
        cfg,
    );
    // SAFETY: `HEIKO_LED_REVERSE` is reserved for this presenter; single
    // owner established in `main` before any other task runs.
    let reverse = Output::new(
        unsafe { AnyPin::steal(board::HEIKO_LED_REVERSE) },
        Level::Low,
        cfg,
    );
    (stop, forward, reverse)
}

#[embassy_executor::task]
pub async fn task(stop: Output<'static>, forward: Output<'static>, reverse: Output<'static>) {
    let mut leds = Leds {
        stop,
        forward,
        reverse,
    };
    let mut rx = match LED_MODE.receiver() {
        Some(r) => r,
        None => {
            log::error!("led_presenter: no receiver slot in LED_MODE");
            return;
        }
    };
    let mut mode = rx.try_get().unwrap_or(LedMode::Boot);
    let mut phase = false;
    let mut last_tick = Instant::now();

    loop {
        // Prefer mode updates; otherwise advance blink phase.
        match embassy_futures::select::select(
            rx.changed(),
            Timer::after(Duration::from_millis(125)),
        )
        .await
        {
            embassy_futures::select::Either::First(m) => {
                mode = m;
                phase = false;
                last_tick = Instant::now();
            }
            embassy_futures::select::Either::Second(()) => {
                let period_ms = match mode {
                    LedMode::Boot | LedMode::Disconnect => 500, // 1 Hz half-period
                    LedMode::Pairing | LedMode::EStop => 250,   // 2 Hz half / alternate
                    LedMode::DriveForward | LedMode::DriveReverse => 1000,
                };
                if last_tick.elapsed().as_millis() >= period_ms {
                    phase = !phase;
                    last_tick = Instant::now();
                }
            }
        }

        match mode {
            LedMode::Boot | LedMode::Disconnect => {
                leds.set(phase, false, false);
            }
            LedMode::Pairing => {
                // Greens alternate; STOP off.
                leds.set(false, phase, !phase);
            }
            LedMode::DriveForward => leds.set(false, true, false),
            LedMode::DriveReverse => leds.set(false, false, true),
            LedMode::EStop => {
                // Solid STOP; blink the last direction green (forward by convention).
                leds.set(true, phase, false);
            }
        }
    }
}
