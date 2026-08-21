//! Busy screen while a Wi-Fi scan runs.

use crate::context::ScreenCtx;
use crate::intent::{AppEvent, Intent};
use crate::nav::{Nav, ScreenId};
use crate::screen::Screen;
use crate::view::UiView;

/// Busy screen while a Wi-Fi scan runs.
pub struct SsidScanningScreen;

impl Screen for SsidScanningScreen {
    fn id(&self) -> ScreenId {
        ScreenId::SsidScanning
    }

    /// Busy screen while a Wi-Fi scan runs.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        g.set(0, cx.s.msg_scanning_wifi, false);
        g.set(5, cx.s.hint_scanning_wifi, false);
        UiView::Grid(g)
    }

    /// Kick off a scan as soon as this screen is shown.
    fn on_enter(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.emit(Intent::WifiScan);
    }

    /// Scan finished → show results.
    fn on_app_event(&mut self, e: AppEvent, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if e == AppEvent::ScanDone {
            nav.replace(ScreenId::SsidScan);
        }
    }

    /// Cancel still opens the (possibly empty) result list.
    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.replace(ScreenId::SsidScan);
    }
}
