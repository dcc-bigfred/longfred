//! Battery measurement (ADC) and charge-level publication.

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Watch;
use embassy_time::{Duration, Timer};
use esp_hal::analog::adc::{Adc, AdcConfig, AdcPin, Attenuation};
use esp_hal::gpio::AnalogPin;
use esp_hal::peripherals::ADC1;

use crate::config::{power, sizes};
use crate::net::{self, PROTO_COMMANDS};
use crate::power::sleep::{self, SleepReason};
use longfred_proto::command::ClientCommand;

/// Latest ADC sample published for the throttle icon and Diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatterySample {
    pub percent: u8,
    pub millivolts: u16,
    pub raw: u16,
}

pub static BATTERY: Watch<CriticalSectionRawMutex, Option<BatterySample>, 2> = Watch::new();

fn volts_to_percent(volts: f32) -> u8 {
    if volts >= 4.2 {
        return 100;
    }
    if volts <= 3.2 {
        return 0;
    }
    (((volts - 3.2) / 1.0) * 100.0) as u8
}

async fn run<PIN>(adc1: ADC1<'static>, battery_pin: PIN)
where
    PIN: AnalogPin + esp_hal::analog::adc::AdcChannel + 'static,
{
    if !power::USE_BATTERY_TEST {
        return;
    }

    let mut adc_config = AdcConfig::new();
    let mut pin: AdcPin<PIN, ADC1> = adc_config.enable_pin(battery_pin, Attenuation::_11dB);
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
            // Suggested factor makes `raw * factor / 1000 == 4.2` on a full cell.
            if raw > 0 {
                let suggested = 4200.0 / raw as f32;
                let current = power::BATTERY_CONVERSION_FACTOR;
                log::info!(
                    "battery: raw={raw} volts={volts:.3} percent={percent} suggested_factor={suggested:.4} (current={current})"
                );
            }
            let millivolts = (raw as f32 * power::BATTERY_CONVERSION_FACTOR) as u16;
            tx.send(Some(BatterySample {
                percent,
                millivolts,
                raw: raw as u16,
            }));
            if power::USE_BATTERY_SLEEP_AT_PERCENT > 0
                && percent < power::USE_BATTERY_SLEEP_AT_PERCENT
            {
                for throttle in 0..sizes::MAX_THROTTLES as u8 {
                    let _ = PROTO_COMMANDS.try_send(ClientCommand::EStop { throttle });
                }
                net::set_http_ota_enabled(false);
                sleep::begin_sleep(SleepReason::Battery);
            }
        }
        Timer::after(Duration::from_secs(power::BATTERY_POLL_S)).await;
    }
}

#[cfg(not(feature = "variant-markwtech-v1-1"))]
#[embassy_executor::task]
pub async fn task(adc1: ADC1<'static>, battery_pin: esp_hal::peripherals::GPIO1<'static>) {
    run(adc1, battery_pin).await;
}

#[cfg(feature = "variant-markwtech-v1-1")]
#[embassy_executor::task]
pub async fn task(adc1: ADC1<'static>, battery_pin: esp_hal::peripherals::GPIO4<'static>) {
    run(adc1, battery_pin).await;
}
