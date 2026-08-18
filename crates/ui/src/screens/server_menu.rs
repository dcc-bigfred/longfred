//! Server submenu: reconnect, pair, change connection, disconnect.

use super::helpers::{digit_key, list_digit, list_star_confirms, page_list};
use crate::context::ScreenCtx;
use crate::i18n::Strings;
use crate::intent::Intent;
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::screen::{KeyBindings, Screen};
use crate::session::ChoiceKind;
use crate::view::UiView;
use crate::widgets::PagedList;

#[derive(Clone, Copy)]
enum ServerMenuItem {
    Reconnect,
    Pair,
    Change,
    Disconnect,
}

impl ServerMenuItem {
    const ALL: [Self; 4] = [Self::Reconnect, Self::Pair, Self::Change, Self::Disconnect];

    fn label(self, s: &Strings) -> &'static str {
        match self {
            Self::Reconnect => s.server_reconnect,
            Self::Pair => s.server_pair,
            Self::Change => s.server_change,
            Self::Disconnect => s.server_disconnect,
        }
    }

    fn activate(self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        match self {
            Self::Reconnect => {
                if cx.drive.persist.last_server.is_some() {
                    nav.emit(Intent::ServerReconnect);
                } else {
                    nav.overlay(cx.s.overlay_no_saved_server);
                }
            }
            Self::Pair => nav.go(ScreenId::Pairing),
            Self::Change => {
                cx.session.choice = ChoiceKind::ServerConnect;
                nav.go(ScreenId::Choice);
            }
            Self::Disconnect => nav.emit(Intent::ServerDisconnect),
        }
    }
}

/// Server submenu from the main menu.
pub struct ServerMenuScreen {
    list: PagedList,
}

impl ServerMenuScreen {
    /// Numbered four-item server menu.
    #[must_use]
    pub fn new() -> Self {
        Self {
            list: PagedList::new(true),
        }
    }

    fn labels(cx: &ScreenCtx<'_>) -> [&'static str; 4] {
        ServerMenuItem::ALL.map(|item| item.label(cx.s))
    }

    fn height(cx: &ScreenCtx<'_>) -> u16 {
        cx.env.geometry.height
    }

    fn current_at(&self, labels: &[&str], h: u16) -> Option<ServerMenuItem> {
        ServerMenuItem::ALL
            .get(self.list.global_index(labels, h))
            .copied()
    }

    fn current(&self, cx: &ScreenCtx<'_>) -> Option<ServerMenuItem> {
        self.current_at(&Self::labels(cx), Self::height(cx))
    }
}

impl Default for ServerMenuScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for ServerMenuScreen {
    fn id(&self) -> ScreenId {
        ScreenId::ServerMenu
    }

    fn key_bindings(&self, _cx: &ScreenCtx<'_>) -> KeyBindings {
        KeyBindings::NAVIGATION
    }

    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        let labels = Self::labels(cx);
        self.list
            .draw(&mut g, Some(cx.s.menu_server), &labels, Self::height(cx));
        UiView::Grid(g)
    }

    fn on_list_step(&mut self, d: Step, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        match d {
            Step::Prev => self.list.list_prev(&labels, Self::height(cx)),
            Step::Next => self.list.list_next(&labels, Self::height(cx)),
        }
    }

    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let Some(d) = digit_key(c) else { return };
        let labels = Self::labels(cx);
        let h = Self::height(cx);
        if list_digit(&mut self.list, d, &labels, h).is_some()
            && let Some(item) = self.current_at(&labels, h)
        {
            item.activate(cx, nav);
        }
    }

    fn on_star(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        let h = Self::height(cx);
        if list_star_confirms(&mut self.list, &labels, h)
            && let Some(item) = self.current_at(&labels, h)
        {
            item.activate(cx, nav);
        }
    }

    fn on_fn_key(&mut self, k: u8, down: bool, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if !down {
            return;
        }
        let labels = Self::labels(cx);
        let h = Self::height(cx);
        if self.list.select_fn_key(k, &labels, h).is_some() {
            let _ = self.list.clear_index();
            if let Some(item) = self.current_at(&labels, h) {
                item.activate(cx, nav);
            }
        }
    }

    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let _ = self.list.clear_index();
        if let Some(item) = self.current(cx) {
            item.activate(cx, nav);
        }
    }

    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if self.list.clear_index() {
            return;
        }
        nav.back();
    }

    fn on_page(&mut self, d: PageDir, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        page_list(&mut self.list, d, &labels, Self::height(cx));
    }
}
