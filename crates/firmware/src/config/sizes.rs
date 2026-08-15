//! Collection capacities (heapless) — single source of truth for sizes.

pub use longfred_proto::model::{MAX_FUNCTIONS, MAX_THROTTLES};

pub const MAX_FOUND_SSIDS: usize = 60;
pub const MAX_FOUND_SERVERS: usize = 5;
pub const MAX_FOUND_WIT_SERVERS: usize = MAX_FOUND_SERVERS;
pub const MAX_ROSTER: usize = 70;
pub const MAX_TURNOUT_LIST: usize = 60;
pub const MAX_ROUTE_LIST: usize = 60;

pub const MAX_SSID_LEN: usize = 32;
pub const MAX_PASSWORD_LEN: usize = 64;

/// embassy-net socket count (DHCP + DNS + mDNS UDP + session TCP/UDP + HTTP OTA).
pub const NET_SOCKETS: usize = 6;

/// Soft-AP programming stack: HTTP TCP + DHCP UDP + spare.
pub const PROG_NET_SOCKETS: usize = 4;

/// Inactive OTA app slot size from `partitions.csv` (`ota_0` / `ota_1`).
pub const OTA_SLOT_BYTES: u32 = 0x3C_0000;
