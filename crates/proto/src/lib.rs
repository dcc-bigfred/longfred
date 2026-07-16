#![cfg_attr(not(test), no_std)]
//! LongFred WiThrottle protocol: wire parser + command builder (pure, host-testable).

pub mod adapter;
pub mod command;
pub mod events;
pub mod mdns;
pub mod menu;
pub mod model;
pub mod parser;
pub mod persist;
pub mod protocol;
pub mod wt;
pub mod z21;

pub use command::{ClientCommand, LocoId, Protocol};
pub use events::ServerEvent;
pub use model::{Direction, RouteState, TrackPower, TurnoutState};
