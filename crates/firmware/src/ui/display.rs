//! SSD1306 OLED driver — UiView renderer (geometry from active variant).

use embassy_time::{Duration, Timer};
use embedded_graphics::{
    image::{Image, ImageRaw},
    mono_font::{MonoTextStyle, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle, Triangle},
    text::{Baseline, Text},
};
use ssd1306::{I2CDisplayInterface, Ssd1306, mode::BufferedGraphicsMode, prelude::*};

use crate::board::descriptor::{DisplayGeometry, LAYOUT_128X64};
use crate::config::board;
use crate::config::network::PAIRING_HTTP_URL;
use crate::input::i2c_bus::SharedI2cDevice;
use crate::ui::view::{GridView, LINE_LEN, ThrottleView, UiView};
use crate::ui::{UI_VIEW, fonts, splash};

const BLINK_PERIOD_MS: u64 = 200;
const GRID_LEFT_X: i32 = 0;
/// Content-row Y positions for 128×64 (header at y=0, then 6 full-width lines).
const GRID_Y_64: [i32; 6] = [10, 20, 30, 40, 50, 60];
/// Content-row Y positions for 128×32 (header at y=0, then 3 full-width lines).
const GRID_Y_32: [i32; 3] = [8, 16, 24];

#[cfg(feature = "variant-longfred-mini")]
type PanelSize = DisplaySize128x32;
#[cfg(not(feature = "variant-longfred-mini"))]
type PanelSize = DisplaySize128x64;

pub type Display =
    Ssd1306<I2CInterface<SharedI2cDevice>, PanelSize, BufferedGraphicsMode<PanelSize>>;

fn geometry() -> DisplayGeometry {
    crate::board::variants::active()
        .display
        .unwrap_or(LAYOUT_128X64)
}

fn line_text(grid: &GridView, idx: usize) -> &str {
    grid.lines.get(idx).map(|l| l.as_str()).unwrap_or("")
}

fn line_invert(grid: &GridView, idx: usize) -> bool {
    grid.invert.get(idx).copied().unwrap_or(false)
}

fn col_max_chars(x: i32) -> usize {
    let right = geometry().width as i32;
    ((right - x).max(6) / 6) as usize
}

fn draw_grid_line(
    display: &mut Display,
    x: i32,
    y: i32,
    text: &str,
    invert: bool,
    style_on: MonoTextStyle<'_, BinaryColor>,
) {
    let max = col_max_chars(x).min(LINE_LEN);
    let shown = if text.len() > max { &text[..max] } else { text };
    let w = (shown.len() as u32) * 6;
    let h = 10u32;
    if invert {
        Rectangle::new(Point::new(x, y), Size::new(w + 2, h))
            .into_styled(
                PrimitiveStyleBuilder::new()
                    .fill_color(BinaryColor::On)
                    .build(),
            )
            .draw(display)
            .ok();
        let inv = MonoTextStyleBuilder::new()
            .font(&fonts::TEXT)
            .text_color(BinaryColor::Off)
            .build();
        Text::with_baseline(shown, Point::new(x + 1, y), inv, Baseline::Top)
            .draw(display)
            .ok();
    } else {
        Text::with_baseline(shown, Point::new(x, y), style_on, Baseline::Top)
            .draw(display)
            .ok();
    }
}

fn draw_caps_arrow(display: &mut Display, uppercase: bool) {
    let style = PrimitiveStyleBuilder::new()
        .fill_color(BinaryColor::On)
        .build();
    let tri = if uppercase {
        Triangle::new(Point::new(1, 7), Point::new(7, 7), Point::new(4, 1))
    } else {
        Triangle::new(Point::new(1, 1), Point::new(7, 1), Point::new(4, 7))
    };
    tri.into_styled(style).draw(display).ok();
}

fn draw_grid(display: &mut Display, grid: &GridView, text_style: MonoTextStyle<'_, BinaryColor>) {
    let geom = geometry();
    let is_mini = geom.height <= 32;
    let grid_y: &[i32] = if is_mini { &GRID_Y_32 } else { &GRID_Y_64 };

    if grid.top_line && !is_mini {
        Rectangle::new(Point::new(0, 11), Size::new(127, 1))
            .into_styled(
                PrimitiveStyleBuilder::new()
                    .fill_color(BinaryColor::On)
                    .build(),
            )
            .draw(display)
            .ok();
    }
    if grid.foot_line && !is_mini {
        Rectangle::new(Point::new(0, 51), Size::new(127, 1))
            .into_styled(
                PrimitiveStyleBuilder::new()
                    .fill_color(BinaryColor::On)
                    .build(),
            )
            .draw(display)
            .ok();
    }

    for (row, y) in grid_y.iter().copied().enumerate() {
        let idx = row + 1;
        draw_grid_line(
            display,
            GRID_LEFT_X,
            y,
            line_text(grid, idx),
            line_invert(grid, idx),
            text_style,
        );
    }
    if !grid.lines.is_empty() {
        let header_x = if grid.caps.is_some() { 8 } else { GRID_LEFT_X };
        draw_grid_line(
            display,
            header_x,
            0,
            line_text(grid, 0),
            line_invert(grid, 0),
            text_style,
        );
    }
    if let Some(upper) = grid.caps {
        draw_caps_arrow(display, upper);
    }
}

