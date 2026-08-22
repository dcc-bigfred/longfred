//! Live Wi-Fi scan results.

use longfred_proto::model::MAX_FOUND_SSIDS;

use super::helpers::{
    digit_key, height, list_digit, list_star_confirms, page_list, pick_ssid, set_list_hint,
    step_list,
};
use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::screen::Screen;
use crate::view::UiView;
use crate::widgets::PagedList;

/// Live Wi-Fi scan results.
pub struct SsidScanScreen {
    list: PagedList,
}

impl SsidScanScreen {
    /// Numbered list of last scan results.
    #[must_use]
    pub fn new() -> Self {
        Self {
            list: PagedList::new(true).with_footer(true),
        }
    }

    fn names<'a>(cx: &'a ScreenCtx<'_>) -> heapless::Vec<&'a str, MAX_FOUND_SSIDS> {
        let mut names = heapless::Vec::new();
        for s in cx.net.scanned_ssids {
            let _ = names.push(s.ssid.as_str());
        }
        names
    }

    fn pick_at(cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>, idx: usize) {
        cx.session.selected_ssid_idx = idx;
        cx.session.selected_from_scan = true;
        if let Some(s) = cx.net.scanned_ssids.get(idx) {
            let ssid = s.ssid.clone();
            pick_ssid(cx, ssid.as_str());
            nav.go(ScreenId::Password);
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
        let names = Self::names(cx);
        self.list
            .draw(&mut g, Some(cx.s.msg_ssids_found), &names, height(cx));
        set_list_hint(&mut g, cx, cx.s.hint_ssid_list);
        UiView::Grid(g)
    }

    /// Move the highlighted scanned-SSID row.
    fn on_list_step(&mut self, d: Step, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let names = Self::names(cx);
        step_list(&mut self.list, d, &names, height(cx));
    }

    /// Page the scan-result list.
    fn on_page(&mut self, d: PageDir, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let names = Self::names(cx);
        page_list(&mut self.list, d, &names, height(cx));
    }

    /// Menu restarts a scan.
    fn on_menu_key(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.replace(ScreenId::SsidScanning);
        nav.emit(Intent::WifiScan);
    }

    /// Digit jumps to that SSID and selects it; `*` builds a 1-based index.
    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let Some(d) = digit_key(c) else { return };
        let idx = {
            let names = Self::names(cx);
            let h = height(cx);
            list_digit(&mut self.list, d, &names, h)
        };
        if let Some(idx) = idx {
            Self::pick_at(cx, nav, idx);
        }
    }

    fn on_star(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let idx = {
            let names = Self::names(cx);
            let h = height(cx);
            list_star_confirms(&mut self.list, &names, h).then(|| self.list.global_index(&names, h))
        };
        if let Some(idx) = idx {
            Self::pick_at(cx, nav, idx);
        }
    }

    fn on_fn_key(&mut self, k: u8, down: bool, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if !down {
            return;
        }
        let idx = {
            let names = Self::names(cx);
            let h = height(cx);
            let found = self.list.select_fn_key(k, &names, h);
            if found.is_some() {
                let _ = self.list.clear_index();
            }
            found
        };
        if let Some(idx) = idx {
            Self::pick_at(cx, nav, idx);
        }
    }

    /// Remember the scanned SSID and open the password screen.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let _ = self.list.clear_index();
        let idx = {
            let names = Self::names(cx);
            self.list.global_index(&names, height(cx))
        };
        Self::pick_at(cx, nav, idx);
    }

    /// Settings scan pops; boot with compiled SSIDs goes to that list; else stay.
    fn on_cancel(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if self.list.clear_index() {
            return;
        }
        if cx.session.wifi_from_settings {
            nav.back();
        } else if !cx.env.compiled_networks.is_empty() {
            nav.replace(ScreenId::SsidList);
        }
    }
}
