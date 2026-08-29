//! RSSI-driven roaming decision engine (host-testable, no I/O).
//!
//! The engine is pure logic: it takes RSSI samples and scan results and
//! produces roam actions. The firmware [`wifi::connection`](crate) task owns the
//! `WifiController` and executes the actions. See `ARCHITECTURE.md` par. 1.8
//! for the host-testable core principle.

use crate::persist::RadioConfig;

/// Maximum number of neighbor-report candidates we can hold.
pub const MAX_NEIGHBORS: usize = 8;

/// A scanned or neighbor-report BSS candidate for roaming.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BssCandidate {
    pub bssid: [u8; 6],
    pub channel: u8,
    pub rssi: i8,
}

/// Action returned by [`RoamEngine::on_sample`].
#[derive(Clone, Debug)]
pub enum RoamAction {
    /// Below threshold but not yet debounced; or above threshold; do nothing.
    Idle,
    /// Threshold crossed for `debounce` consecutive samples: scan for a better AP.
    /// `channels` is `None` for a full scan, or a filtered list from a
    /// 802.11k neighbor report.
    Scan { channels: Option<heapless::Vec<u8, 4>> },
    /// A better AP was found (from scan results or a neighbor report):
    /// roam to this BSSID.
    RoamTo { bssid: [u8; 6], channel: u8 },
}

impl PartialEq for RoamAction {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Idle, Self::Idle) => true,
            (Self::RoamTo { bssid: a, channel: ca }, Self::RoamTo { bssid: b, channel: cb }) => {
                a == b && ca == cb
            }
            (Self::Scan { channels: a }, Self::Scan { channels: b }) => a == b,
            _ => false,
        }
    }
}

impl Eq for RoamAction {}

/// Pure roaming decision engine. No I/O, no async, no statics.
///
/// State: consecutive samples below threshold, last scan timestamp,
/// current BSSID, and a cooldown timestamp after a successful roam.
#[derive(Clone, Debug)]
pub struct RoamEngine {
    below_count: u8,
    last_scan_ms: u64,
    current_bssid: [u8; 6],
    cooldown_until_ms: u64,
}

impl Default for RoamEngine {
    fn default() -> Self {
        Self {
            below_count: 0,
            last_scan_ms: 0,
            current_bssid: [0; 6],
            cooldown_until_ms: 0,
        }
    }
}

impl RoamEngine {
    /// Create a new engine with the given current BSSID.
    #[must_use]
    pub fn new(current_bssid: [u8; 6]) -> Self {
        Self {
            current_bssid,
            ..Self::default()
        }
    }

    /// Update the current BSSID after a successful association.
    pub fn set_current_bssid(&mut self, bssid: [u8; 6]) {
        self.current_bssid = bssid;
        self.below_count = 0;
    }

    /// Feed an RSSI sample. Returns the action to take.
    ///
    /// - Above threshold: reset `below_count`, return `Idle`.
    /// - Below threshold for < `debounce` samples: increment, return `Idle`.
    /// - Below threshold for >= `debounce` samples: return `Scan` (if
    ///   `roam_scan_interval_s` has elapsed and cooldown is over).
    pub fn on_sample(
        &mut self,
        rssi: i8,
        now_ms: u64,
        cfg: &RadioConfig,
    ) -> RoamAction {
        if !cfg.roam_enabled {
            self.below_count = 0;
            return RoamAction::Idle;
        }
        if rssi >= cfg.roam_rssi_threshold {
            self.below_count = 0;
            return RoamAction::Idle;
        }
        self.below_count = self.below_count.saturating_add(1);
        if self.below_count < cfg.roam_debounce_samples {
            return RoamAction::Idle;
        }
        // Debounce satisfied. Check scan interval and cooldown.
        let min_scan_gap_ms = (cfg.roam_scan_interval_s as u64) * 1000;
        if now_ms < self.cooldown_until_ms {
            return RoamAction::Idle;
        }
        if now_ms.saturating_sub(self.last_scan_ms) < min_scan_gap_ms {
            return RoamAction::Idle;
        }
        self.last_scan_ms = now_ms;
        RoamAction::Scan { channels: None }
    }

    /// Pick the best candidate from scan results. Returns `Some` if a roam
    /// should fire (RSSI >= current + hysteresis, same SSID, different BSSID).
    #[must_use]
    pub fn on_scan_results(
        &self,
        current_rssi: i8,
        candidates: &[BssCandidate],
        cfg: &RadioConfig,
    ) -> Option<BssCandidate> {
        if !cfg.roam_enabled {
            return None;
        }
        let min_rssi = current_rssi + cfg.roam_hysteresis_db as i8;
        candidates
            .iter()
            .filter(|c| c.bssid != self.current_bssid && c.rssi >= min_rssi)
            .max_by_key(|c| c.rssi)
            .copied()
    }