/// Compact row of currently-ON function numbers (F0–F28).
fn draw_fn_active(
    display: &mut Display,
    functions: u32,
    y: i32,
    char_w: i32,
    font: &embedded_graphics::mono_font::MonoFont<'_>,
) {
    const X0: i32 = 4;
    const MAX_X: i32 = 124;

    let style = MonoTextStyleBuilder::new()
        .font(font)
        .text_color(BinaryColor::On)
        .build();

    let mut x = X0;
    let mut first = true;
    let mut truncated = false;

    for f in 0u8..29 {
        if (functions & (1u32 << f)) == 0 {
            continue;
        }

        let digits = if f < 10 { 1i32 } else { 2i32 };
        let gap = if first { 0 } else { char_w };
        let needed = gap + digits * char_w;

        if x + needed > MAX_X {
            truncated = true;
            break;
        }

        if !first {
            Text::with_baseline(" ", Point::new(x, y), style, Baseline::Top)
                .draw(display)
                .ok();
            x += char_w;
        }
        first = false;

        let mut label = heapless::String::<2>::new();
        if f >= 10 {
            let _ = label.push((b'0' + f / 10) as char);
        }
        let _ = label.push((b'0' + f % 10) as char);
        Text::with_baseline(label.as_str(), Point::new(x, y), style, Baseline::Top)
            .draw(display)
            .ok();
        x += digits * char_w;
    }

    if truncated && x + char_w <= MAX_X {
        Text::with_baseline("+", Point::new(x, y), style, Baseline::Top)
            .draw(display)
            .ok();
    }
}

fn draw_battery_icon(
    display: &mut Display,
    percent: u8,
    show_percent: bool,
    text_style: MonoTextStyle<'_, BinaryColor>,
) {
    let x = 114i32;
    let y = 0i32;
    Rectangle::new(Point::new(x, y), Size::new(12, 8))
        .into_styled(
            PrimitiveStyleBuilder::new()
                .stroke_color(BinaryColor::On)
                .stroke_width(1)
                .build(),
        )
        .draw(display)
        .ok();
    let thresholds = [10u8, 25, 50, 75, 90];
    for (i, th) in thresholds.iter().enumerate() {
        if percent >= *th {
            Rectangle::new(Point::new(x + 2 + i as i32, y + 6), Size::new(1, 2))
                .into_styled(
                    PrimitiveStyleBuilder::new()
                        .fill_color(BinaryColor::On)
                        .build(),
                )
                .draw(display)
                .ok();
        }
    }
    if show_percent {
        let mut s = heapless::String::<4>::new();
        if percent >= 100 {
            let _ = s.push_str("100");
        } else if percent >= 10 {
            let _ = s.push((b'0' + percent / 10) as char);
            let _ = s.push((b'0' + percent % 10) as char);
        } else {
            let _ = s.push((b'0' + percent) as char);
        }
        let _ = s.push('%');
        Text::with_baseline(s.as_str(), Point::new(96, 0), text_style, Baseline::Top)
            .draw(display)
            .ok();
    }
}

