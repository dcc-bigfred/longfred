//! SSD1306 OLED driver — UiView renderer (geometry from active variant).

use embassy_time::{Duration, Timer};
use embedded_graphics::{
    mono_font::{MonoTextStyle, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle},
    text::{Baseline, Text},
};
use ssd1306::{I2CDisplayInterface, Ssd1306, mode::BufferedGraphicsMode, prelude::*};

use crate::board::descriptor::{DisplayGeometry, LAYOUT_128X64};
use crate::config::board;
use crate::input::i2c_bus::SharedI2cDevice;
use crate::ui::view::{GridView, LINE_LEN, ThrottleView, UiView};
use crate::ui::{UI_VIEW, fonts};

const BLINK_PERIOD_MS: u64 = 200;
const GRID_LEFT_X: i32 = 0;
const GRID_RIGHT_X: i32 = 64;
/// Content-row Y positions for 128×64 (6 rows × 2 cols).
const GRID_Y_64: [i32; 6] = [10, 20, 30, 40, 50, 60];
/// Content-row Y positions for 128×32 (3 rows × 2 cols).
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

fn draw_grid_line(
    display: &mut Display,
    x: i32,
    y: i32,
    text: &str,
    invert: bool,
    style_on: MonoTextStyle<'_, BinaryColor>,
) {
    let w = (text.len().min(LINE_LEN) as u32) * 6;
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
        Text::with_baseline(text, Point::new(x + 1, y), inv, Baseline::Top)
            .draw(display)
            .ok();
    } else {
        Text::with_baseline(text, Point::new(x, y), style_on, Baseline::Top)
            .draw(display)
            .ok();
    }
}

fn draw_grid(display: &mut Display, grid: &GridView, text_style: MonoTextStyle<'_, BinaryColor>) {
    let geom = geometry();
    let is_mini = geom.height <= 32;
    let rows = geom.grid_lines / 2;
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

    for row in 0..rows {
        let y = grid_y.get(row).copied().unwrap_or(0);
        let left_idx = row + 1;
        if left_idx < geom.grid_lines {
            draw_grid_line(
                display,
                GRID_LEFT_X,
                y,
                line_text(grid, left_idx),
                line_invert(grid, left_idx),
                text_style,
            );
        }
        let right_idx = row + 1 + rows;
        if right_idx < geom.grid_lines {
            draw_grid_line(
                display,
                GRID_RIGHT_X,
                y,
                line_text(grid, right_idx),
                line_invert(grid, right_idx),
                text_style,
            );
        }
    }
    if !grid.lines.is_empty() {
        draw_grid_line(
            display,
            GRID_LEFT_X,
            0,
            line_text(grid, 0),
            line_invert(grid, 0),
            text_style,
        );
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

    // Splash so the panel shows something before domain publishes UiView.
    {
        let mut splash = crate::ui::view::GridView::new();
        splash.set(0, "LongFred", false);
        splash.set(1, "boot...", false);
        crate::ui::UI_VIEW
            .sender()
            .send(crate::ui::view::UiView::Grid(splash));
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
    let mut ui_rx = UI_VIEW.receiver();

    loop {
        display.clear_buffer();

        if !is_mini {
            Rectangle::new(Point::new(0, 0), Size::new(127, 63))
                .into_styled(frame)
                .draw(&mut display)
                .ok();
        }

        let view = ui_rx.as_mut().and_then(|r| r.try_get()).unwrap_or_default();

        match &view {
            UiView::Grid(g) => draw_grid(&mut display, g, text_style),
            UiView::Throttle(t) => draw_throttle(&mut display, t, title_style, text_style),
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
