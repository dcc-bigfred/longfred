//! Function-key mapping and chord hold detection (host-testable).

/// Map a physical function key (0..=8) through optional shift layers.
///
/// - neither shift → `key`
/// - `shift1` only → `key + 9`
/// - `shift2` (with or without `shift1`) → `key + 18`
pub fn map_fn_key(key: u8, shift1: bool, shift2: bool) -> u8 {
    debug_assert!(key <= 8);
    if shift2 {
        key.saturating_add(18)
    } else if shift1 {
        key.saturating_add(9)
    } else {
        key
    }
}

/// Two-button chord hold detector.
///
/// Returns `true` from [`ChordState::update`] exactly once when both inputs
/// have been held continuously for `hold_ms`. Resets when either input is
/// released.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChordState {
    pub both_since_ms: Option<u64>,
    pub fired: bool,
}

impl ChordState {
    pub const fn new() -> Self {
        Self {
            both_since_ms: None,
            fired: false,
        }
    }

    /// Update chord tracking. Returns `true` once when both `a` and `b` have
    /// been held for at least `hold_ms` since they first became both-true.
    pub fn update(&mut self, a: bool, b: bool, now_ms: u64, hold_ms: u64) -> bool {
        if !(a && b) {
            self.both_since_ms = None;
            self.fired = false;
            return false;
        }

        let since = match self.both_since_ms {
            Some(t) => t,
            None => {
                self.both_since_ms = Some(now_ms);
                now_ms
            }
        };

        if self.fired {
            return false;
        }

        if now_ms.saturating_sub(since) >= hold_ms {
            self.fired = true;
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_fn_key_no_shift() {
        for k in 0..=8 {
            assert_eq!(map_fn_key(k, false, false), k);
        }
    }

    #[test]
    fn map_fn_key_shift1() {
        for k in 0..=8 {
            assert_eq!(map_fn_key(k, true, false), k + 9);
        }
    }

    #[test]
    fn map_fn_key_shift2() {
        for k in 0..=8 {
            assert_eq!(map_fn_key(k, false, true), k + 18);
            // shift2 wins when both are pressed
            assert_eq!(map_fn_key(k, true, true), k + 18);
        }
    }

    #[test]
    fn chord_fires_once_after_hold() {
        let mut c = ChordState::new();
        assert!(!c.update(true, true, 0, 1000));
        assert!(!c.update(true, true, 500, 1000));
        assert!(c.update(true, true, 1000, 1000));
        // already fired
        assert!(!c.update(true, true, 1500, 1000));
        assert!(!c.update(true, true, 2000, 1000));
    }

    #[test]
    fn chord_resets_on_release() {
        let mut c = ChordState::new();
        assert!(!c.update(true, true, 0, 1000));
        assert!(c.update(true, true, 1000, 1000));
        assert!(!c.update(true, false, 1100, 1000));
        assert_eq!(c.both_since_ms, None);
        assert!(!c.fired);
        // can fire again after re-hold
        assert!(!c.update(true, true, 1200, 1000));
        assert!(c.update(true, true, 2200, 1000));
    }

    #[test]
    fn chord_requires_both() {
        let mut c = ChordState::new();
        assert!(!c.update(true, false, 0, 100));
        assert!(!c.update(false, true, 50, 100));
        assert!(!c.update(false, false, 100, 100));
        assert!(!c.update(true, true, 200, 100));
        assert!(c.update(true, true, 300, 100));
    }

    #[test]
    fn chord_zero_hold_fires_immediately() {
        let mut c = ChordState::new();
        assert!(c.update(true, true, 42, 0));
        assert!(!c.update(true, true, 42, 0));
    }

    #[test]
    fn chord_partial_release_aborts() {
        let mut c = ChordState::new();
        assert!(!c.update(true, true, 0, 500));
        assert!(!c.update(true, true, 200, 500));
        // release one before hold expires
        assert!(!c.update(false, true, 300, 500));
        // re-press — timer restarts
        assert!(!c.update(true, true, 400, 500));
        assert!(!c.update(true, true, 800, 500));
        assert!(c.update(true, true, 900, 500));
    }
}
