//! Deep sleep with GPIO wake (LP pin).

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Timer};
use esp_hal::gpio::RtcPinWithResistors;
use esp_hal::rtc_cntl::Rtc;
use esp_hal::rtc_cntl::sleep::{Ext1WakeupSource, WakeupLevel};

use crate::config::power;
use crate::ui::UI_VIEW;
use crate::ui::i18n;
use crate::ui::view::{GridView, UiView};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SleepReason {
    Command,
    Inactivity,
    Battery,
}

pub static SLEEP_CTRL: Signal<CriticalSectionRawMutex, SleepReason> = Signal::new();

/// Turn the OLED panel back on (no-op for subscribers if already on).
pub fn unblank_display() {
    crate::ui::DISPLAY_ON.sender().send(true);
}

/// Show the sleep screen and enter deep sleep after the reason-specific delay.
pub fn begin_sleep(reason: SleepReason) {
    unblank_display();
    SLEEP_CTRL.signal(reason);
}

fn sleep_view(reason: SleepReason) -> UiView {
    let mut g = GridView::new();
    g.set(0, i18n::APP_NAME, false);
    match reason {
        SleepReason::Inactivity => g.set(2, i18n::tr().msg_auto_sleep, false),
        SleepReason::Battery => g.set(2, i18n::tr().msg_battery_sleep, false),
        SleepReason::Command => {}
    }
    g.set(3, i18n::tr().msg_start_sleep, false);
    UiView::Grid(g)
}

#[embassy_executor::task]
pub async fn task(
    lpwr: esp_hal::peripherals::LPWR<'static>,
    mut wake_gpio: esp_hal::peripherals::GPIO0<'static>,
) {
    loop {
        let reason = SLEEP_CTRL.wait().await;
        unblank_display();
        UI_VIEW.sender().send(sleep_view(reason));
        let delay = match reason {
            SleepReason::Inactivity | SleepReason::Battery => Duration::from_millis(10_000),
            SleepReason::Command => Duration::from_millis(power::SLEEP_SCREEN_DELAY_MS),
        };
        Timer::after(delay).await;

        let pin_slot: (&mut dyn RtcPinWithResistors, WakeupLevel) =
            (&mut wake_gpio, WakeupLevel::Low);
        let mut pins = [pin_slot];
        let wake = Ext1WakeupSource::new(&mut pins);
        let mut rtc = Rtc::new(lpwr);
        rtc.sleep_deep(&[&wake]);
    }
}
