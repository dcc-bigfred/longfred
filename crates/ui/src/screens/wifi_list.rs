//! Compiled-SSID picker.

use super::helpers::{
    compiled_ssids, digit_key, height, page_list, pick_ssid, set_list_hint, step_list,
};
use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::screen::Screen;
use crate::view::UiView;
use crate::widgets::PagedList;

/// Compiled-SSID picker.
pub struct SsidListScreen {
    list: PagedList,
}

impl SsidListScreen {
    /// Numbered list of firmware-compiled SSIDs.
    #[must_use]
    pub fn new() -> Self {
        Self {
            list: PagedList::new(true),
        }
    }

    fn pick_at(cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>, idx: usize) {
        let ssid = cx.env.compiled_networks.get(idx).map(|n| n.ssid);
        if let Some(ssid) = ssid {
            cx.session.selected_ssid_idx = idx;
            cx.session.selected_from_scan = false;
            pick_ssid(cx, ssid);
            nav.go(ScreenId::Password);
        }
    }
}

impl Default for SsidListScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for SsidListScreen {
    fn id(&self) -> ScreenId {
        ScreenId::SsidList
    }

    /// Compiled SSIDs; footer on 128×64 lists key hints.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        let names = compiled_ssids(cx);
        self.list
            .draw(&mut g, Some(cx.s.msg_ssids_listed), &names, height(cx));
        set_list_hint(&mut g, cx, cx.s.hint_ssid_list);
        UiView::Grid(g)
    }

    /// Move the highlighted compiled-SSID row.
    fn on_list_step(&mut self, d: Step, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let names = compiled_ssids(cx);
        step_list(&mut self.list, d, &names, height(cx));
    }

    /// Left / Right Menu pages the compiled-SSID list.
    fn on_page(&mut self, d: PageDir, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let names = compiled_ssids(cx);
        page_list(&mut self.list, d, &names, height(cx));
    }

    /// Menu starts a live scan.
    fn on_menu_key(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.replace(ScreenId::SsidScanning);
        nav.emit(Intent::WifiScan);
    }

    /// Digit jumps to that SSID and selects it.
    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let Some(d) = digit_key(c) else { return };
        let idx = {
            let names = compiled_ssids(cx);
            let h = height(cx);
            self.list
                .select_digit(d, &names, h)
                .is_some()
                .then(|| self.list.global_index(&names, h))
        };
        if let Some(idx) = idx {
            Self::pick_at(cx, nav, idx);
        }
    }

    /// Remember the compiled SSID and open the password screen.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let idx = {
            let names = compiled_ssids(cx);
            self.list.global_index(&names, height(cx))
        };
        Self::pick_at(cx, nav, idx);
    }

    /// Leave the wizard for the drive HUD.
    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.root(ScreenId::Throttle);
    }
}
