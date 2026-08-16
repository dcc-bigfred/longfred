//! Network configuration (equivalent of config_network.h).
//! NOTE: this is a sample file with placeholders. Real SSID/passwords must NOT
//! be committed to the repository (see TODO below).

pub use longfred_ui::CompiledNetwork as WifiNetwork;

// TODO: replace with real data; eventually via file/override outside VCS.
pub const NETWORKS: &[WifiNetwork] = &[];

pub const USE_WIFI_COUNTRY_CODE: bool = false;
pub const COUNTRY_CODE: &str = "01";

pub const SSID_CONNECTION_TIMEOUT_MS: u64 = 5_000;
pub const WIFI_FAIL_MSG_MS: u64 = 1_500;
pub const SPLASH_MS: u64 = 2_000;
pub const SERVER_CONNECTION_TIMEOUT_MS: u64 = 5_000;
#[allow(dead_code)]
pub const AUTO_CONNECT_TO_FIRST_DEFINED_SERVER: bool = false;
pub const AUTO_CONNECT_TO_FIRST_WITHROTTLE_SERVER: bool = false;
pub const RESTORE_ACQUIRED_LOCOS: bool = true;

// --- Command rate limiting / speed coalescing ---
/// Minimum gap between any outbound WiThrottle commands.
pub const OUTBOUND_COMMANDS_MIN_DELAY_MS: u64 = 20;
/// Speed coalesce window: only the last value within the window is sent to the server.
pub const SPEED_COALESCE_WINDOW_MS: u64 = 200;
/// Domain task tick (trailing speed flush). Must be less than `SPEED_COALESCE_WINDOW_MS`.
pub const DOMAIN_TICK_MS: u64 = 50;

// --- TCP (latency + dead-connection detection) ---
pub const TCP_NODELAY: bool = true;
pub const TCP_KEEPALIVE_S: u64 = 5;
pub const TCP_TIMEOUT_S: u64 = 8;

// --- WiThrottle reconnect backoff ---
pub const RECONNECT_MIN_MS: u64 = 500;
pub const RECONNECT_MAX_MS: u64 = 5_000;

// --- WiFi 6 (bigfred event infrastructure) ---
/// Enable 802.11ax on 2.4 GHz for OFDMA scheduling with WiFi 6 APs.
pub const WIFI_ENABLE_11AX: bool = true;
/// Disable modem power-save (latency over battery savings while connected).
pub const WIFI_FORCE_POWER_SAVE_NONE: bool = true;

pub const SEND_LEADING_CR_LF: bool = true;
pub const MDNS_WAIT_MS: u64 = 10_000;
pub const SORT_WIFI_NETWORKS: bool = false;
pub const USE_FAST_WIFI_SCAN: bool = false;
pub const BYPASS_WIFI_SCAN_ON_STARTUP: bool = false;

/// Default WiThrottle server (DCC-EX AP) when mDNS finds nothing.
pub const DEFAULT_WIT_IP: [u8; 4] = [192, 168, 4, 1];
pub const DEFAULT_WIT_PORT: u16 = 2560;

/// Default Z21 command station endpoint.
pub const DEFAULT_Z21_IP: [u8; 4] = [192, 168, 0, 111];
pub const DEFAULT_Z21_PORT: u16 = 21105;
pub const Z21_BROADCAST_FLAGS: u32 = 0x0000_0001;

/// Default subnet prefix when auto-filling static IP fields.
pub const DEFAULT_PREFIX_LEN: u8 = 24;

/// Soft-AP provisioning page (matches AP IPv4 `192.168.0.1`).
pub const PAIRING_HTTP_URL: &str = "http://192.168.0.1/";
