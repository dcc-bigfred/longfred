//! UI intents (effects) and inbound app events.

use longfred_proto::action::Action;
use longfred_proto::persist::{DeviceIdentity, Language, StaticIpConfig};

/// Side-effect requested by a screen. Firmware interprets these; the UI crate
/// does not talk to Wi-Fi, storage, or the command station itself.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Intent {
    Action(Action),
    AcquireAddr,
    AcquireRoster(usize),
    ReleaseAll,
    /// Toggle DCC function `0..=31`.
    Function(u8),
    WifiScan,
    WifiConnect,
    ServerSelect(usize),
    ServerManual,
    HeartbeatToggle,
    DropBeforeAcquireToggle,
    HashFunctionsToggle,
    Sleep,
    RequestMdns,
    SaveNetwork(StaticIpConfig),
    SaveDevice(DeviceIdentity),
    RegenerateDeviceId,
    SetLanguage(Language),
    EnterProgrammingMode,
    SetHttpOta(bool),
}

/// Events the firmware pushes into the router (boot / network lifecycle).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AppEvent {
    WifiReady,
    ScanDone,
    ServerConnected,
    WifiFailed,
}
