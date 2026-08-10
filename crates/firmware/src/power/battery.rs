//! Battery measurement (ADC) and charge-level publication.

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Watch;
use embassy_time::{Duration, Timer};
use esp_hal::analog::adc::{Adc, AdcConfig, Attenuation};

use crate::config::power;
use crate::power::sleep::{SLEEP_CTRL, SleepReason};

pub static BATTERY: Watch<CriticalSectionRawMutex, Option<u8>, 2> = Watch::new();

fn volts_to_percent(volts: f32) -> u8 {
    if volts >= 4.2 {
        return 100;
    }
    if volts <= 3.2 {
        return 0;
    }
    (((volts - 3.2) / 1.0) * 100.0) as u8
}

#[embassy_executor::task]
pub async fn task(
    adc1: esp_hal::peripherals::ADC1<'static>,
    battery_pin: esp_hal::peripherals::GPIO1<'static>,
) {
    if !power::USE_BATTERY_TEST {
        return;
    }

    let mut adc_config = AdcConfig::new();
    let mut pin = adc_config.enable_pin(battery_pin, Attenuation::_11dB);
    let mut adc = Adc::new(adc1, adc_config);
    let tx = BATTERY.sender();

    loop {
        let mut sum = 0u32;
        let mut count = 0u32;
        for _ in 0..power::ADC_READS {
            if let Ok(v) = nb::block!(adc.read_oneshot(&mut pin)) {
                sum += v as u32;
                count += 1;
            }
        }
        if count > 0 {
            let raw = sum / count;
            let volts = raw as f32 * power::BATTERY_CONVERSION_FACTOR / 1000.0;
            let percent = volts_to_percent(volts);
            tx.send(Some(percent));
            if power::USE_BATTERY_SLEEP_AT_PERCENT > 0
                && percent < power::USE_BATTERY_SLEEP_AT_PERCENT
            {
                SLEEP_CTRL.signal(SleepReason::Battery);
            }
        }
        Timer::after(Duration::from_secs(power::BATTERY_POLL_S)).await;
    }
}
