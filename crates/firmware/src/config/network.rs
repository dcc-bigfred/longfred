//! Konfiguracja sieci (odpowiednik config_network.h).
//! UWAGA: to plik przykładowy z placeholderami. Realne SSID/hasła NIE powinny
//! trafiać do repozytorium (patrz TODO poniżej).

/// Predefiniowana sieć WiFi z prefiksami turnoutów/route dla danego serwera.
pub struct WifiNetwork {
    pub ssid: &'static str,
    pub password: &'static str,
    pub turnout_prefix: &'static str,
    pub route_prefix: &'static str,
}

// TODO: podmienić na realne dane; docelowo przez plik/override poza VCS.
pub const NETWORKS: &[WifiNetwork] = &[WifiNetwork {
    ssid: "Network1",
    password: "password1",
    turnout_prefix: "NT",
    route_prefix: "IO:AUTO:",
}];

pub const USE_WIFI_COUNTRY_CODE: bool = false;
pub const COUNTRY_CODE: &str = "01";

pub const SSID_CONNECTION_TIMEOUT_MS: u64 = 10_000;
pub const AUTO_CONNECT_TO_FIRST_DEFINED_SERVER: bool = false;
pub const AUTO_CONNECT_TO_FIRST_WITHROTTLE_SERVER: bool = true;
pub const OUTBOUND_COMMANDS_MIN_DELAY_MS: u64 = 50;
pub const SEND_LEADING_CR_LF: bool = true;
pub const MDNS_WAIT_MS: u64 = 10_000;
pub const SORT_WIFI_NETWORKS: bool = false;
pub const USE_FAST_WIFI_SCAN: bool = false;
pub const BYPASS_WIFI_SCAN_ON_STARTUP: bool = false;

/// Domyślny serwer WiThrottle (DCC-EX AP), gdy mDNS nic nie znajdzie.
pub const DEFAULT_WIT_IP: [u8; 4] = [192, 168, 4, 1];
pub const DEFAULT_WIT_PORT: u16 = 2560;
