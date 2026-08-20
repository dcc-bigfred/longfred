//! Screen identifiers and navigation commands issued by a screen.

use crate::i18n::OVERLAY_TIMEOUT_MS;

/// Logical screen (one object; may contain several internal pages).
///
/// The router reconstructs the corresponding screen type on every navigation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScreenId {
    /// Boot splash / product name.
    Splash,
    /// Compiled-SSID picker.
    SsidList,
    /// Live Wi-Fi scan results.
    SsidScan,
    /// Scan in progress (placeholder).
    SsidScanning,
    /// Wi-Fi password editor.
    Password,
    /// mDNS command-station list.
    ServerList,
    /// Confirm a discovered server before connecting.
    ServerConfirm,
    /// WIT vs Z21 protocol pick.
    ServerProto,
    /// Manual IP:port entry.
    ServerEntry,
    /// STA / handshake wait.
    Connecting,
    /// Drive HUD.
    Throttle,
    /// Main menu.
    Menu,
    /// Settings / extras.
    Extras,
    /// Server reconnect / pair / change / disconnect.
    ServerMenu,
    /// Generic numbered option list ([`crate::session::ChoiceKind`]).
    Choice,
    /// Roster / loco picker.
    RosterList,
    /// Manual DCC address (menu row when the effective source is address-only).
    AddrEdit,
    /// Manual six-digit `BigFred` code.
    Pairing,
    /// `BigFred` pairing sequence in progress.
    PairingWait,
    /// DCC function list.
    FunctionList,
    /// Direct command list (`MarkWTech` extra keys).
    DirectCommands,
    /// DHCP vs static summary.
    IpConfig,
    /// Field-by-field IPv4 editor.
    IpEdit,
    /// Device name / id summary.
    Device,
    /// Device name editor.
    DeviceNameEdit,
    /// Device numeric-id editor.
    DeviceIdEdit,
    /// Number of active throttle slots (`1..=9`).
    SlotCountEdit,
    /// Language picker.
    Language,
    /// HTTP OTA / firmware page.
    FirmwareUpdate,
    /// STA join failed.
    WifiFailed,
    /// Five-page diagnostics (battery / version / board / RF+ping / Wi-Fi).
    Diagnostics,
}

/// List cursor step.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    /// Previous row.
    Prev,
    /// Next row.
    Next,
}

/// Page / left-right step.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PageDir {
    /// Previous page (or leave on page 0).
    Prev,
    /// Next page.
    Next,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum NavCmd {
    Go(ScreenId),
    Replace(ScreenId),
    Back,
    Root(ScreenId),
}

/// Pending full-screen overlay (orthogonal to [`NavCmd`]).
#[derive(Clone, Debug)]
pub(crate) struct OverlayRequest {
    pub text: heapless::String<64>,
    pub timeout_ms: u64,
}

/// Injected navigator: screens change route and emit intents without knowing the router.
pub struct Nav<'a> {
    cmd: &'a mut Option<NavCmd>,
    overlay: &'a mut Option<OverlayRequest>,
    intents: &'a mut heapless::Vec<crate::intent::Intent, 4>,
}

impl<'a> Nav<'a> {
    pub(crate) fn new(
        cmd: &'a mut Option<NavCmd>,
        overlay: &'a mut Option<OverlayRequest>,
        intents: &'a mut heapless::Vec<crate::intent::Intent, 4>,
    ) -> Self {
        Self {
            cmd,
            overlay,
            intents,
        }
    }

    fn set_cmd(&mut self, cmd: NavCmd) {
        debug_assert!(self.cmd.is_none(), "screen issued two nav commands");
        *self.cmd = Some(cmd);
    }

    /// Push the current screen and open `id`.
    pub fn go(&mut self, id: ScreenId) {
        self.set_cmd(NavCmd::Go(id));
    }

    /// Switch to `id` without leaving a back-stack entry.
    pub fn replace(&mut self, id: ScreenId) {
        self.set_cmd(NavCmd::Replace(id));
    }

    /// Pop the back stack (or go to throttle if empty).
    pub fn back(&mut self) {
        self.set_cmd(NavCmd::Back);
    }

    /// Clear the stack and open `id` (typical: throttle after connect).
    pub fn root(&mut self, id: ScreenId) {
        self.set_cmd(NavCmd::Root(id));
    }

    /// Show a full-screen overlay (default timeout) on top of the destination screen.
    pub fn overlay(&mut self, text: &str) {
        self.overlay_for(text, OVERLAY_TIMEOUT_MS);
    }

    /// Show a full-screen overlay that auto-dismisses after `timeout_ms`.
    pub fn overlay_for(&mut self, text: &str, timeout_ms: u64) {
        let mut s = heapless::String::<64>::new();
        for c in text.chars() {
            if s.push(c).is_err() {
                break;
            }
        }
        *self.overlay = Some(OverlayRequest {
            text: s,
            timeout_ms,
        });
    }

    /// Queue a side-effect for the firmware interpreter.
    ///
    /// Extra intents beyond the queue capacity are dropped. That is a
    /// programming defect: a handler must not emit more than four intents.
    pub fn emit(&mut self, intent: crate::intent::Intent) {
        let overflow = self.intents.push(intent).is_err();
        debug_assert!(!overflow, "intent queue overflow");
    }
}
