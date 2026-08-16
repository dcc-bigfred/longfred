use crate::model::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerEvent {
    HeartbeatConfig {
        seconds: u32,
    },
    Version(ShortText),
    ServerType(ShortText),
    ServerDescription(LongText),
    Message(LongText),
    Alert(LongText),
    WebPort(u16),
    TrackPower(TrackPower),

    Speed {
        throttle: char,
        speed: u8,
    },
    DirectionLead {
        throttle: char,
        dir: Direction,
    },
    DirectionLoco {
        throttle: char,
        addr: LocoAddr,
        dir: Direction,
    },
    FunctionState {
        throttle: char,
        func: u8,
        on: bool,
    },
    RosterFunctionLabels {
        throttle: char,
        labels: [ShortText; MAX_FUNCTIONS],
    },

    RosterEntriesCount(u16),
    RosterEntry {
        index: u16,
        name: ShortText,
        address: i32,
        length: char,
    },

    AddressAdded {
        throttle: char,
        addr: LocoAddr,
        entry: LongText,
    },
    AddressRemoved {
        throttle: char,
        addr: LocoAddr,
        entry: LongText,
    },
    StealNeeded {
        throttle: char,
        addr: LocoAddr,
        entry: LongText,
    },

    Unknown(LongText),
}
