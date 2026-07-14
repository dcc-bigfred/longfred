#![cfg_attr(not(test), no_std)]
//! LongFred WiThrottle protocol: wire parser + command builder (pure, host-testable).

pub mod events;
pub mod mdns;
pub mod model;
pub mod parser;
pub mod protocol;

pub use events::ServerEvent;
pub use model::{Direction, RouteState, TrackPower, TurnoutState};
