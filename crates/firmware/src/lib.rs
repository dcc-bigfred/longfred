#![no_std]
//! LongFred firmware: application library (config, domain, input, UI, network).
//! Entry point and HAL initialization are in `src/bin/main.rs`.

pub mod config;
pub mod domain;
pub mod input;
pub mod net;
pub mod power;
pub mod storage;
pub mod ui;
