//! Sterownik SSD1306 128x64 przez async I2C + task ekranu startowego.

use embassy_time::{Duration, Timer};
use embedded_graphics::{
    mono_font::MonoTextStyleBuilder,
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle},
    text::{Baseline, Text},
};
use esp_hal::gpio::AnyPin;
use esp_hal::i2c::master::{Config, I2c, Instance};
use esp_hal::time::Rate;
use esp_hal::Async;
use ssd1306::{
    mode::BufferedGraphicsModeAsync,
    prelude::*,
    I2CDisplayInterface, Ssd1306Async,
};

use crate::config::board;
use crate::domain::{self, model::DomainSnapshot};
use crate::net::{self, NetStatus, WitConnState, WitEndpoint};
use crate::ui::{fonts, i18n};

const BLINK_PERIOD_MS: u64 = 1000;

fn push_u8(buf: &mut heapless::String<24>, n: u8) {
    if n >= 100 {
        let _ = buf.push((b'0' + n / 100) as char);
    }
    if n >= 10 {
        let _ = buf.push((b'0' + (n / 10) % 10) as char);
    }
    let _ = buf.push((b'0' + n % 10) as char);
}

fn fmt_domain_line(snap: DomainSnapshot) -> heapless::String<24> {
    let mut s = heapless::String::<24>::new();
    if snap.has_loco {
        let _ = s.push('T');
        push_u8(&mut s, snap.current + 1);
        let _ = s.push_str(" v");
        if snap.speed >= 100 {
            let _ = s.push((b'0' + snap.speed / 100) as char);
        }
        if snap.speed >= 10 {
            let _ = s.push((b'0' + (snap.speed / 10) % 10) as char);
        }
        let _ = s.push((b'0' + snap.speed % 10) as char);
        let _ = s.push(' ');
        let _ = s.push(if snap.forward { 'F' } else { 'R' });
        let _ = s.push_str(" n");
        push_u8(&mut s, snap.consist_len);
    } else if !snap.addr.is_empty() {
        let _ = s.push_str("addr:");
        let _ = s.push_str(snap.addr.as_str());
    } else {
        let _ = s.push_str(i18n::MSG_ACQUIRE_HINT);
    }
    s
}

fn fmt_endpoint(ep: WitEndpoint) -> heapless::String<24> {
    let mut s = heapless::String::<24>::new();
    let _ = s.push_str("srv ");
    push_u8(&mut s, ep.ip[0]);
    let _ = s.push('.');
    push_u8(&mut s, ep.ip[1]);
    let _ = s.push('.');
    push_u8(&mut s, ep.ip[2]);
    let _ = s.push('.');
    push_u8(&mut s, ep.ip[3]);
    s
}

/// Buduje async I2C na `I2C0` z pinów BSP.
///
/// # Safety
///
/// Wywoływać raz, z `main`; piny `OLED_SDA`/`OLED_SCL` nie mogą być użyte gdzie indziej.
pub fn build_i2c(i2c: impl Instance + 'static) -> I2c<'static, Async> {
    I2c::new(
        i2c,
        Config::default().with_frequency(Rate::from_khz(board::OLED_I2C_FREQ_KHZ)),
    )
    .unwrap()
    .with_sda(unsafe { AnyPin::steal(board::OLED_SDA) })
    .with_scl(unsafe { AnyPin::steal(board::OLED_SCL) })
    .into_async()
}

pub type Display = Ssd1306Async<
    I2CInterface<I2c<'static, Async>>,
    DisplaySize128x64,
    BufferedGraphicsModeAsync<DisplaySize128x64>,
>;

#[embassy_executor::task]
pub async fn task(i2c: I2c<'static, Async>) {
    let interface = I2CDisplayInterface::new_custom_address(i2c, board::OLED_I2C_ADDRESS);
    let mut display: Display = Ssd1306Async::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();

    if display.init().await.is_err() {
        return;
    }

    let title_style = MonoTextStyleBuilder::new()
        .font(&fonts::TITLE)
        .text_color(BinaryColor::On)
        .build();
    let text_style = MonoTextStyleBuilder::new()
        .font(&fonts::TEXT)
        .text_color(BinaryColor::On)
        .build();
    let frame = PrimitiveStyleBuilder::new()
        .stroke_color(BinaryColor::On)
        .stroke_width(1)
        .build();

    let mut blink = false;
    let mut net_rx = net::STATE.receiver();
    let mut srv_rx = net::WIT_SERVER.receiver();
    let mut wit_rx = net::WIT_CONN.receiver();
    let mut domain_rx = domain::DOMAIN_STATE.receiver();
    loop {
        display.clear_buffer();

        Rectangle::new(Point::new(0, 0), Size::new(127, 63))
            .into_styled(frame)
            .draw(&mut display)
            .ok();

        Text::with_baseline(i18n::APP_NAME, Point::new(4, 2), title_style, Baseline::Top)
            .draw(&mut display)
            .ok();

        let domain_snap = domain_rx
            .as_mut()
            .and_then(|r| r.try_get())
            .unwrap_or_default();
        let domain_line = fmt_domain_line(domain_snap);
        Text::with_baseline(domain_line.as_str(), Point::new(4, 14), text_style, Baseline::Top)
            .draw(&mut display)
            .ok();

        let status = net_rx
            .as_mut()
            .and_then(|r| r.try_get())
            .unwrap_or(NetStatus::Disconnected);
        let status_text = match status {
            NetStatus::Disconnected => i18n::MSG_WIFI_DISCONNECTED,
            NetStatus::Connecting => i18n::MSG_WIFI_CONNECTING,
            NetStatus::WifiConnected => i18n::MSG_WIFI_CONNECTED,
            NetStatus::Ready => i18n::MSG_NET_READY,
        };
        Text::with_baseline(status_text, Point::new(4, 36), text_style, Baseline::Top)
            .draw(&mut display)
            .ok();

        let wit_state = wit_rx
            .as_mut()
            .and_then(|r| r.try_get())
            .unwrap_or(WitConnState::Disconnected);
        let wit_text = match wit_state {
            WitConnState::Disconnected => i18n::MSG_WIT_DISCONNECTED,
            WitConnState::Connecting => i18n::MSG_WIT_CONNECTING,
            WitConnState::Connected => i18n::MSG_WIT_CONNECTED,
        };
        Text::with_baseline(wit_text, Point::new(4, 48), text_style, Baseline::Top)
            .draw(&mut display)
            .ok();

        if wit_state == WitConnState::Disconnected {
            if let Some(ep) = srv_rx.as_mut().and_then(|r| r.try_get()).flatten() {
                let line = fmt_endpoint(ep);
                Text::with_baseline(line.as_str(), Point::new(4, 58), text_style, Baseline::Top)
                    .draw(&mut display)
                    .ok();
            } else {
                let msg = if status == NetStatus::Ready {
                    i18n::MSG_SRV_SEARCHING
                } else {
                    i18n::MSG_SRV_NONE
                };
                Text::with_baseline(msg, Point::new(4, 58), text_style, Baseline::Top)
                    .draw(&mut display)
                    .ok();
            }
        }

        // Wskaźnik cyklu (miganie) — dowód cyklicznego flush().
        if blink {
            display.set_pixel(124, 4, true);
        }
        blink = !blink;

        display.flush().await.ok();
        Timer::after(Duration::from_millis(BLINK_PERIOD_MS)).await;
    }
}
