//! `BigFred` pairing progress screen.

use crate::context::ScreenCtx;
use crate::nav::{Nav, ScreenId};
use crate::screen::Screen;
use crate::view::UiView;

/// Non-interactive wait while the adapter sends function digits.
pub struct PairingWaitScreen;

impl Screen for PairingWaitScreen {
    fn id(&self) -> ScreenId {
        ScreenId::PairingWait
    }

    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        g.set(2, cx.s.msg_pairing, false);
        UiView::Grid(g)
    }

    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.root(ScreenId::Throttle);
    }
}
