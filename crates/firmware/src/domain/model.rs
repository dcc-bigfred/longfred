//! Typy wartościowe warstwy domenowej (throttle, roster, snapshot UI).

use longfred_proto::model::{Direction, LocoAddr, ShortText, TrackPower, MAX_FUNCTIONS, MAX_THROTTLES};

pub const MAX_LOCOS: usize = 10;
pub const MAX_SPEED: u8 = 126;
pub const SHORT_DCC_ADDRESS_LIMIT: u32 = 127;

/// Wpis rosteru z serwera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosterEntry {
    pub name: ShortText,
    pub address: i32,
    pub length: char,
}

/// Czy funkcja DCC dotyczy leada czy całego składu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFollow {
    Lead,
    All,
}

/// Stan jednego slotu MultiThrottle.
#[derive(Debug, Clone)]
pub struct ThrottleSlot {
    pub speed: u8,
    pub direction: Direction,
    /// Kierunek „facing” per lok w składzie (równolegle do `consist`).
    pub facing: heapless::Vec<Direction, MAX_LOCOS>,
    pub functions: [bool; MAX_FUNCTIONS],
    pub labels: [ShortText; MAX_FUNCTIONS],
    pub follow: [FunctionFollow; MAX_FUNCTIONS],
    pub consist: heapless::Vec<LocoAddr, MAX_LOCOS>,
    pub speed_step: u8,
}

impl ThrottleSlot {
    pub fn new(speed_step: u8) -> Self {
        let mut follow = [FunctionFollow::Lead; MAX_FUNCTIONS];
        follow[0] = FunctionFollow::All;
        Self {
            speed: 0,
            direction: Direction::Forward,
            facing: heapless::Vec::new(),
            functions: [false; MAX_FUNCTIONS],
            labels: core::array::from_fn(|_| ShortText::new()),
            follow,
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

/// Snapshot stanu dla UI (Watch) — tylko pola prymitywne / krótkie stringi.
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
