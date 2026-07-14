//! Konfiguracja compile-time (odpowiednik `config_*.h` z WiTcontroller).

pub mod board;
pub mod buttons;
pub mod keypad;
pub mod network;
pub mod sizes;

/// Nazwa urządzenia zgłaszana serwerowi WiThrottle (handshake `N{name}`).
pub const DEVICE_NAME: &str = "LongFred";

/// Id urządzenia dla handshake `HU{id}` (unikalny wśród klientów).
pub const DEVICE_ID: &str = "8001";
