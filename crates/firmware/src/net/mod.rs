//! Warstwa sieci: WiFi STA + stos embassy-net (Etap 4), mDNS (Etap 5), TCP (Etap 7).

pub mod mdns;
pub mod wifi;
pub mod wit;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::watch::Watch;
use longfred_proto::events::ServerEvent;
use longfred_proto::protocol::Cmd;

/// Status połączenia sieciowego publikowany do UI/logów.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetStatus {
    Disconnected,
    Connecting,
    WifiConnected,
    /// Stos ma adres IP (DHCP gotowe).
    Ready,
}

/// Jedno źródło prawdy o statusie sieci. 2 odbiorców: UI + mDNS task.
pub static STATE: Watch<CriticalSectionRawMutex, NetStatus, 2> =
    Watch::new_with(NetStatus::Disconnected);

/// Wybrany serwer WiThrottle (adres + port) do użycia przez klienta TCP (Etap 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WitEndpoint {
    pub ip: [u8; 4],
    pub port: u16,
}

/// Publikacja wybranego serwera. 2 odbiorców: UI + klient TCP (Etap 7).
pub static WIT_SERVER: Watch<CriticalSectionRawMutex, Option<WitEndpoint>, 2> =
    Watch::new_with(None);

/// Status połączenia z serwerem WiThrottle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WitConnState {
    Disconnected,
    Connecting,
    Connected,
}

/// Status klienta WiThrottle. 2 odbiorców: UI + (rezerwa).
pub static WIT_CONN: Watch<CriticalSectionRawMutex, WitConnState, 2> =
    Watch::new_with(WitConnState::Disconnected);

pub const WIT_EVENTS_DEPTH: usize = 16;
pub const WIT_COMMANDS_DEPTH: usize = 16;

/// Zdarzenia z serwera (klient → domena, Etap 8).
pub static WIT_EVENTS: Channel<CriticalSectionRawMutex, ServerEvent, WIT_EVENTS_DEPTH> =
    Channel::new();

/// Komendy do serwera (domena → klient, Etap 8).
pub static WIT_COMMANDS: Channel<CriticalSectionRawMutex, Cmd, WIT_COMMANDS_DEPTH> =
    Channel::new();
