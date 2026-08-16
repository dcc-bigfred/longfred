//! Screen identifiers and navigation commands issued by a screen.

/// Logical screen (one object; may contain several internal pages).
///
/// The router reconstructs the corresponding screen type on every navigation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScreenId {
    Splash,
    SsidList,
    SsidScan,
    SsidScanning,
    Password,
    ServerList,
    ServerProto,
    ServerEntry,
    Connecting,
    Throttle,
    Menu,
    Extras,
    RosterList,
    FunctionList,
    DirectCommands,
    IpConfig,
    IpEdit,
    Device,
    DeviceNameEdit,
    DeviceIdEdit,
    Language,
    FirmwareUpdate,
    WifiFailed,
    Diagnostics,
}

/// List cursor step.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    Prev,
    Next,
}

/// Page / left-right step.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PageDir {
    Prev,
    Next,
}

pub(crate) enum NavCmd {
    Go(ScreenId),
    Replace(ScreenId),
    Back,
    Root(ScreenId),
}

/// Injected navigator: screens change route and emit intents without knowing the router.
pub struct Nav<'a> {
    cmd: &'a mut Option<NavCmd>,
    intents: &'a mut heapless::Vec<crate::intent::Intent, 4>,
}

impl<'a> Nav<'a> {
    pub(crate) fn new(
        cmd: &'a mut Option<NavCmd>,
        intents: &'a mut heapless::Vec<crate::intent::Intent, 4>,
    ) -> Self {
        Self { cmd, intents }
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

    /// Queue a side-effect for the firmware interpreter.
    ///
    /// Extra intents beyond the queue capacity are dropped. That is a
    /// programming defect: a handler must not emit more than four intents.
    pub fn emit(&mut self, intent: crate::intent::Intent) {
        let overflow = self.intents.push(intent).is_err();
        debug_assert!(!overflow, "intent queue overflow");
    }
}
