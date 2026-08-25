//! Reusable OLED line-chart builder: scale, percentiles, threshold, footer.

use core::fmt::Write as _;

use crate::view::{CHART_HISTORY_LEN, ChartView, LINE_LEN, Line, push_oled};

/// Default ping Y maximum (ms); raised when samples or the threshold exceed it.
pub const PING_Y_MAX_DEFAULT: i16 = 250;
/// Horizontal threshold drawn on the ping chart (ms).
pub const PING_THRESHOLD_MS: i16 = 50;
/// ICMP timeout plotted as this many milliseconds.
pub const PING_TIMEOUT_MS: i16 = 1000;
/// Default RSSI Y range (dB).
pub const RSSI_Y_MIN_DEFAULT: i16 = -100;
/// Default RSSI Y maximum (dB).
pub const RSSI_Y_MAX_DEFAULT: i16 = 0;
/// Padding applied when fitting the RSSI range to samples.
pub const RSSI_FIT_PAD: i16 = 5;

/// How [`build_chart`] derives the painted Y range from [`ChartData`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChartScale {
    /// Keep `y_min`; raise `y_max` to fit samples and threshold.
    ExpandMax,
    /// Fit samples with `pad`, then clamp to `[y_min, y_max]`.
    FitPad {
        /// Extra units below min / above max sample.
        pad: i16,
    },
}

/// Input for [`build_chart`]. Screens fill this; they do not draw pixels.
pub struct ChartData<'a> {
    /// Title row.
    pub title: &'static str,
    /// Sample series, oldest first.
    pub samples: &'a [i16],
    /// Lower bound (and ExpandMax floor).
    pub y_min: i16,
    /// Upper bound (ExpandMax default; FitPad clamp).
    pub y_max: i16,
    /// Optional horizontal threshold in sample units.
    pub threshold: Option<i16>,
    /// Unit suffix for percentile labels (`ms`, `dB`).
    pub unit: &'static str,
    /// When set, footer gets p50 / p90 / p99 lines.
    pub percentiles: bool,
    /// Extra caption lines (battery ETA / voltage).
    pub extra_lines: &'a [Line],
    /// Y-axis policy.
    pub scale: ChartScale,
}

/// Push `value` onto a fixed-capacity ring (oldest dropped when full).
pub fn push_sample<const N: usize>(hist: &mut heapless::Vec<i16, N>, value: i16) {
    if hist.is_full() {
        hist.remove(0);
    }
    let _ = hist.push(value);
}

/// Build a paint-ready [`ChartView`] from [`ChartData`].
#[must_use]
pub fn build_chart(data: ChartData<'_>) -> ChartView {
    let (y_min, y_max) = auto_scale(&data);
    let mut samples = heapless::Vec::new();
    for &v in data.samples {
        if samples.is_full() {
            break;
        }
        let _ = samples.push(v);
    }
    let mut footer = heapless::Vec::new();
    if data.percentiles {
        for line in percentile_lines(data.samples, data.unit) {
            let _ = footer.push(line);
        }
    }
    for extra in data.extra_lines {
        if footer.is_full() {
            break;
        }
        let mut line = Line::new();
        push_oled(&mut line, extra.as_str());
        let _ = footer.push(line);
    }
    ChartView {
        title: data.title,
        samples,
        y_min,
        y_max,
        threshold: data.threshold,
        footer,
    }
}

/// Nearest-rank percentile (`p` is 0..=100). `None` when `samples` is empty.
#[must_use]
pub fn percentile(samples: &[i16], p: u8) -> Option<i16> {
    let sorted = sorted_copy(samples);
    if sorted.is_empty() {
        return None;
    }
    let n = sorted.len();
    let rank = n
        .saturating_mul(usize::from(p.min(100)))
        .div_ceil(100)
        .max(1);
    sorted.get(rank - 1).copied()
}

fn auto_scale(data: &ChartData<'_>) -> (i16, i16) {
    match data.scale {
        ChartScale::ExpandMax => {
            let mut y_max = data.y_max;
            if let Some(t) = data.threshold {
                y_max = y_max.max(t);
            }
            for &s in data.samples {
                y_max = y_max.max(s);
            }
            let y_min = data.y_min;
            if y_min >= y_max {
                (y_min, y_min.saturating_add(1))
            } else {
                (y_min, y_max)
            }
        }
        ChartScale::FitPad { pad } => {
            if data.samples.is_empty() {
                return order_span(data.y_min, data.y_max);
            }
            let mut lo = data.samples[0];
            let mut hi = data.samples[0];
            for &s in data.samples {
                lo = lo.min(s);
                hi = hi.max(s);
            }
            let lo = lo.saturating_sub(pad).max(data.y_min);
            let hi = hi.saturating_add(pad).min(data.y_max);
            order_span(lo, hi)
        }
    }
}

fn order_span(lo: i16, hi: i16) -> (i16, i16) {
    if lo >= hi {
        (lo, lo.saturating_add(1))
    } else {
        (lo, hi)
    }
}

fn sorted_copy(samples: &[i16]) -> heapless::Vec<i16, CHART_HISTORY_LEN> {
    let mut out = heapless::Vec::new();
    for &v in samples {
        if out.is_full() {
            break;
        }
        let _ = out.push(v);
        let mut i = out.len() - 1;
        while i > 0 && out[i] < out[i - 1] {
            out.swap(i, i - 1);
            i -= 1;
        }
    }
    out
}

