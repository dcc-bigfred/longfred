//! Language picker (boot wizard or Extras).

use longfred_proto::persist::Language;

use super::helpers::page_list;
use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::screen::{KeyBindings, MenuModel, Screen};
use crate::view::UiView;
use crate::widgets::PagedList;

pub struct LanguageScreen {
    list: PagedList,
}

impl LanguageScreen {
    /// Unnumbered EN/PL/DE picker.
    pub fn new() -> Self {
        Self {
            list: PagedList::new(false),
        }
    }

    /// Localized language names.
    fn labels(cx: &ScreenCtx<'_>) -> [&'static str; 3] {
        [cx.s.lang_en, cx.s.lang_pl, cx.s.lang_de]
    }

    /// Display height used for paging.
    fn height(cx: &ScreenCtx<'_>) -> u16 {
        cx.env.geometry.height
    }
}

impl Default for LanguageScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for LanguageScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Language
    }

    fn key_bindings(&self, _cx: &ScreenCtx<'_>) -> KeyBindings {
        KeyBindings::NAVIGATION
    }

    /// No [`MenuModel`]; the router uses list handlers instead.
    fn menu<'a>(&'a self, cx: &'a ScreenCtx<'_>) -> Option<MenuModel<'a>> {
        let _ = cx;
        None
    }

    /// Language names plus a footer hint.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        let labels = Self::labels(cx);
        self.list.draw(
            &mut g,
            Some(cx.s.msg_language),
            &labels,
            false,
            Self::height(cx),
        );
        g.set(5, cx.s.hint_language, false);
        UiView::Grid(g)
    }

    /// Move the highlighted language row.
    fn on_list_step(&mut self, d: Step, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        match d {
            Step::Prev => self.list.list_prev(&labels, false, Self::height(cx)),
            Step::Next => self.list.list_next(&labels, false, Self::height(cx)),
        }
    }

    /// Digit jumps to that row and selects it.
    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if let Some(d) = c.to_digit(10) {
            let labels = Self::labels(cx);
            if self
                .list
                .select_digit(d as u8, &labels, false, Self::height(cx))
                .is_some()
            {
                self.on_select(cx, nav);
            }
        }
    }

    /// Persist the language. On first boot stay here so firmware can start the Wi-Fi wizard.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let idx = {
            let labels = Self::labels(cx);
            self.list.global_index(&labels, false, Self::height(cx))
        };
        let lang = match idx {
            1 => Language::Pl,
            2 => Language::De,
            _ => Language::En,
        };
        nav.emit(Intent::SetLanguage(lang));
        if cx.session.boot_language {
            cx.session.boot_language = false;
        } else {
            nav.back();
        }
    }

    /// Page the language list if it overflows.
    fn on_page(&mut self, d: PageDir, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        page_list(&mut self.list, d, &labels, false, Self::height(cx));
    }

    /// Back is ignored during the boot wizard; otherwise pop to Extras.
    fn on_cancel(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if cx.session.boot_language {
            return;
        }
        nav.back();
    }
}
