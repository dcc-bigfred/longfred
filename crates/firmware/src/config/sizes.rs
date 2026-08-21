//! Collection capacities (heapless) — single source of truth for sizes.

pub use longfred_proto::model::{
    MAX_FOUND_SERVERS, MAX_FOUND_SSIDS, MAX_FUNCTIONS, MAX_ROSTER, MAX_THROTTLES,
};

pub const MAX_FOUND_WIT_SERVERS: usize = MAX_FOUND_SERVERS;

pub const MAX_SSID_LEN: usize = 32;
pub const MAX_PASSWORD_LEN: usize = 64;

/// embassy-net sockets, including concurrent session TCP and pairing HTTP client.
pub const NET_SOCKETS: usize = 8;

/// Soft-AP programming stack: HTTP TCP + DHCP UDP + spare.
pub const PROG_NET_SOCKETS: usize = 4;

/// Inactive OTA app slot size from `partitions.csv` (`ota_0` / `ota_1`).
pub const OTA_SLOT_BYTES: u32 = 0x3C_0000;
