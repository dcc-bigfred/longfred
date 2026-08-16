//! Wi-Fi association failed.

use crate::context::ScreenCtx;
use crate::intent::AppEvent;
use crate::nav::{Nav, ScreenId};
use crate::screen::Screen;
use crate::view::UiView;

pub struct WifiFailedScreen;

impl Screen for WifiFailedScreen {
    fn id(&self) -> ScreenId {
        ScreenId::WifiFailed
    }

    /// Two-line failure message; `WifiReady` still advances to servers.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        g.set(0, cx.s.msg_wifi_fail_1, false);
        g.set(1, cx.s.msg_wifi_fail_2, false);
        g.set(5, cx.s.hint_wifi_fail, false);
        UiView::Grid(g)
    }

    /// Late association success skips this screen.
    fn on_app_event(&mut self, e: AppEvent, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if e == AppEvent::WifiReady {
            nav.replace(ScreenId::ServerList);
        }
    }

    /// Retry from the scan-result list.
    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.replace(ScreenId::SsidScan);
    }
}
