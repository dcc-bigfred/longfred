//! Compiled-SSID picker.

use super::helpers::{compiled_ssids, height, pick_ssid, step_list};
use crate::context::ScreenCtx;
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::screen::Screen;
use crate::view::UiView;
use crate::widgets::PagedList;

pub struct SsidListScreen {
    list: PagedList,
}

impl SsidListScreen {
    /// Numbered list of firmware-compiled SSIDs.
    pub fn new() -> Self {
        Self {
            list: PagedList::new(true),
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

    /// Compiled SSIDs; page-right starts a live scan rather than paging.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        let names = compiled_ssids(cx);
        self.list.draw(
            &mut g,
            Some(cx.s.msg_ssids_listed),
            &names,
            true,
            height(cx),
        );
        UiView::Grid(g)
    }

    /// Move the highlighted compiled-SSID row.
    fn on_list_step(&mut self, d: Step, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let names = compiled_ssids(cx);
        step_list(&mut self.list, d, &names, true, height(cx));
    }

    /// Page-next starts a scan (not a list page).
    fn on_page(&mut self, d: PageDir, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if d == PageDir::Next {
            nav.replace(ScreenId::SsidScanning);
        }
    }

    /// Digit jumps to that SSID and selects it.
    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if let Some(d) = c.to_digit(10) {
            let hit = {
                let names = compiled_ssids(cx);
                self.list
                    .select_digit(d as u8, &names, true, height(cx))
                    .is_some()
            };
            if hit {
                self.on_select(cx, nav);
            }
        }
    }

    /// Remember the compiled SSID and open the password screen.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let idx = {
            let names = compiled_ssids(cx);
            self.list.global_index(&names, true, height(cx))
        };
        let ssid = cx.env.compiled_networks.get(idx).map(|n| n.ssid);
        if let Some(ssid) = ssid {
            cx.session.selected_ssid_idx = idx;
            cx.session.selected_from_scan = false;
            pick_ssid(cx, ssid);
            nav.go(ScreenId::Password);
        }
    }

    /// Leave the wizard for the drive HUD.
    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.root(ScreenId::Throttle);
    }
}
