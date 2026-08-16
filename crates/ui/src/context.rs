//! Borrowed render / input context (no `DomainState`, no embassy).

use longfred_proto::mdns::WitServer;
use longfred_proto::model::{
    MAX_FOUND_SERVERS, MAX_FOUND_SSIDS, RosterEntry, ThrottleSlot, TrackPower,
};
use longfred_proto::net_status::{
    ConnState, NetStatus, PingStatus, ServerEndpoint, SsidInfo, StaNet, WifiLink,
};
use longfred_proto::persist::PersistRecord;

use crate::geometry::DisplayGeometry;
use crate::i18n::{HintSet, Strings};
use crate::session::UiSession;

/// Borrowed drive / roster snapshot.
pub struct DriveInfo<'a> {
    pub slots: &'a [ThrottleSlot],
    pub current: usize,
    pub roster: &'a [RosterEntry],
    pub track_power: TrackPower,
    pub persist: &'a PersistRecord,
    pub message: Option<&'a str>,
    pub speed_multiplier: u8,
    pub heartbeat_on: bool,
    pub drop_before_acquire: bool,
}

/// Borrowed network snapshot.
pub struct NetInfo<'a> {
    pub status: NetStatus,
    pub conn: ConnState,
    pub server: Option<ServerEndpoint>,
    pub scanned_ssids: &'a heapless::Vec<SsidInfo, MAX_FOUND_SSIDS>,
    pub found_servers: &'a heapless::Vec<WitServer, MAX_FOUND_SERVERS>,
    pub wifi_link: Option<WifiLink>,
    pub sta_net: Option<StaNet>,
    pub ping: PingStatus,
    pub sta_ipv4: Option<[u8; 4]>,
    pub http_ota: bool,
    pub http_ota_busy: bool,
}

/// ADC / charge sample for the throttle icon and Diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatteryInfo {
    pub percent: u8,
    pub millivolts: u16,
    pub raw: u16,
}

/// Firmware-compiled SSIDs shown in the picker. Extra `NETWORKS` entries are dropped
/// (debug builds assert).
pub const MAX_COMPILED_NETWORKS: usize = 16;

/// Build-time / board facts injected by firmware (no `cfg` in screens).
#[derive(Clone, Copy, Debug)]
pub struct CompiledNetwork {
    pub ssid: &'static str,
    pub password: &'static str,
}

pub struct UiEnv {
    pub geometry: DisplayGeometry,
    pub has_keypad: bool,
    pub hint_set: HintSet,
    pub app_name: &'static str,
    pub fw_version: &'static str,
    pub fn_to_dcc: [u8; 11],
    pub hash_shows_functions: bool,
    pub compiled_networks: &'static [CompiledNetwork],
    pub default_wit_ip: [u8; 4],
    pub default_wit_port: u16,
    pub default_z21_ip: [u8; 4],
    pub default_z21_port: u16,
    pub default_prefix_len: u8,
    pub board_id: &'static str,
    pub board_mcu: &'static str,
    pub battery_factor: f32,
}

impl UiEnv {
    #[must_use]
    pub fn list_slots(&self) -> &'static [usize] {
        crate::view::list_slots_for(self.geometry.height)
    }
}

/// Per-iteration context passed into screen methods.
pub struct ScreenCtx<'a> {
    pub drive: DriveInfo<'a>,
    pub net: NetInfo<'a>,
    pub env: &'a UiEnv,
    pub s: &'a Strings,
    pub now_ms: u64,
    pub battery: Option<BatteryInfo>,
    pub session: &'a mut UiSession,
}
