//! Boot coordinator: splash → language wizard → wifi (replaces domain BootWait).

use crate::intent::AppEvent;
use crate::nav::ScreenId;

/// Boot phase owned by the firmware loop, driving the router via [`AppEvent`] / navigation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootPhase {
    Splash,
    Language,
    WifiConnect,
    WifiFailed,
    ServerConnect,
    Done,
}

impl BootPhase {
    pub fn start_screen(self) -> ScreenId {
        match self {
            Self::Splash => ScreenId::Splash,
            Self::Language => ScreenId::Language,
            Self::WifiConnect => ScreenId::Connecting,
            Self::WifiFailed => ScreenId::WifiFailed,
            Self::ServerConnect => ScreenId::ServerList,
            Self::Done => ScreenId::Throttle,
        }
    }

    pub fn on_app_event(self, e: AppEvent) -> Self {
        match (self, e) {
            (Self::WifiConnect, AppEvent::WifiReady) => Self::ServerConnect,
            (Self::WifiConnect, AppEvent::WifiFailed) => Self::WifiFailed,
            (Self::WifiFailed, AppEvent::WifiReady) => Self::ServerConnect,
            (Self::ServerConnect, AppEvent::ServerConnected) => Self::Done,
            (Self::Language, AppEvent::PersistLoaded) => Self::WifiConnect,
            (_, AppEvent::ScanDone) => self,
            _ => self,
        }
    }
}
