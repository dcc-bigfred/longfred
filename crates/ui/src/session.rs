//! Shared draft state that outlives a single screen object.

use longfred_proto::command::Protocol;
use longfred_proto::persist::{DeviceIdentity, StaticIpConfig};

/// Battery icon mode on the throttle HUD.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BatteryMode {
    /// Hide the icon.
    None,
    /// Icon only.
    Icon,
    /// Icon plus percent.
    IconPercent,
}

/// Field currently being edited on the static-IP wizard.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum NetField {
    /// DHCP (`0`) vs static (`1`).
    #[default]
    Dhcp,
    /// Client IPv4 (12 digits).
    Ip,
    /// Prefix length (`0..=32`).
    Prefix,
    /// Gateway IPv4.
    Gateway,
    /// DNS IPv4.
    Dns,
}

impl NetField {
    /// Short OLED label for this field.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Dhcp => "Mode",
            Self::Ip => "IP",
            Self::Prefix => "Mask",
            Self::Gateway => "GW",
            Self::Dns => "DNS",
        }
    }

    /// Maximum digit characters accepted by the editor.
    #[must_use]
    pub const fn max_digits(self) -> usize {
        match self {
            Self::Dhcp => 1,
            Self::Prefix => 2,
            Self::Ip | Self::Gateway | Self::Dns => 12,
        }
    }

    /// Next wizard field, or `None` after DNS (save).
    #[must_use]
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

/// Which option list [`crate::nav::ScreenId::Choice`] is showing.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ChoiceKind {
    /// Dead-man switch: leave on / turn off.
    #[default]
    DeadMan,
    /// Roster source: auto / static / address.
    RosterSource,
    /// How to pick a command station: mDNS / manual IP.
    ServerConnect,
    /// Client IPv4 mode: DHCP / static.
    IpMode,
}

/// Drafts and flags shared across screens (Wi-Fi wizard, device, net config).
///
/// Screen objects are discarded on navigation, so anything the user typed that
/// must survive `Back` belongs here.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct UiSession {
    /// SSID chosen in the Wi-Fi wizard.
    pub selected_ssid: heapless::String<32>,
    /// `true` when [`Self::selected_ssid`] came from a live scan.
    pub selected_from_scan: bool,
    /// Index in the compiled or scanned list.
    pub selected_ssid_idx: usize,
    /// Persist the password after a successful join.
    pub pending_password_save: bool,
    /// Password draft.
    pub password: heapless::String<64>,

    /// Client IPv4 draft.
    pub net_cfg: StaticIpConfig,
    /// Field currently shown on [`crate::nav::ScreenId::IpEdit`].
    pub ip_field: NetField,
    /// Protocol chosen on the manual-entry path.
    pub manual_protocol: Protocol,
    /// `true` when server entry was opened from the mDNS list (Back returns there).
    pub server_entry_from_list: bool,
    /// Index in [`crate::context::NetInfo::found_servers`] awaiting confirm.
    pub pending_server_idx: Option<usize>,
    /// Manual `aaa.bbb.ccc.ddd:port` digits.
    pub server_digits: heapless::String<17>,

    /// Device name / id draft.
    pub device: DeviceIdentity,
    /// DCC address digits for acquire.
    pub addr: heapless::String<8>,

    /// Throttle battery icon mode.
    pub battery_mode: BatteryMode,
    /// `#` opens the function list instead of a function toggle.
    pub hash_functions: bool,
    /// Splash has already been dismissed this boot.
    pub splash_done: bool,
    /// First-boot language wizard is still active.
    pub boot_language: bool,
    /// Kind for [`crate::nav::ScreenId::Choice`] (screen is rebuilt on navigate).
    pub choice: ChoiceKind,
    /// Scan opened from Server → Wi-Fi settings (Back returns there).
    pub wifi_from_settings: bool,
}

impl UiSession {
    /// Empty drafts; battery icon on; DHCP field first.
    #[must_use]
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
            choice: ChoiceKind::DeadMan,
            wifi_from_settings: false,
            server_entry_from_list: false,
            pending_server_idx: None,
            ip_field: NetField::Dhcp,
            addr: heapless::String::new(),
            server_digits: heapless::String::new(),
        }
    }

    /// Cycle none → icon → icon+percent → none.
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
