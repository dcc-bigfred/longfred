//! Domain value types (throttle, roster, UI snapshot).

use longfred_proto::model::{
    Direction, LocoAddr, MAX_FUNCTIONS, MAX_THROTTLES, ShortText, TrackPower,
};

pub const MAX_LOCOS: usize = 10;
pub const MAX_SPEED: u8 = 126;
pub const SHORT_DCC_ADDRESS_LIMIT: u32 = 127;

/// Roster entry from the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosterEntry {
    pub name: ShortText,
    pub address: i32,
    pub length: char,
}

/// State of one MultiThrottle slot.
#[derive(Debug, Clone)]
pub struct ThrottleSlot {
    pub speed: u8,
    pub direction: Direction,
    /// Per-loco "facing" direction in the consist (parallel to `consist`).
    pub facing: heapless::Vec<Direction, MAX_LOCOS>,
    pub functions: [bool; MAX_FUNCTIONS],
    pub labels: [ShortText; MAX_FUNCTIONS],
    pub consist: heapless::Vec<LocoAddr, MAX_LOCOS>,
    pub speed_step: u8,
}

impl ThrottleSlot {
    pub fn new(speed_step: u8) -> Self {
        Self {
            speed: 0,
            direction: Direction::Forward,
            facing: heapless::Vec::new(),
            functions: [false; MAX_FUNCTIONS],
            labels: core::array::from_fn(|_| ShortText::new()),
            consist: heapless::Vec::new(),
            speed_step,
        }
    }

    pub fn throttle_char(&self, index: usize) -> char {
        (b'0' + index as u8) as char
    }

    pub fn has_loco(&self) -> bool {
        !self.consist.is_empty()
    }
}

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

pub fn throttle_index(c: char) -> Option<usize> {
    if c.is_ascii_digit() {
        let i = (c as u8 - b'0') as usize;
        if i < MAX_THROTTLES {
            return Some(i);
        }
    }
    None
}

pub fn throttle_char(index: usize) -> char {
    (b'0' + index as u8) as char
}

pub fn opposite(dir: Direction) -> Direction {
    match dir {
        Direction::Forward => Direction::Reverse,
        Direction::Reverse => Direction::Forward,
    }
}

pub fn track_power_on(tp: TrackPower) -> bool {
    matches!(tp, TrackPower::On)
}
