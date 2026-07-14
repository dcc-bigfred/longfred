pub const MAX_THROTTLES: usize = 6;
pub const MAX_FUNCTIONS: usize = 32;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnoutState {
    Closed,
    Thrown,
    Unknown,
    Inconsistent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteState {
    Active,
    Inactive,
    Inconsistent,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnoutAction {
    Throw,
    Close,
    Toggle,
}

impl TurnoutAction {
    pub fn to_wire(self) -> char {
        match self {
            TurnoutAction::Close => 'C',
            TurnoutAction::Throw => 'T',
            TurnoutAction::Toggle => '2',
        }
    }
}
