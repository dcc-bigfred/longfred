//! Network layer: WiFi STA + embassy-net stack, mDNS, protocol session.

pub mod mdns;
#[cfg(not(feature = "sim"))]
pub mod provisioning;
pub mod session;
pub mod wifi;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_sync::watch::Watch;
use heapless::String;
use longfred_proto::command::{ClientCommand, Protocol};
use longfred_proto::events::ServerEvent;
use longfred_proto::mdns::WitServer;
use longfred_proto::persist::{DeviceIdentity, StaticIpConfig};

use crate::config::sizes;

/// Network connection status published to UI / logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetStatus {
    Disconnected,
    Connecting,
    WifiConnected,
    /// Stack has an IPv4 address (DHCP complete).
    Ready,
}

/// Single source of truth for network status. Two subscribers: UI + mDNS task.
pub static STATE: Watch<CriticalSectionRawMutex, NetStatus, 2> =
    Watch::new_with(NetStatus::Disconnected);

/// Selected command-station endpoint (address + port + protocol).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerEndpoint {
    pub ip: [u8; 4],
    pub port: u16,
    pub protocol: Protocol,
}

/// Published selected server. Two subscribers: session client + domain task.
pub static SERVER: Watch<CriticalSectionRawMutex, Option<ServerEndpoint>, 2> =
    Watch::new_with(None);

/// Command-station session connection status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnState {
    Disconnected,
    Connecting,
    Connected,
}

pub static CONN: Watch<CriticalSectionRawMutex, ConnState, 2> =
    Watch::new_with(ConnState::Disconnected);

pub const PROTO_EVENTS_DEPTH: usize = 16;
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

/// WiFi scan results (WiFi task → domain).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsidInfo {
    pub ssid: String<32>,
    pub rssi: i8,
    pub open: bool,
}

pub static WIFI_SCAN: Signal<CriticalSectionRawMutex, heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>> =
    Signal::new();

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

// Legacy type aliases for gradual migration.
pub type WitEndpoint = ServerEndpoint;
pub type WitConnState = ConnState;
pub const WIT_EVENTS_DEPTH: usize = PROTO_EVENTS_DEPTH;
pub const WIT_COMMANDS_DEPTH: usize = PROTO_COMMANDS_DEPTH;
