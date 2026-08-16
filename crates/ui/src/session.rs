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

/// Field currently being edited on the static-IP wizard.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum NetField {
    #[default]
    Dhcp,
    Ip,
    Prefix,
    Gateway,
    Dns,
}

impl NetField {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Dhcp => "Mode",
            Self::Ip => "IP",
            Self::Prefix => "Mask",
            Self::Gateway => "GW",
            Self::Dns => "DNS",
        }
    }

    pub const fn max_digits(self) -> usize {
        match self {
            Self::Dhcp => 1,
            Self::Prefix => 2,
            Self::Ip | Self::Gateway | Self::Dns => 12,
        }
    }

    pub const fn next(self) -> Option<Self> {
        match self {
            Self::Dhcp => Some(Self::Ip),
            Self::Ip => Some(Self::Prefix),
            Self::Prefix => Some(Self::Gateway),
            Self::Gateway => Some(Self::Dns),
            Self::Dns => None,
        }
    }
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
    pub ip_field: NetField,
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
            ip_field: NetField::Dhcp,
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
