#![cfg_attr(not(test), no_std)]
//! LongFred WiThrottle protocol: wire parser + command builder (pure, host-testable).
//!
//! Public item docs are filled incrementally; CI clippy allows `missing_docs` for the
//! same reason. Prefer documenting new public API when adding it.
#![allow(missing_docs)]

pub mod action;
pub mod adapter;
pub mod caps;
pub mod catalog;
pub mod command;
pub mod events;
pub mod image;
pub mod input_map;
pub mod mdns;
pub mod menu;
pub mod model;
pub mod net_status;
pub mod parser;
pub mod persist;
pub mod protocol;
pub mod provisioning;
pub mod wt;
pub mod z21;

pub use action::Action;
pub use caps::{LocoSource, LocoSourceMask, Probe, ProtocolCaps, Transport};
pub use catalog::{
    AddressCatalog, Catalog, LocoCatalog, LocoRef, ServerCatalog, StaticCatalog, neighbour_index,
    resolve_effective,
};
pub use command::{ClientCommand, LocoId, Protocol};
pub use events::ServerEvent;
pub use model::{Direction, TrackPower};