fn percentile_lines(samples: &[i16], unit: &str) -> heapless::Vec<Line, 3> {
    let mut lines = heapless::Vec::new();
    let p50 = format_pct("p50", percentile(samples, 50), unit);
    let p90 = format_pct("p90", percentile(samples, 90), unit);
    let p99 = format_pct("p99", percentile(samples, 99), unit);
    let mut packed = Line::new();
    if try_append(&mut packed, p50.as_str()) && try_append(&mut packed, p90.as_str()) {
        let _ = lines.push(packed);
        let mut rest = Line::new();
        push_oled(&mut rest, p99.as_str());
        let _ = lines.push(rest);
        return lines;
    }
    let mut l1 = Line::new();
    push_oled(&mut l1, p50.as_str());
    let _ = lines.push(l1);
    let mut l2 = Line::new();
    if try_append(&mut l2, p90.as_str()) && try_append(&mut l2, p99.as_str()) {
        let _ = lines.push(l2);
    } else {
        let mut only_p90 = Line::new();
        push_oled(&mut only_p90, p90.as_str());
        let _ = lines.push(only_p90);
        let mut l3 = Line::new();
        push_oled(&mut l3, p99.as_str());
        let _ = lines.push(l3);
    }
    lines
}

fn format_pct(label: &str, value: Option<i16>, unit: &str) -> heapless::String<16> {
    let mut s = heapless::String::new();
    let _ = s.push_str(label);
    let _ = s.push(' ');
    match value {
        Some(v) => {
            let _ = write!(s, "{v}{unit}");
        }
        None => {
            let _ = s.push_str("---");
        }
    }
    s
}

fn try_append(line: &mut Line, token: &str) -> bool {
    let add = if line.is_empty() { 0 } else { 1 };
    if line.len() + add + token.len() > LINE_LEN {
        return false;
    }
    if !line.is_empty() {
        let _ = line.push(' ');
    }
    push_oled(line, token);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ping_data<'a>(samples: &'a [i16]) -> ChartData<'a> {
        ChartData {
            title: "Ping",
            samples,
            y_min: 0,
            y_max: PING_Y_MAX_DEFAULT,
            threshold: Some(PING_THRESHOLD_MS),
            unit: "ms",
            percentiles: true,
            extra_lines: &[],
            scale: ChartScale::ExpandMax,
        }
    }

    fn rssi_data<'a>(samples: &'a [i16]) -> ChartData<'a> {
        ChartData {
            title: "RSSI",
            samples,
            y_min: RSSI_Y_MIN_DEFAULT,
            y_max: RSSI_Y_MAX_DEFAULT,
            threshold: None,
            unit: "dB",
            percentiles: true,
            extra_lines: &[],
            scale: ChartScale::FitPad { pad: RSSI_FIT_PAD },
        }
    }

    #[test]
    fn percentile_nearest_rank() {
        let s = [1, 2, 3, 4, 5];
        assert_eq!(percentile(&s, 0), Some(1));
        assert_eq!(percentile(&s, 50), Some(3));
        assert_eq!(percentile(&s, 100), Some(5));
        assert_eq!(percentile(&[20, 1000], 99), Some(1000));
        assert_eq!(percentile(&[], 50), None);
    }

    #[test]
    fn ping_scale_stays_at_default_below_250() {
        let c = build_chart(ping_data(&[10, 20, 40]));
        assert_eq!(c.y_min, 0);
        assert_eq!(c.y_max, PING_Y_MAX_DEFAULT);
        assert_eq!(c.threshold, Some(PING_THRESHOLD_MS));
        assert!(c.footer.iter().any(|l| l.as_str().contains("p50")));
    }

    #[test]
    fn ping_scale_expands_for_large_sample() {
        let c = build_chart(ping_data(&[10, 400]));
        assert_eq!(c.y_max, 400);
    }

    #[test]
    fn ping_timeout_value_expands_scale() {
        let c = build_chart(ping_data(&[20, PING_TIMEOUT_MS]));
        assert_eq!(c.y_max, PING_TIMEOUT_MS);
        assert_eq!(
            percentile(&[20, PING_TIMEOUT_MS], 99),
            Some(PING_TIMEOUT_MS)
        );
    }

    #[test]
    fn rssi_has_no_threshold_and_fits_samples() {
        let c = build_chart(rssi_data(&[-45, -40, -42]));
        assert_eq!(c.threshold, None);
        assert_eq!(c.y_min, -50);
        assert_eq!(c.y_max, -35);
    }

    #[test]
    fn rssi_empty_uses_default_range() {
        let c = build_chart(rssi_data(&[]));
        assert_eq!(c.y_min, RSSI_Y_MIN_DEFAULT);
        assert_eq!(c.y_max, RSSI_Y_MAX_DEFAULT);
        assert!(c.footer.iter().any(|l| l.as_str().contains("---")));
    }

    #[test]
    fn push_sample_rings() {
        let mut hist = heapless::Vec::<i16, 3>::new();
        push_sample(&mut hist, 1);
        push_sample(&mut hist, 2);
        push_sample(&mut hist, 3);
        push_sample(&mut hist, 4);
        assert_eq!(hist.as_slice(), &[2, 3, 4]);
    }
}