fn draw_throttle_standard(
    display: &mut Display,
    t: &ThrottleView,
    title_style: MonoTextStyle<'_, BinaryColor>,
    text_style: MonoTextStyle<'_, BinaryColor>,
) {
    let mut throttle_label = heapless::String::<4>::new();
    let _ = throttle_label.push((b'0' + t.current) as char);
    Rectangle::new(Point::new(0, 0), Size::new(14, 14))
        .into_styled(
            PrimitiveStyleBuilder::new()
                .stroke_color(BinaryColor::On)
                .stroke_width(1)
                .build(),
        )
        .draw(display)
        .ok();
    Text::with_baseline(
        throttle_label.as_str(),
        Point::new(4, 2),
        text_style,
        Baseline::Top,
    )
    .draw(display)
    .ok();

    let mut spd = heapless::String::<4>::new();
    if t.speed >= 100 {
        let _ = spd.push((b'0' + t.speed / 100) as char);
    }
    if t.speed >= 10 {
        let _ = spd.push((b'0' + (t.speed / 10) % 10) as char);
    }
    let _ = spd.push((b'0' + t.speed % 10) as char);
    Text::with_baseline(spd.as_str(), Point::new(36, 2), title_style, Baseline::Top)
        .draw(display)
        .ok();

    let dir = if t.forward { "Fwd" } else { "Rev" };
    Text::with_baseline(dir, Point::new(90, 4), text_style, Baseline::Top)
        .draw(display)
        .ok();

    if !t.heartbeat_on {
        Rectangle::new(Point::new(100, 2), Size::new(8, 8))
            .into_styled(
                PrimitiveStyleBuilder::new()
                    .stroke_color(BinaryColor::On)
                    .stroke_width(1)
                    .build(),
            )
            .draw(display)
            .ok();
        Rectangle::new(Point::new(100, 6), Size::new(8, 1))
            .into_styled(
                PrimitiveStyleBuilder::new()
                    .fill_color(BinaryColor::On)
                    .build(),
            )
            .draw(display)
            .ok();
    }

    if t.power_on {
        Rectangle::new(Point::new(112, 2), Size::new(8, 8))
            .into_styled(
                PrimitiveStyleBuilder::new()
                    .fill_color(BinaryColor::On)
                    .build(),
            )
            .draw(display)
            .ok();
    } else {
        Rectangle::new(Point::new(112, 2), Size::new(8, 8))
            .into_styled(
                PrimitiveStyleBuilder::new()
                    .stroke_color(BinaryColor::On)
                    .stroke_width(1)
                    .build(),
            )
            .draw(display)
            .ok();
    }

    if let Some(pct) = t.battery {
        draw_battery_icon(display, pct, t.battery_show_percent, text_style);
    }

    Text::with_baseline(
        t.loco.as_str(),
        Point::new(4, 18),
        text_style,
        Baseline::Top,
    )
    .draw(display)
    .ok();

    draw_fn_active(display, t.functions, 44, 6, &fonts::TEXT);

    Text::with_baseline(
        t.footer.as_str(),
        Point::new(4, 54),
        text_style,
        Baseline::Top,
    )
    .draw(display)
    .ok();
}

/// Compact throttle for 128×32: speed + dir + loco / footer / function strip.
fn draw_throttle_mini(
    display: &mut Display,
    t: &ThrottleView,
    text_style: MonoTextStyle<'_, BinaryColor>,
) {
    let speed_style = MonoTextStyleBuilder::new()
        .font(&fonts::FONT_8X13)
        .text_color(BinaryColor::On)
        .build();

    let mut spd = heapless::String::<4>::new();
    if t.speed >= 100 {
        let _ = spd.push((b'0' + t.speed / 100) as char);
    }
    if t.speed >= 10 {
        let _ = spd.push((b'0' + (t.speed / 10) % 10) as char);
    }
    let _ = spd.push((b'0' + t.speed % 10) as char);
    Text::with_baseline(spd.as_str(), Point::new(0, 0), speed_style, Baseline::Top)
        .draw(display)
        .ok();

    let dir = if t.forward { "F" } else { "R" };
    Text::with_baseline(dir, Point::new(40, 2), text_style, Baseline::Top)
        .draw(display)
        .ok();

    Text::with_baseline(
        t.loco.as_str(),
        Point::new(54, 2),
        text_style,
        Baseline::Top,
    )
    .draw(display)
    .ok();

    Text::with_baseline(
        t.footer.as_str(),
        Point::new(0, 13),
        text_style,
        Baseline::Top,
    )
    .draw(display)
    .ok();

    draw_fn_active(display, t.functions, 25, 4, &fonts::FONT_4X6);
}

fn draw_throttle(
    display: &mut Display,
    t: &ThrottleView,
    title_style: MonoTextStyle<'_, BinaryColor>,
    text_style: MonoTextStyle<'_, BinaryColor>,
) {
    let geom = geometry();
    if geom.height <= 32 {
        draw_throttle_mini(display, t, text_style);
    } else {
        draw_throttle_standard(display, t, title_style, text_style);
    }
}

fn draw_splash(display: &mut Display) {
    let geom = geometry();
    let hint_style = MonoTextStyleBuilder::new()
        .font(&fonts::FONT_4X6)
        .text_color(BinaryColor::On)
        .build();
    if geom.height <= 32 {
        let title_style = MonoTextStyleBuilder::new()
            .font(&fonts::TITLE)
            .text_color(BinaryColor::On)
            .build();
        Text::with_baseline("BigFred", Point::new(4, 0), title_style, Baseline::Top)
            .draw(display)
            .ok();
        let hint = splash::HINT;
        let w = (hint.len() as i32) * 4;
        let x = (geom.width as i32 - w).max(0);
        Text::with_baseline(hint, Point::new(x, 26), hint_style, Baseline::Top)
            .draw(display)
            .ok();
        return;
    }
    let raw = ImageRaw::<BinaryColor>::new(splash::SPLASH_RAW, splash::SPLASH_WIDTH);
    Image::new(&raw, Point::new(0, 0)).draw(display).ok();
    let hint = splash::HINT;
    let w = (hint.len() as i32) * 4;
    let x = (geom.width as i32 - w).max(0);
    Text::with_baseline(hint, Point::new(x, 58), hint_style, Baseline::Top)
        .draw(display)
        .ok();
}

