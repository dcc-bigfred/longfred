//! Borrowed render / input context (no `DomainState`, no embassy).

use longfred_proto::LocoSource;
use longfred_proto::model::{
    MAX_FOUND_SERVERS, MAX_FOUND_SSIDS, RosterEntry, ThrottleSlot, TrackPower,
};
use longfred_proto::network::WitServer;
use longfred_proto::network::{
    ConnState, NetStatus, PingStatus, ServerEndpoint, SsidInfo, StaNet, WifiLink,
};
use longfred_proto::persist::PersistRecord;

use crate::geometry::DisplayGeometry;
use crate::i18n::{HintSet, Strings};
use crate::session::UiSession;

/// Borrowed drive / roster snapshot.
pub struct DriveInfo<'a> {
    /// Throttle slots (index [`Self::current`] is the HUD slot).
    pub slots: &'a [ThrottleSlot],
    /// Active throttle slot index.
    pub current: usize,
    /// WIT / static roster slice; which one the UI reads is [`Self::effective_loco_source`].
    pub roster: &'a [RosterEntry],
    /// Session-resolved catalogue source (ARCHITECTURE.md §7). Not written back to NVS.
    pub effective_loco_source: LocoSource,
    /// Track power as last reported by the station.
    pub track_power: TrackPower,
    /// Persisted settings.
    pub persist: &'a PersistRecord,
    /// Optional status line (acquire error, …).
    pub message: Option<&'a str>,
    /// Speed multiplier (`1`, `2`, `4`, …).
    pub speed_multiplier: u8,
    /// Active throttle slot count (`1..=MAX_THROTTLES`).
    pub max_throttles: usize,
    /// WiThrottle dead-man switch enabled (`*+` sent to station).
    pub dead_man_switch_on: bool,
}

/// Borrowed network snapshot.
pub struct NetInfo<'a> {
    /// High-level Wi-Fi / server status.
    pub status: NetStatus,
    /// Connection state machine.
    pub conn: ConnState,
    /// Connected command station, if any.
    pub server: Option<ServerEndpoint>,
    /// Last scan results.
    pub scanned_ssids: &'a heapless::Vec<SsidInfo, MAX_FOUND_SSIDS>,
    /// Last mDNS results.
    pub found_servers: &'a heapless::Vec<WitServer, MAX_FOUND_SERVERS>,
    /// STA link stats.
    pub wifi_link: Option<WifiLink>,
    /// STA IPv4 / gateway / DNS from DHCP.
    pub sta_net: Option<StaNet>,
    /// Ping-to-station status.
    pub ping: PingStatus,
    /// STA IPv4 address.
    pub sta_ipv4: Option<[u8; 4]>,
    /// HTTP OTA enabled.
    pub http_ota: bool,
    /// HTTP OTA transfer in progress.
    pub http_ota_busy: bool,
}

/// ADC / charge sample for the throttle icon and Diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatteryInfo {
    /// Estimated charge `0..=100`.
    pub percent: u8,
    /// Pack millivolts (`pin_mv * factor`).
    pub millivolts: u16,
    /// Calibrated millivolts at the ADC pin.
    pub pin_mv: u16,
    /// Lowest single pin sample this boot.
    pub pin_mv_min: u16,
    /// Highest single pin sample this boot.
    pub pin_mv_max: u16,
    /// USB / VBUS present (plugged in / charging).
    pub charging: bool,
}

/// Firmware-compiled SSIDs shown in the picker. Extra `NETWORKS` entries are dropped
/// (debug builds assert).
pub const MAX_COMPILED_NETWORKS: usize = 16;

/// Build-time / board facts injected by firmware (no `cfg` in screens).
#[derive(Clone, Copy, Debug)]
pub struct CompiledNetwork {
    /// Network name.
    pub ssid: &'static str,
    /// PSK (may be empty for open networks).
    pub password: &'static str,
}

/// Board / firmware constants that screens must not `cfg` on.
pub struct UiEnv {
    /// OLED size and grid.
    pub geometry: DisplayGeometry,
    /// Hardware has a numeric keypad.
    pub has_keypad: bool,
    /// Joystick vs keypad hint strings.
    pub hint_set: HintSet,
    /// Product name on the splash.
    pub app_name: &'static str,
    /// Firmware version string.
    pub fw_version: &'static str,
    /// Map of `LongFred` Fn keys to DCC function numbers.
    pub fn_to_dcc: [u8; 11],
    /// `#` opens the function list on this variant.
    pub hash_shows_functions: bool,
    /// SSIDs compiled into firmware.
    pub compiled_networks: &'static [CompiledNetwork],
    /// Default WIT IP for manual entry.
    pub default_wit_ip: [u8; 4],
    /// Default WIT port.
    pub default_wit_port: u16,
    /// Default Z21 IP for manual entry.
    pub default_z21_ip: [u8; 4],
    /// Default Z21 port.
    pub default_z21_port: u16,
    /// Default IPv4 prefix when auto-filling from a static IP.
    pub default_prefix_len: u8,
    /// Board identifier (Diagnostics).
    pub board_id: &'static str,
    /// MCU identifier (Diagnostics).
    pub board_mcu: &'static str,
    /// Divider ratio applied to calibrated pin millivolts (Vbat / Vpin).
    pub battery_factor: f32,
}

impl UiEnv {
    /// Content-row indices for a paged list on this geometry.
    #[must_use]
    pub fn list_slots(&self, footer: bool) -> &'static [usize] {
        crate::view::list_slots_for(self.geometry.height, footer)
    }
}

/// Per-iteration context passed into screen methods.
pub struct ScreenCtx<'a> {
    /// Drive / roster snapshot.
    pub drive: DriveInfo<'a>,
    /// Network snapshot.
    pub net: NetInfo<'a>,
    /// Board / firmware constants.
    pub env: &'a UiEnv,
    /// Active language strings.
    pub s: &'a Strings,
    /// Monotonic milliseconds (keyboard idle commit).
    pub now_ms: u64,
    /// Latest battery sample.
    pub battery: Option<BatteryInfo>,
    /// Ring of battery percent samples (oldest first), one per ADC poll.
    pub battery_history: &'a [u8],
    /// Drafts that outlive a screen object.
    pub session: &'a mut UiSession,
}
