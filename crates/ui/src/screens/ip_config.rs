//! Client IPv4 config summary.

use super::helpers::write_ip_line;
use crate::context::ScreenCtx;
use crate::nav::{Nav, ScreenId};
use crate::screen::Screen;
use crate::view::{Line, UiView};

/// Client IPv4 config summary.
pub struct IpConfigScreen;

impl Screen for IpConfigScreen {
    fn id(&self) -> ScreenId {
        ScreenId::IpConfig
    }

    /// DHCP vs static summary; static also shows the current IPv4.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        g.set(0, cx.s.msg_net_config, false);
        match cx.drive.persist.network {
            Some(n) if !n.dhcp => {
                let mut line = Line::new();
                let _ = line.push_str(cx.s.msg_net_static);
                let _ = line.push(' ');
                write_ip_line(&mut line, n.ip);
                g.set(1, line.as_str(), false);
            }
            _ => {
                g.set(1, cx.s.msg_net_dhcp, false);
            }
        }
        g.set(5, cx.s.hint_net_config, false);
        UiView::Grid(g)
    }

    /// Copy persist into the session draft and replace with the field editor (Back → Extras).
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        cx.session.net_cfg = cx.drive.persist.network.unwrap_or_default();
        cx.session.ip_field = crate::session::NetField::Dhcp;
        nav.replace(ScreenId::IpEdit);
    }

    /// Return to Extras.
    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.back();
    }
}
