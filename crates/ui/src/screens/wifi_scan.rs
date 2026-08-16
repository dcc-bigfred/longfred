//! Live Wi-Fi scan results.

use longfred_proto::model::MAX_FOUND_SSIDS;

use super::helpers::{height, page_list, pick_ssid, step_list};
use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::screen::Screen;
use crate::view::UiView;
use crate::widgets::PagedList;

pub struct SsidScanScreen {
    list: PagedList,
}

impl SsidScanScreen {
    /// Numbered list of last scan results.
    pub fn new() -> Self {
        Self {
            list: PagedList::new(true),
        }
    }
}

impl Default for SsidScanScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for SsidScanScreen {
    fn id(&self) -> ScreenId {
        ScreenId::SsidScan
    }

    /// SSIDs from the last Wi-Fi scan.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        let mut names: heapless::Vec<&str, MAX_FOUND_SSIDS> = heapless::Vec::new();
        for s in cx.net.scanned_ssids {
            let _ = names.push(s.ssid.as_str());
        }
        self.list
            .draw(&mut g, Some(cx.s.msg_ssids_found), &names, true, height(cx));
        UiView::Grid(g)
    }

    /// Move the highlighted scanned-SSID row.
    fn on_list_step(&mut self, d: Step, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let mut names: heapless::Vec<&str, MAX_FOUND_SSIDS> = heapless::Vec::new();
        for s in cx.net.scanned_ssids {
            let _ = names.push(s.ssid.as_str());
        }
        step_list(&mut self.list, d, &names, true, height(cx));
    }

    /// Page the scan-result list.
    fn on_page(&mut self, d: PageDir, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let mut names: heapless::Vec<&str, MAX_FOUND_SSIDS> = heapless::Vec::new();
        for s in cx.net.scanned_ssids {
            let _ = names.push(s.ssid.as_str());
        }
        page_list(&mut self.list, d, &names, true, height(cx));
    }

    /// Menu restarts a scan.
    fn on_menu_key(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.replace(ScreenId::SsidScanning);
        nav.emit(Intent::WifiScan);
    }

    /// Digit jumps to that SSID and selects it.
    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if let Some(d) = c.to_digit(10) {
            let hit = {
                let mut names: heapless::Vec<&str, MAX_FOUND_SSIDS> = heapless::Vec::new();
                for s in cx.net.scanned_ssids {
                    let _ = names.push(s.ssid.as_str());
                }
                self.list
                    .select_digit(d as u8, &names, true, height(cx))
                    .is_some()
            };
            if hit {
                self.on_select(cx, nav);
            }
        }
    }

    /// Remember the scanned SSID and open the password screen.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let idx = {
            let mut names: heapless::Vec<&str, MAX_FOUND_SSIDS> = heapless::Vec::new();
            for s in cx.net.scanned_ssids {
                let _ = names.push(s.ssid.as_str());
            }
            self.list.global_index(&names, true, height(cx))
        };
        cx.session.selected_ssid_idx = idx;
        cx.session.selected_from_scan = true;
        if let Some(s) = cx.net.scanned_ssids.get(idx) {
            let ssid = s.ssid.clone();
            pick_ssid(cx, ssid.as_str());
            nav.go(ScreenId::Password);
        }
    }

    /// Back to compiled SSIDs when they exist; otherwise stay (scan is the root list).
    fn on_cancel(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if !cx.env.compiled_networks.is_empty() {
            nav.replace(ScreenId::SsidList);
        }
    }
}
