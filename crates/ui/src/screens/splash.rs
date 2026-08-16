//! Boot splash.

use crate::context::ScreenCtx;
use crate::nav::{Nav, ScreenId};
use crate::screen::Screen;
use crate::view::UiView;

pub struct SplashScreen;

impl SplashScreen {
    /// Empty splash (no local state).
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for SplashScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for SplashScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Splash
    }

    /// Full-screen splash; input is ignored except select/cancel/menu.
    fn view(&self, _cx: &ScreenCtx<'_>) -> UiView {
        UiView::Splash
    }

    /// Mark splash done, then Language (first boot) or Connecting.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        cx.session.splash_done = true;
        if cx.session.boot_language {
            nav.replace(ScreenId::Language);
        } else {
            nav.replace(ScreenId::Connecting);
        }
    }

    /// Same as select — any confirm skips the splash.
    fn on_cancel(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        self.on_select(cx, nav);
    }

    /// Same as select.
    fn on_menu_key(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        self.on_select(cx, nav);
    }
}
