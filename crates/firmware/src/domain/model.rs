//! Domain value types — re-exported from proto plus a leftover snapshot type.

pub use longfred_proto::model::{
    MAX_LOCOS, MAX_SPEED, RosterEntry, SHORT_DCC_ADDRESS_LIMIT, ThrottleSlot, opposite,
    throttle_char, throttle_index, track_power_on,
};

/// UI state snapshot (Watch) — primitive fields / short strings only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainSnapshot {
    pub current: u8,
    pub speed: u8,
    pub forward: bool,
    pub consist_len: u8,
    pub power_on: bool,
    pub has_loco: bool,
    pub acquiring: bool,
    pub addr: heapless::String<5>,
}

impl Default for DomainSnapshot {
    fn default() -> Self {
        Self {
            current: 0,
            speed: 0,
            forward: true,
            consist_len: 0,
            power_on: false,
            has_loco: false,
            acquiring: false,
            addr: heapless::String::new(),
        }
    }
}
