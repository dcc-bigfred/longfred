//! HTTP OTA toggle screen.

use super::helpers::write_ip_line;
use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, ScreenId};
use crate::screen::Screen;
use crate::view::{Line, UiView};

pub struct FirmwareUpdateScreen;

impl Screen for FirmwareUpdateScreen {
    fn id(&self) -> ScreenId {
        ScreenId::FirmwareUpdate
    }

    /// STA IP, HTTP-OTA on/off, and an in-progress line while a transfer runs.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        g.set(0, cx.s.msg_fw_update, false);
        if let Some(ip) = cx.net.sta_ipv4 {
            let mut line = Line::new();
            write_ip_line(&mut line, ip);
            g.set(1, line.as_str(), false);
        } else {
            g.set(1, cx.s.msg_fw_no_ip, false);
        }
        g.set(
            2,
            if cx.net.http_ota {
                cx.s.msg_fw_http_on
            } else {
                cx.s.msg_fw_http_off
            },
            false,
        );
        if cx.net.http_ota_busy {
            g.set(3, cx.s.msg_fw_updating, false);
        }
        g.set(5, cx.s.hint_fw_update, false);
        UiView::Grid(g)
    }

    /// Toggle HTTP OTA.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.emit(Intent::SetHttpOta(!cx.net.http_ota));
    }

    /// Disable HTTP OTA and leave.
    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.emit(Intent::SetHttpOta(false));
        nav.back();
    }

    /// Disable HTTP OTA and open the main menu.
    fn on_menu_key(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.emit(Intent::SetHttpOta(false));
        nav.go(ScreenId::Menu);
    }
}
