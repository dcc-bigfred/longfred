//! Battery measurement (ADC) and charge-level publication.

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Watch;
use embassy_time::{Duration, Instant, Timer};
use esp_hal::analog::adc::{Adc, AdcCalCurve, AdcConfig, Attenuation};
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
    /// Pack millivolts (`pin_mv * factor`).
    pub millivolts: u16,
    /// Calibrated millivolts at the ADC pin, averaged.
    pub pin_mv: u16,
    /// Lowest single pin sample this boot (Diagnostics: is the cell moving?).
    pub pin_mv_min: u16,
    /// Highest single pin sample this boot.
    pub pin_mv_max: u16,
    /// USB / VBUS present (charge in progress or at least plugged in).
    pub charging: bool,
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

async fn run<PIN>(
    adc1: ADC1<'static>,
    battery_pin: PIN,
    charging_pin: Option<esp_hal::gpio::Input<'static>>,
) where
    PIN: AnalogPin + esp_hal::analog::adc::AdcChannel + 'static,
{
    if !power::USE_BATTERY_TEST {
        return;
    }

    let mut adc_config = AdcConfig::new();
    // Curve fitting also applies the efuse bias (`AdcCalBasic`). Without it the
    // C6 clips its own output before the 12-bit truncation and the reading
    // sticks at a fixed code. Result is millivolts at the pin, so
    // `BATTERY_CONVERSION_FACTOR` is the divider ratio (Vbat / Vpin).
    let mut pin = adc_config
        .enable_pin_with_cal::<PIN, AdcCalCurve<ADC1<'static>>>(battery_pin, Attenuation::_11dB);
    let mut adc = Adc::new(adc1, adc_config);
    let tx = BATTERY.sender();
    let mut pin_mv_min = u16::MAX;
    let mut pin_mv_max = 0u16;
    let mut charging = false;

    loop {
        for _ in 0..power::ADC_DUMMY_READS {
            let _ = nb::block!(adc.read_oneshot(&mut pin));
            Timer::after(Duration::from_millis(power::ADC_SETTLE_MS)).await;
        }
        let mut sum = 0u32;
        let mut count = 0u32;
        for _ in 0..power::ADC_READS {
            Timer::after(Duration::from_millis(power::ADC_SETTLE_MS)).await;
            if let Ok(v) = nb::block!(adc.read_oneshot(&mut pin)) {
                pin_mv_min = pin_mv_min.min(v);
                pin_mv_max = pin_mv_max.max(v);
                sum += u32::from(v);
                count += 1;
            }
        }
        if count > 0 {
            let pin_mv = sum / count;
            let volts = pin_mv as f32 * power::BATTERY_CONVERSION_FACTOR / 1000.0;
            let percent = volts_to_percent(volts);
            charging = charging_pin.as_ref().is_some_and(|p| p.is_high());
            let millivolts = (pin_mv as f32 * power::BATTERY_CONVERSION_FACTOR) as u16;
            // Suggested factor makes `pin_mv * factor / 1000 == 4.2` on a full cell.
            if pin_mv > 0 {
                let suggested = 4200.0 / pin_mv as f32;
                let current = power::BATTERY_CONVERSION_FACTOR;
                log::info!(
                    "battery: pin_mv={pin_mv} volts={volts:.3} percent={percent} charging={charging} pin_span={pin_mv_min}-{pin_mv_max} suggested_factor={suggested:.4} (current={current})"
                );
            }
            tx.send(Some(BatterySample {
                percent,
                millivolts,
                pin_mv: pin_mv as u16,
                pin_mv_min,
                pin_mv_max,
                charging,
            }));
            if power::USE_BATTERY_SLEEP_AT_PERCENT > 0
                && percent < power::USE_BATTERY_SLEEP_AT_PERCENT
                && !charging
            {
                let deadline = Instant::now() + Duration::from_millis(200);
                for throttle in 0..sizes::MAX_THROTTLES as u8 {
                    let cmd = ClientCommand::EStop { throttle };
                    loop {
                        if PROTO_COMMANDS.try_send(cmd.clone()).is_ok() {
                            break;
                        }
                        if Instant::now() >= deadline {
                            log::warn!("battery: estop dropped, command channel full");
                            break;
                        }
                        Timer::after(Duration::from_millis(10)).await;
                    }
                }
                net::set_http_ota_enabled(false);
                sleep::begin_sleep(SleepReason::Battery);
            }
        }
        let poll_s = if charging {
            power::BATTERY_POLL_CHARGING_S
        } else {
            power::BATTERY_POLL_S
        };
        Timer::after(Duration::from_secs(poll_s)).await;
    }
}

#[cfg(not(feature = "variant-markwtech-v1-1"))]
#[embassy_executor::task]
pub async fn task(adc1: ADC1<'static>, battery_pin: esp_hal::peripherals::GPIO1<'static>) {
    run(adc1, battery_pin, None).await;
}

#[cfg(feature = "variant-markwtech-v1-1")]
#[embassy_executor::task]
pub async fn task(
    adc1: ADC1<'static>,
    battery_pin: esp_hal::peripherals::GPIO4<'static>,
    vbus_pin: esp_hal::peripherals::GPIO10<'static>,
) {
    use esp_hal::gpio::{Input, InputConfig, Pull};
    let vbus = Input::new(vbus_pin, InputConfig::default().with_pull(Pull::Down));
    run(adc1, battery_pin, Some(vbus)).await;
}
