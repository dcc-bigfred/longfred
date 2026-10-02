//! Shared network types: mDNS, session status, Soft-AP provisioning DTOs.

pub mod mdns;
pub mod net_status;
pub mod provisioning;
pub mod roam;

pub use mdns::{
    MDNS_MULTICAST_V4, MDNS_PORT, OTA_HTTP_SERVICE, OtaHost, WitServer, build_ota_announce,
    build_ptr_query, collect_ota_hosts, collect_servers, sort_bigfred_first,
};
pub use net_status::{
    ConnState, NetStatus, PingStatus, ServerEndpoint, SsidInfo, StaNet, WifiLink,
};
