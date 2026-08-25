#![no_std]
//! LongFred firmware: application library (config, domain, input, UI, network).
//! Entry point and HAL initialization are in `src/bin/main.rs`.
//!
//! Public item docs are filled incrementally; CI clippy allows `missing_docs` for the
//! same reason. Prefer documenting new public API when adding it.
#![allow(missing_docs)]

pub mod board;
pub mod config;
pub mod domain;
pub mod input;
pub mod net;
pub mod power;
pub mod storage;
pub mod ui;

/// Spawn an embassy task or software-reset. Silent spawn skips brick the handset.
#[macro_export]
macro_rules! spawn_or_reset {
    ($spawner:expr, $expr:expr, $name:expr) => {{
        match $expr {
            Ok(token) => {
                $spawner.spawn(token);
            }
            Err(_) => {
                ::log::error!("boot: {} task pool exhausted — reset", $name);
                ::esp_hal::system::software_reset();
            }
        }
    }};
}
