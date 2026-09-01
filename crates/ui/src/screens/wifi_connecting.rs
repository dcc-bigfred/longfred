//! Association in progress.

use crate::context::ScreenCtx;
use crate::intent::{AppEvent, Intent};
use crate::nav::{Nav, ScreenId};
use crate::screen::Screen;
use crate::view::UiView;

/// Association in progress.
pub struct ConnectingScreen;

impl ConnectingScreen {
    fn abort_to_menu(nav: &mut Nav<'_>) {
        nav.emit(Intent::AbortConnect);
        nav.root(ScreenId::Menu);
    }
}

impl Screen for ConnectingScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Connecting
    }

    /// Status line plus the SSID being joined. Does not emit `WifiConnect` itself.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        g.set(1, cx.s.msg_trying_connect, false);
        g.set(2, cx.session.selected_ssid.as_str(), false);
        super::helpers::set_list_hint(&mut g, cx, cx.s.hint_connecting);
        UiView::Grid(g)
    }

    /// `WifiReady` → servers, `WifiFailed` → error, `ServerConnected` → throttle.
    fn on_app_event(&mut self, e: AppEvent, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        match e {
            AppEvent::WifiReady => nav.replace(ScreenId::ServerList),
            AppEvent::WifiFailed => nav.replace(ScreenId::WifiFailed),
            AppEvent::ServerConnected => nav.root(ScreenId::Throttle),
            AppEvent::ScanDone
            | AppEvent::PairingRequired
            | AppEvent::PairingStarted
            | AppEvent::PairingSucceeded
            | AppEvent::PairingFailed => {}
        }
    }

    /// Skip the wizard: main menu, no network or server picker.
    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        Self::abort_to_menu(nav);
    }

    /// Root to Menu (do not push it over Connecting; firmware would yank back).
    fn on_menu_key(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        Self::abort_to_menu(nav);
    }
}
