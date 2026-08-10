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