/// Version-1 (21×21), ECC Low encoding of [`PAIRING_HTTP_URL`].
/// Regenerated with `qrencode -l L -t ASCII -m 0 'http://192.168.0.1/'`.
const PAIRING_QR_SIZE: i32 = 21;
const PAIRING_QR_BITS: [u8; 56] = [
    0xfe, 0x63, 0xfc, 0x15, 0x90, 0x6e, 0x92, 0xbb, 0x75, 0x85, 0xdb, 0xa3, 0xae, 0xc1, 0x79, 0x07,
    0xfa, 0xaf, 0xe0, 0x07, 0x00, 0xfb, 0xdd, 0x52, 0x2d, 0x47, 0xe1, 0xd4, 0xac, 0x43, 0x01, 0xc7,
    0x63, 0x04, 0x80, 0x62, 0xf7, 0xfb, 0x6a, 0xd0, 0x47, 0x9c, 0xba, 0xcc, 0xd5, 0xd6, 0xf5, 0x2e,
    0xa9, 0x89, 0x05, 0x36, 0x4f, 0xec, 0xf1, 0x00,
];

fn pairing_qr_dark(x: i32, y: i32) -> bool {
    if x < 0 || y < 0 || x >= PAIRING_QR_SIZE || y >= PAIRING_QR_SIZE {
        return false;
    }
    let i = (y * PAIRING_QR_SIZE + x) as usize;
    PAIRING_QR_BITS[i / 8] & (1 << (7 - (i % 8))) != 0
}

fn draw_pairing_qr(display: &mut Display, text_style: MonoTextStyle<'_, BinaryColor>) {
    let url = PAIRING_HTTP_URL;
    let geom = geometry();
    let text_h = 12i32;
    let text_y = (geom.height as i32 - text_h).max(0);
    let n = PAIRING_QR_SIZE;
    let avail = text_y.max(1);
    let scale = (avail / n).clamp(1, 3);
    let qr_px = n * scale;
    let x0 = (geom.width as i32 - qr_px) / 2;
    let y0 = ((avail - qr_px) / 2).max(0);
    for y in 0..n {
        for x in 0..n {
            if !pairing_qr_dark(x, y) {
                continue;
            }
            for dy in 0..scale {
                for dx in 0..scale {
                    let px = x0 + x * scale + dx;
                    let py = y0 + y * scale + dy;
                    if px >= 0 && py >= 0 {
                        display.set_pixel(px as u32, py as u32, true);
                    }
                }
            }
        }
    }

    let w = (url.len() as i32) * 6;
    let x = ((geom.width as i32 - w) / 2).max(0);
    Text::with_baseline(url, Point::new(x, text_y), text_style, Baseline::Top)
        .draw(display)
        .ok();
}

#[embassy_executor::task]
pub async fn task(i2c: SharedI2cDevice) {
    let geom = geometry();
    let is_mini = geom.height <= 32;

    let interface = I2CDisplayInterface::new_custom_address(i2c, board::OLED_I2C_ADDRESS);
    let mut display: Display = Ssd1306::new(interface, PanelSize {}, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();

    // Blocking I2C: async esp-hal master hard-resets in Wokwi on first xfer.
    if display.init().is_err() {
        log::error!("oled: init failed");
        return;
    }
    log::info!("oled: init ok ({}x{})", geom.width, geom.height);

    crate::ui::UI_VIEW
        .sender()
        .send(crate::ui::view::UiView::Splash);

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
    let mut ui_rx = UI_VIEW.receiver();

    loop {
        display.clear_buffer();

        let view = ui_rx.as_mut().and_then(|r| r.try_get()).unwrap_or_default();
        let is_splash = matches!(view, UiView::Splash);
        let is_pairing_qr = matches!(view, UiView::PairingQr);

        if !is_mini && !is_splash && !is_pairing_qr {
            Rectangle::new(Point::new(0, 0), Size::new(127, 63))
                .into_styled(frame)
                .draw(&mut display)
                .ok();
        }

        match &view {
            UiView::Grid(g) => draw_grid(&mut display, g, text_style),
            UiView::Throttle(t) => draw_throttle(&mut display, t, title_style, text_style),
            UiView::Splash => draw_splash(&mut display),
            UiView::PairingQr => draw_pairing_qr(&mut display, text_style),
        }

        if blink {
            let blink_y = if is_mini { 2 } else { 4 };
            display.set_pixel(124, blink_y, true);
        }
        blink = !blink;

        display.flush().ok();
        Timer::after(Duration::from_millis(BLINK_PERIOD_MS)).await;
    }
}
