//! Network layer: WiFi STA + embassy-net stack, mDNS, protocol session.

pub mod mdns;
pub mod pairing_http;
pub mod ping;
pub mod probe;
pub mod provisioning;
pub mod session;
pub mod wifi;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_sync::watch::Watch;
use heapless::String;
use longfred_proto::command::ClientCommand;
use longfred_proto::events::ServerEvent;
use longfred_proto::network::WitServer;
use longfred_proto::persist::{DeviceIdentity, StaticIpConfig};

use crate::config::sizes;

pub use longfred_proto::network::{
    ConnState, NetStatus, PingStatus, ServerEndpoint, SsidInfo, StaNet, WifiLink,
};

/// Single source of truth for network status. Two subscribers: UI + mDNS task.
pub static STATE: Watch<CriticalSectionRawMutex, NetStatus, 2> =
    Watch::new_with(NetStatus::Disconnected);

/// Published selected server. Two subscribers: session client + domain task.
pub static SERVER: Watch<CriticalSectionRawMutex, Option<ServerEndpoint>, 2> =
    Watch::new_with(None);

pub static CONN: Watch<CriticalSectionRawMutex, ConnState, 2> =
    Watch::new_with(ConnState::Disconnected);

/// One WiThrottle roster line can decode to count + 70 entry events in one read.
pub const PROTO_EVENTS_DEPTH: usize = 80;
pub const PROTO_COMMANDS_DEPTH: usize = 16;
pub const WIFI_CTRL_DEPTH: usize = 4;

/// Events from the server (session → domain).
pub static PROTO_EVENTS: Channel<CriticalSectionRawMutex, ServerEvent, PROTO_EVENTS_DEPTH> =
    Channel::new();

/// Commands to the server (domain → session).
pub static PROTO_COMMANDS: Channel<CriticalSectionRawMutex, ClientCommand, PROTO_COMMANDS_DEPTH> =
    Channel::new();

/// WiFi control (scan / connect) from the domain task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WifiCmd {
    Scan,
    Connect {
        ssid: String<32>,
        password: String<64>,
    },
}

pub static WIFI_CTRL: Channel<CriticalSectionRawMutex, WifiCmd, WIFI_CTRL_DEPTH> = Channel::new();

pub static WIFI_SCAN: Signal<
    CriticalSectionRawMutex,
    heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
> = Signal::new();

/// mDNS-discovered command stations.
pub static FOUND_SERVERS: Signal<
    CriticalSectionRawMutex,
    heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
> = Signal::new();

/// Request another mDNS discovery round.
pub static MDNS_CTRL: Channel<CriticalSectionRawMutex, (), 2> = Channel::new();

/// Live device identity for WiThrottle handshake (domain/storage → session).
pub static DEVICE: Watch<CriticalSectionRawMutex, DeviceIdentity, 2> =
    Watch::new_with(DeviceIdentity::empty());

/// DHCP client hostname (`longred_XXXXXX`), set at boot from NVS or entropy.
pub static WIFI_HOSTNAME: Watch<CriticalSectionRawMutex, heapless::String<16>, 2> =
    Watch::new_with(heapless::String::new());

/// Live IPv4 stack configuration (domain → config_task).
pub static NET_CONFIG_CTRL: Signal<CriticalSectionRawMutex, StaticIpConfig> = Signal::new();

/// STA IPv4 once DHCP/static config is up (UI + mDNS OTA announce).
pub static STA_IPV4: Watch<CriticalSectionRawMutex, Option<[u8; 4]>, 2> = Watch::new_with(None);

pub static STA_NET: Watch<CriticalSectionRawMutex, Option<StaNet>, 2> = Watch::new_with(None);

pub static WIFI_LINK: Watch<CriticalSectionRawMutex, Option<WifiLink>, 2> = Watch::new_with(None);

pub static PING: Watch<CriticalSectionRawMutex, PingStatus, 2> = Watch::new_with(PingStatus::Idle);

/// ICMP echo runs only while Diagnostics is on screen (domain → ping task).
pub static PING_ENABLE: Watch<CriticalSectionRawMutex, bool, 2> = Watch::new_with(false);

/// User-enabled STA HTTP OTA server (menu screen).
pub static HTTP_OTA_ENABLE: Watch<CriticalSectionRawMutex, bool, 4> = Watch::new_with(false);

/// Firmware POST in progress (OLED "Updating").
pub static HTTP_OTA_BUSY: Watch<CriticalSectionRawMutex, bool, 2> = Watch::new_with(false);

pub fn http_ota_enabled() -> bool {
    HTTP_OTA_ENABLE.try_get().unwrap_or(false)
}

pub fn http_ota_busy() -> bool {
    HTTP_OTA_BUSY.try_get().unwrap_or(false)
}

pub fn sta_ipv4() -> Option<[u8; 4]> {
    STA_IPV4.try_get().flatten()
}

pub fn set_http_ota_enabled(on: bool) {
    HTTP_OTA_ENABLE.sender().send(on);
}

pub fn set_ping_enabled(on: bool) {
    if PING_ENABLE.try_get() == Some(on) {
        return;
    }
    PING_ENABLE.sender().send(on);
    if !on {
        PING.sender().send(PingStatus::Idle);
    }
}

// Legacy type aliases for gradual migration.
pub type WitEndpoint = ServerEndpoint;
pub type WitConnState = ConnState;
pub const WIT_EVENTS_DEPTH: usize = PROTO_EVENTS_DEPTH;
pub const WIT_COMMANDS_DEPTH: usize = PROTO_COMMANDS_DEPTH;
