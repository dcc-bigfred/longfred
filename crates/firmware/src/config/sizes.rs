//! Pojemności kolekcji (heapless) — jedno źródło prawdy dla rozmiarów.

pub use longfred_proto::model::{MAX_FUNCTIONS, MAX_THROTTLES};

pub const MAX_FOUND_SSIDS: usize = 60;
pub const MAX_FOUND_WIT_SERVERS: usize = 5;
pub const MAX_ROSTER: usize = 70;
pub const MAX_TURNOUT_LIST: usize = 60;
pub const MAX_ROUTE_LIST: usize = 60;

pub const MAX_SSID_LEN: usize = 32;
pub const MAX_PASSWORD_LEN: usize = 64;

/// Liczba gniazd embassy-net (DHCP + DNS + TCP z zapasem).
pub const NET_SOCKETS: usize = 4;
