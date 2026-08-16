//! Shared draft state that outlives a single screen object.

use longfred_proto::command::Protocol;
use longfred_proto::persist::{DeviceIdentity, StaticIpConfig};

/// Battery icon mode on the throttle HUD.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BatteryMode {
    None,
    Icon,
    IconPercent,
}

/// Drafts and flags shared across screens (Wi-Fi wizard, device, net config).
#[derive(Clone, Debug)]
pub struct UiSession {
    pub selected_ssid: heapless::String<32>,
    pub selected_from_scan: bool,
    pub selected_ssid_idx: usize,
    pub pending_password_save: bool,
    pub password: heapless::String<64>,
    pub net_cfg: StaticIpConfig,
    pub manual_protocol: Protocol,
    pub device: DeviceIdentity,
    pub battery_mode: BatteryMode,
    pub hash_functions: bool,
    pub splash_done: bool,
    pub boot_language: bool,
    pub server_entry_from_list: bool,
    pub ip_field: u8,
    pub addr: heapless::String<8>,
    pub server_digits: heapless::String<17>,
}

impl UiSession {
    pub fn new() -> Self {
        Self {
            selected_ssid: heapless::String::new(),
            selected_from_scan: false,
            selected_ssid_idx: 0,
            pending_password_save: false,
            password: heapless::String::new(),
            net_cfg: StaticIpConfig::default(),
            manual_protocol: Protocol::WiThrottle,
            device: DeviceIdentity::empty(),
            battery_mode: BatteryMode::Icon,
            hash_functions: false,
            splash_done: false,
            boot_language: false,
            server_entry_from_list: false,
            ip_field: 0,
            addr: heapless::String::new(),
            server_digits: heapless::String::new(),
        }
    }

    pub fn cycle_battery_mode(&mut self) {
        self.battery_mode = match self.battery_mode {
            BatteryMode::None => BatteryMode::Icon,
            BatteryMode::Icon => BatteryMode::IconPercent,
            BatteryMode::IconPercent => BatteryMode::None,
        };
    }
}

impl Default for UiSession {
    fn default() -> Self {
        Self::new()
    }
}
