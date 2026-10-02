//! Network / session status types shared by firmware and the host-testable UI.

use crate::command::Protocol;

/// Network connection status published to UI / logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetStatus {
    Disconnected,
    Connecting,
    WifiConnected,
    /// Stack has an IPv4 address (DHCP complete).
    Ready,
}

/// Selected command-station endpoint (address + port + protocol).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerEndpoint {
    pub ip: [u8; 4],
    pub port: u16,
    pub protocol: Protocol,
}

/// Command-station session connection status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnState {
    Disconnected,
    Connecting,
    Connected,
}

/// One Wi-Fi scan result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsidInfo {
    pub ssid: heapless::String<32>,
    pub rssi: i8,
    pub open: bool,
    /// BSSID (MAC) of the AP. Needed for BSSID-locked roaming.
    pub bssid: [u8; 6],
    /// Operating channel. Needed for channel-filtered scan.
    pub channel: u8,
}

/// STA IPv4 config + MAC (Diagnostics).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaNet {
    pub ip: [u8; 4],
    pub prefix: u8,
    pub gateway: Option<[u8; 4]>,
    pub dns: Option<[u8; 4]>,
    pub mac: [u8; 6],
}

/// Associated AP (RSSI / BSSID / channel).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WifiLink {
    pub ssid: heapless::String<32>,
    pub rssi: i8,
    pub bssid: [u8; 6],
    pub channel: u8,
}

/// ICMP echo to the selected command station.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PingStatus {
    Idle,
    Ms(u16),
    Timeout,
}
