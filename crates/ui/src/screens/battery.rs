//! Battery percent chart, voltage, and estimated time-to-empty.

use core::fmt::Write as _;

use crate::chart::{ChartData, ChartScale, build_chart};
use crate::context::ScreenCtx;
use crate::nav::{Nav, PageDir, ScreenId};
use crate::screen::Screen;
use crate::view::{BATTERY_SAMPLE_INTERVAL_S, CHART_HISTORY_LEN, Line, UiView, push_oled};

/// Battery chart screen (Extras → Battery).
pub struct BatteryScreen;

impl BatteryScreen {
    /// Stateless viewer; history lives in [`ScreenCtx::battery_history`].
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for BatteryScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for BatteryScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Battery
    }

    /// Pixel chart of percent history plus ETA and voltage captions.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut samples = heapless::Vec::<i16, CHART_HISTORY_LEN>::new();
        for &pct in cx.battery_history {
            if samples.is_full() {
                break;
            }
            let _ = samples.push(i16::from(pct));
        }
        let charging = cx.battery.is_some_and(|b| b.charging);
        let eta = eta_line(cx.battery_history, charging, cx.s.diag_na);
        let volts = volts_line(cx.battery.map(|b| b.millivolts), cx.s.diag_na);
        let extra = [eta, volts];
        UiView::Chart(build_chart(ChartData {
            title: cx.s.extras_battery,
            samples: samples.as_slice(),
            y_min: 0,
            y_max: 100,
            threshold: None,
            unit: "%",
            percentiles: false,
            extra_lines: extra.as_slice(),
            scale: ChartScale::ExpandMax,
        }))
    }

    /// Left / previous page leaves the chart.
    fn on_page(&mut self, d: PageDir, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if d == PageDir::Prev {
            nav.back();
        }
    }
}

fn eta_line(samples: &[u8], charging: bool, na: &str) -> Line {
    let mut line = Line::new();
    if charging {
        push_oled(&mut line, na);
        return line;
    }
    let Some(secs) = eta_seconds(samples) else {
        push_oled(&mut line, na);
        return line;
    };
    format_duration(&mut line, secs);
    line
}

fn volts_line(millivolts: Option<u16>, na: &str) -> Line {
    let mut line = Line::new();
    let Some(mv) = millivolts else {
        push_oled(&mut line, na);
        return line;
    };
    let whole = mv / 1000;
    let frac = (mv % 1000) / 10;
    let _ = write!(line, "{whole}.{frac:02} V");
    line
}

/// Linear extrapolation from first→last sample. `None` if not discharging.
fn eta_seconds(samples: &[u8]) -> Option<u32> {
    let n = u32::try_from(samples.len()).unwrap_or(0);
    if n < 2 {
        return None;
    }
    let first = u32::from(samples[0]);
    let last = u32::from(*samples.last()?);
    if last == 0 {
        return Some(0);
    }
    let dropped = first.checked_sub(last)?;
    if dropped == 0 {
        return None;
    }
    let span_s = (n - 1).saturating_mul(BATTERY_SAMPLE_INTERVAL_S);
    Some(last.saturating_mul(span_s) / dropped)
}

fn format_duration(line: &mut Line, secs: u32) {
    if secs == 0 {
        let _ = line.push_str("0m");
        return;
    }
    let mins_total = secs.div_ceil(60);
    let hours = mins_total / 60;
    let mins = mins_total % 60;
    if hours > 0 {
        let _ = write!(line, "{hours}h {mins}m");
    } else {
        let _ = write!(line, "{mins}m");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eta_needs_two_falling_samples() {
        assert_eq!(eta_seconds(&[]), None);
        assert_eq!(eta_seconds(&[80]), None);
        assert_eq!(eta_seconds(&[80, 80]), None);
        assert_eq!(eta_seconds(&[70, 80]), None);
    }

    #[test]
    fn eta_extrapolates_linearly() {
        // 100% → 50% over 10s ⇒ remaining 50% takes 10s.
        assert_eq!(eta_seconds(&[100, 50]), Some(10));
        // 80% → 40% over 20s (3 samples) ⇒ 40% left takes 20s.
        assert_eq!(eta_seconds(&[80, 60, 40]), Some(20));
    }

    #[test]
    fn eta_empty_cell_is_zero() {
        assert_eq!(eta_seconds(&[10, 0]), Some(0));
    }

    #[test]
    fn charging_and_missing_voltage_use_na() {
        assert_eq!(eta_line(&[100, 50], true, "---").as_str(), "---");
        assert_eq!(volts_line(None, "---").as_str(), "---");
        assert_eq!(volts_line(Some(3850), "---").as_str(), "3.85 V");
        assert_eq!(volts_line(Some(4200), "---").as_str(), "4.20 V");
    }

    #[test]
    fn duration_rounds_up_to_minutes() {
        let mut l = Line::new();
        format_duration(&mut l, 10);
        assert_eq!(l.as_str(), "1m");
        l.clear();
        format_duration(&mut l, 3600);
        assert_eq!(l.as_str(), "1h 0m");
        l.clear();
        format_duration(&mut l, 0);
        assert_eq!(l.as_str(), "0m");
    }
}
