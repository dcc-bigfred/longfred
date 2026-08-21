//! Gray-code quadrature decoder with detent accumulation.
//!
//! Mechanical KY-040 / EC11 bounce reverses a single Gray step; the lookup
//! table cancels that (`+1` then `-1`) and a detent is emitted only after
//! [`DETENT_STEPS`] valid steps in one direction.

/// Gray-code steps that make one mechanical detent on KY-040 / EC11.
pub const DETENT_STEPS: i8 = 4;

/// AiEsp32RotaryEncoder / PJRC table. Index is `(prev << 2) | curr`.
///
/// Pin packing: bit 0 = B, bit 1 = A. A-leads (B still high when A falls)
/// is clockwise, matching the previous `cw = b.is_high()` mapping.
const TABLE: [i8; 16] = [0, -1, 1, 0, 1, 0, 0, -1, -1, 0, 0, 1, 0, 1, -1, 0];

/// Incremental Gray-code decoder. Feed pin levels; receive ±1 per detent.
#[derive(Clone, Copy, Debug)]
pub struct QuadratureDecoder {
    old_ab: u8,
    accum: i8,
}

impl QuadratureDecoder {
    /// Start from the current pin levels so the first sample is not a step.
    pub const fn new(a_high: bool, b_high: bool) -> Self {
        Self {
            old_ab: pack(a_high, b_high),
            accum: 0,
        }
    }

    /// Incorporate one A/B sample. `Some(1)` = clockwise detent, `Some(-1)` =
    /// counter-clockwise. Illegal jumps score 0 (lost detent, not a reverse).
    pub fn update(&mut self, a_high: bool, b_high: bool) -> Option<i8> {
        let curr = pack(a_high, b_high);
        self.old_ab = ((self.old_ab << 2) | curr) & 0x0F;
        self.accum += TABLE[self.old_ab as usize];
        if self.accum >= DETENT_STEPS {
            self.accum = 0;
            Some(1)
        } else if self.accum <= -DETENT_STEPS {
            self.accum = 0;
            Some(-1)
        } else {
            None
        }
    }
}

const fn pack(a_high: bool, b_high: bool) -> u8 {
    ((a_high as u8) << 1) | (b_high as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Clockwise: A falls first from idle 11.
    const CW: [(bool, bool); 4] = [(false, true), (false, false), (true, false), (true, true)];

    /// Counter-clockwise: B falls first from idle 11.
    const CCW: [(bool, bool); 4] = [(true, false), (false, false), (false, true), (true, true)];

    fn feed(dec: &mut QuadratureDecoder, seq: &[(bool, bool)]) -> heapless::Vec<i8, 8> {
        let mut out = heapless::Vec::new();
        for &(a, b) in seq {
            if let Some(d) = dec.update(a, b) {
                let _ = out.push(d);
            }
        }
        out
    }

    #[test]
    fn full_cw_cycle_emits_plus_one() {
        let mut dec = QuadratureDecoder::new(true, true);
        let ev = feed(&mut dec, &CW);
        assert_eq!(ev.as_slice(), &[1]);
    }

    #[test]
    fn full_ccw_cycle_emits_minus_one() {
        let mut dec = QuadratureDecoder::new(true, true);
        let ev = feed(&mut dec, &CCW);
        assert_eq!(ev.as_slice(), &[-1]);
    }

    #[test]
    fn a_bounce_at_idle_cancels() {
        let mut dec = QuadratureDecoder::new(true, true);
        let bounce = [(false, true), (true, true), (false, true), (true, true)];
        let ev = feed(&mut dec, &bounce);
        assert!(ev.is_empty());
        assert_eq!(dec.accum, 0);
    }

    #[test]
    fn partial_cycle_emits_nothing() {
        let mut dec = QuadratureDecoder::new(true, true);
        let ev = feed(&mut dec, &CW[..2]);
        assert!(ev.is_empty());
        assert_eq!(dec.accum, 2);
    }

    #[test]
    fn two_cw_detents() {
        let mut dec = QuadratureDecoder::new(true, true);
        let mut seq = heapless::Vec::<_, 8>::new();
        for &(a, b) in &CW {
            let _ = seq.push((a, b));
        }
        for &(a, b) in &CW {
            let _ = seq.push((a, b));
        }
        let ev = feed(&mut dec, seq.as_slice());
        assert_eq!(ev.as_slice(), &[1, 1]);
    }

    #[test]
    fn illegal_jump_does_not_reverse() {
        let mut dec = QuadratureDecoder::new(true, true);
        // 11 → 00 skips a Gray step; table scores 0.
        assert_eq!(dec.update(false, false), None);
        assert_eq!(dec.accum, 0);
    }

    #[test]
    fn first_sample_matching_init_is_idle() {
        let mut dec = QuadratureDecoder::new(true, true);
        assert_eq!(dec.update(true, true), None);
        assert_eq!(dec.accum, 0);
    }
}
