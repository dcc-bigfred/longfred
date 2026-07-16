//! Compile-time configuration (equivalent of WiTcontroller `config_*.h`).

pub mod board;
pub mod buttons;
pub mod keyboard;
pub mod network;
pub mod power;
pub mod sizes;

/// Default device name (used before NVS load and as factory default).
pub const DEFAULT_DEVICE_NAME: &str = "LongFred";

/// Legacy alias for splash / boot log when persist is not yet loaded.
pub const DEVICE_NAME: &str = DEFAULT_DEVICE_NAME;
