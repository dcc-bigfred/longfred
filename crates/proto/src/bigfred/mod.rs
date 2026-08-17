//! BigFred protocol: WiThrottle drive traffic plus handset pairing.

pub mod adapter;
pub mod pairing_http;

pub use adapter::{BigFredAdapter, PAIRING_CODE_LEN, PAIRING_SENTINEL_ADDR, PAIRING_SENTINEL_NAME};
