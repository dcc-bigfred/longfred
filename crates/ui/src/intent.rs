//! UI intents (effects) and inbound app events.

use longfred_proto::action::Action;
use longfred_proto::persist::{DeviceIdentity, Language, RosterMode, StaticIpConfig};

use crate::nav::PageDir;

/// Side-effect requested by a screen. Firmware interprets these; the UI crate
/// does not talk to Wi-Fi, storage, or the command station itself.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Intent {
    /// Throttle / power / sleep action for the domain.
    Action(Action),
    /// Acquire the address in [`crate::session::UiSession::addr`].
    AcquireAddr,
    /// Acquire WIT roster entry `i`.
    AcquireRoster(usize),
    /// Walk the effective catalogue inside the current throttle slot.
    SelectLoco(PageDir),
    /// Release every acquired loco on this throttle.
    ReleaseAll,
    /// Toggle DCC function `0..=31`.
    Function(u8),
    /// Start a Wi-Fi scan.
    WifiScan,
    /// Join the SSID/password currently in the session.
    WifiConnect,
    /// Connect to discovered server `i`.
    ServerSelect(usize),
    /// Connect using [`crate::session::UiSession::server_digits`].
    ServerManual,
    /// Toggle the `WiThrottle` heartbeat.
    HeartbeatToggle,
    /// Toggle drop-before-acquire.
    DropBeforeAcquireToggle,
    /// Toggle whether `#` opens the function list.
    HashFunctionsToggle,
    /// Request device sleep.
    Sleep,
    /// Refresh the mDNS server list.
    RequestMdns,
    /// Persist client IPv4 configuration.
    SaveNetwork(StaticIpConfig),
    /// Persist device name / id.
    SaveDevice(DeviceIdentity),
    /// Assign a new random device id.
    RegenerateDeviceId,
    /// Persist UI language.
    SetLanguage(Language),
    /// Persist preferred locomotive source (`TAG_ROSTER`).
    SetRosterMode(RosterMode),
    /// Enter programming mode.
    EnterProgrammingMode,
    /// Enable or disable HTTP OTA.
    SetHttpOta(bool),
}

/// Events the firmware pushes into the router (boot / network lifecycle).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AppEvent {
    /// STA associated; continue the connect wizard.
    WifiReady,
    /// Scan results are in [`crate::context::NetInfo::scanned_ssids`].
    ScanDone,
    /// Command station handshake succeeded.
    ServerConnected,
    /// STA join failed; show the retry screen.
    WifiFailed,
}
