//! Warstwa domenowa: akcje (Etap 1) oraz model/stan sterowania (Etap 8).

pub mod actions;
pub mod model;
pub mod state;
pub mod task;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Watch;

use crate::domain::model::DomainSnapshot;

/// Snapshot stanu domeny dla UI. 2 odbiorców: UI + rezerwa.
pub static DOMAIN_STATE: Watch<CriticalSectionRawMutex, DomainSnapshot, 2> = Watch::new_with(
    DomainSnapshot {
        current: 0,
        speed: 0,
        forward: true,
        consist_len: 0,
        power_on: false,
        has_loco: false,
        acquiring: false,
        addr: heapless::String::new(),
    },
);