    /// Mark a successful roam: set the new BSSID and start a cooldown
    /// so we do not ping-pong immediately.
    pub fn on_roam_done(&mut self, new_bssid: [u8; 6], now_ms: u64, cfg: &RadioConfig) {
        self.set_current_bssid(new_bssid);
        // Cooldown = 2x the scan interval, so a freshly roamed client
        // holds the new AP for at least one scan interval before re-evaluating.
        let cooldown_ms = (cfg.roam_scan_interval_s as u64) * 2 * 1000;
        self.cooldown_until_ms = now_ms.saturating_add(cooldown_ms);
    }

    /// Parse an 802.11k Neighbor Report Response (IEEE 802.11-2024, Annex C).
    ///
    /// The report is a sequence of Neighbor Report Elements (EID 52).
    /// Each element: BSSID[6], BSSInfo[4], OperatingClass[1], Channel[1],
    /// PhyType[1], then optional sub-elements (we skip them).
    ///
    /// Returns up to [`MAX_NEIGHBORS`] candidates. Unknown/truncated
    /// elements are silently skipped.
    pub fn parse_neighbor_report(
        bytes: &[u8],
    ) -> heapless::Vec<BssCandidate, MAX_NEIGHBORS> {
        let mut out = heapless::Vec::new();
        let mut i = 0;
        while i + 13 <= bytes.len() && !out.is_full() {
            let bssid = [
                bytes[i],
                bytes[i + 1],
                bytes[i + 2],
                bytes[i + 3],
                bytes[i + 4],
                bytes[i + 5],
            ];
            // Layout per IEEE 802.11: BSSID[6], BSSInfo[4],
            // OperatingClass[1] (i+10), Channel[1] (i+11), PhyType[1] (i+12).
            let channel = bytes[i + 11];
            let _phy = bytes[i + 12];
            let candidate = BssCandidate {
                bssid,
                channel,
                rssi: 0, // filled in by a follow-up scan
            };
            let _ = out.push(candidate);
            i += 13;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(roam: bool) -> RadioConfig {
        let mut c = RadioConfig::default();
        c.roam_enabled = roam;
        c
    }

    #[test]
    fn below_threshold_no_roam_disabled() {
        let mut eng = RoamEngine::new([1; 6]);
        let c = cfg(false);
        assert_eq!(
            eng.on_sample(-80, 0, &c),
            RoamAction::Idle,
        );
    }

    #[test]
    fn above_threshold_idle() {
        let mut eng = RoamEngine::new([1; 6]);
        let c = cfg(true);
        assert_eq!(eng.on_sample(-50, 0, &c), RoamAction::Idle);
        assert_eq!(eng.on_sample(-72, 1000, &c), RoamAction::Idle);
    }

    #[test]
    fn debounce_before_scan() {
        let mut eng = RoamEngine::new([1; 6]);
        let c = cfg(true);
        // 3 samples needed (default debounce = 3)
        assert_eq!(eng.on_sample(-80, 0, &c), RoamAction::Idle);
        assert_eq!(eng.on_sample(-80, 250, &c), RoamAction::Idle);
        // Third sample at 500 ms: debounce satisfied, scan interval = 10 s
        // so we are within the scan gap -> still Idle.
        assert_eq!(eng.on_sample(-80, 500, &c), RoamAction::Idle);
    }

    #[test]
    fn scan_after_interval() {
        let mut eng = RoamEngine::new([1; 6]);
        let c = cfg(true);
        // Cross threshold 3 times, then wait past scan_interval.
        assert_eq!(eng.on_sample(-80, 0, &c), RoamAction::Idle);
        assert_eq!(eng.on_sample(-80, 250, &c), RoamAction::Idle);
        assert_eq!(eng.on_sample(-80, 500, &c), RoamAction::Idle);
        // 10_001 ms: past scan_interval (10 s) -> Scan fires.
        assert_eq!(
            eng.on_sample(-80, 10_001, &c),
            RoamAction::Scan { channels: None },
        );
    }

    #[test]
    fn above_threshold_resets_debounce() {
        let mut eng = RoamEngine::new([1; 6]);
        let c = cfg(true);
        assert_eq!(eng.on_sample(-80, 0, &c), RoamAction::Idle);
        assert_eq!(eng.on_sample(-80, 250, &c), RoamAction::Idle);
        // A single good sample resets the counter.
        assert_eq!(eng.on_sample(-50, 500, &c), RoamAction::Idle);
        assert_eq!(eng.on_sample(-80, 750, &c), RoamAction::Idle);
        assert_eq!(eng.on_sample(-80, 1000, &c), RoamAction::Idle);
        // Only 2 below-threshold since reset, not 3.
        assert_eq!(eng.on_sample(-80, 1250, &c), RoamAction::Idle);
    }

    #[test]
    fn hysteresis_rejects_weak_candidate() {
        let eng = RoamEngine::new([1; 6]);
        let c = cfg(true);
        // Current RSSI -75, candidate -78 (3 dB better, but hysteresis = 8).
        let candidates = [BssCandidate {
            bssid: [2; 6],
            channel: 6,
            rssi: -78,
        }];
        assert_eq!(
            eng.on_scan_results(-75, &candidates, &c),
            None,
        );
    }

    #[test]
    fn hysteresis_accepts_strong_candidate() {
        let eng = RoamEngine::new([1; 6]);
        let c = cfg(true);
        // Current RSSI -75, candidate -65 (10 dB better, hysteresis = 8).
        let candidates = [BssCandidate {
            bssid: [2; 6],
            channel: 6,
            rssi: -65,
        }];
        let pick = eng.on_scan_results(-75, &candidates, &c).unwrap();
        assert_eq!(pick.bssid, [2; 6]);
    }

    #[test]
    fn picks_strongest_of_multiple() {
        let eng = RoamEngine::new([1; 6]);
        let c = cfg(true);
        let candidates = [
            BssCandidate {
                bssid: [2; 6],
                channel: 6,
                rssi: -70,
            },
            BssCandidate {
                bssid: [3; 6],
                channel: 11,
                rssi: -60,
            },
        ];
        let pick = eng.on_scan_results(-75, &candidates, &c).unwrap();
        assert_eq!(pick.bssid, [3; 6]);
    }

    #[test]
    fn ignores_same_bssid() {
        let eng = RoamEngine::new([1; 6]);
        let c = cfg(true);
        let candidates = [BssCandidate {
            bssid: [1; 6], // same as current
            channel: 1,
            rssi: -50,
        }];
        assert_eq!(
            eng.on_scan_results(-80, &candidates, &c),
            None,
        );
    }

    #[test]
    fn cooldown_prevents_immediate_re_roam() {
        let mut eng = RoamEngine::new([1; 6]);
        let c = cfg(true);
        // 3 below-threshold samples to satisfy debounce (default = 3),
        // then scan at t=10_001 (past scan_interval = 10 s).
        assert_eq!(eng.on_sample(-80, 0, &c), RoamAction::Idle);
        assert_eq!(eng.on_sample(-80, 5_000, &c), RoamAction::Idle);
        assert_eq!(eng.on_sample(-80, 10_001, &c), RoamAction::Scan { channels: None });
        eng.on_roam_done([2; 6], 10_001, &c);
        // Immediately after: cooldown active, should not scan.
        assert_eq!(eng.on_sample(-80, 10_250, &c), RoamAction::Idle);
        // After cooldown (20 s = 2 * scan_interval) but still in scan gap
        // (last_scan was at 10_001, gap = 20_001 - 10_001 = 10 s).
        assert_eq!(eng.on_sample(-80, 20_001, &c), RoamAction::Idle);
        // After scan gap (30_001 ms): scan gap = 30_001 - 10_001 = 20 s >= 10 s,
        // and cooldown (20_001) is over. Scan fires.
        assert_eq!(
            eng.on_sample(-80, 30_001, &c),
            RoamAction::Scan { channels: None },
        );
    }

    #[test]
    fn parse_neighbor_report_basic() {
        // Two NREs: BSSID 02:03:04:05:06:07 on channel 6,
        // BSSID 0a:0b:0c:0d:0e:0f on channel 11.
        let report = [
            0x02, 0x03, 0x04, 0x05, 0x06, 0x07, // BSSID
            0x00, 0x00, 0x00, 0x00, // BSSInfo (ignored)
            0x06, // OperatingClass
            0x06, // Channel
            0x00, // PhyType
            0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, // BSSID
            0x00, 0x00, 0x00, 0x00, // BSSInfo
            0x0e, // OperatingClass
            0x0b, // Channel
            0x00, // PhyType
        ];
        let neighbors = RoamEngine::parse_neighbor_report(&report);
        assert_eq!(neighbors.len(), 2);
        assert_eq!(neighbors[0].bssid, [0x02, 0x03, 0x04, 0x05, 0x06, 0x07]);
        assert_eq!(neighbors[0].channel, 6);
        assert_eq!(neighbors[1].bssid, [0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f]);
        assert_eq!(neighbors[1].channel, 11);
    }

    #[test]
    fn parse_neighbor_report_truncated() {
        // A single complete NRE (13 bytes) + 5 bytes of a partial second.
        let report = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, // BSSID
            0x00, 0x00, 0x00, 0x00, // BSSInfo
            0x06, // OperatingClass
            0x01, // Channel
            0x00, // PhyType
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // partial next element
        ];
        let neighbors = RoamEngine::parse_neighbor_report(&report);
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].channel, 1);
    }

    #[test]
    fn parse_neighbor_report_empty() {
        let neighbors = RoamEngine::parse_neighbor_report(&[]);
        assert!(neighbors.is_empty());
    }
}
