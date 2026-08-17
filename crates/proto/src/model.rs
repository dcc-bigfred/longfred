pub const MAX_THROTTLES: usize = 6;
pub const MAX_FUNCTIONS: usize = 32;
pub const MAX_LOCOS: usize = 10;
pub const MAX_SPEED: u8 = 126;
pub const SHORT_DCC_ADDRESS_LIMIT: u32 = 127;
pub const MAX_FOUND_SSIDS: usize = 60;
pub const MAX_FOUND_SERVERS: usize = 5;
pub const MAX_ROSTER: usize = 70;

pub const PROPERTY_SEPARATOR: &str = "<;>";
pub const ENTRY_SEPARATOR: &str = "]\\[";
pub const SEGMENT_SEPARATOR: &str = "}|{";

pub type LocoAddr = heapless::String<12>;
pub type ShortText = heapless::String<32>;
pub type LongText = heapless::String<64>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Reverse,
    Forward,
}

impl Direction {
    pub fn from_wire(c: char) -> Self {
        if c == '0' {
            Direction::Reverse
        } else {
            Direction::Forward
        }
    }

    pub fn to_wire(self) -> char {
        match self {
            Direction::Reverse => '0',
            Direction::Forward => '1',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackPower {
    Off,
    On,
    Unknown,
}

impl TrackPower {
    pub fn from_wire(c: char) -> Self {
        match c {
            '0' => TrackPower::Off,
            '1' => TrackPower::On,
            _ => TrackPower::Unknown,
        }
    }

    pub fn to_wire(self) -> &'static str {
        match self {
            TrackPower::Off => "0",
            TrackPower::On => "1",
            TrackPower::Unknown => "0",
        }
    }
}

/// Throttle slot index (0..=5) to WiThrottle throttle character (`'0'`..`'5'`).
pub fn throttle_char(index: usize) -> char {
    (b'0' + index as u8) as char
}

/// Inverse of [`throttle_char`]: u8 throttle id to wire char.
pub fn throttle_char_u8(throttle: u8) -> char {
    (b'0' + throttle) as char
}

/// Wire throttle character (`'0'`..`'5'`) to slot index.
pub fn throttle_index(c: char) -> Option<usize> {
    if c.is_ascii_digit() {
        let i = (c as u8 - b'0') as usize;
        if i < MAX_THROTTLES {
            return Some(i);
        }
    }
    None
}

/// Opposite running direction.
pub fn opposite(dir: Direction) -> Direction {
    match dir {
        Direction::Forward => Direction::Reverse,
        Direction::Reverse => Direction::Forward,
    }
}

/// Whether track power is known-on.
pub fn track_power_on(tp: TrackPower) -> bool {
    matches!(tp, TrackPower::On)
}

/// Roster entry from the command station.
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
    /// Cursor in the effective loco catalogue for HUD list-walk. Independent of slot index.
    pub list_idx: Option<usize>,
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
            list_idx: None,
        }
    }

    pub fn throttle_char(&self, index: usize) -> char {
        throttle_char(index)
    }

    pub fn has_loco(&self) -> bool {
        !self.consist.is_empty()
    }
}
