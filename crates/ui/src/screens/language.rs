//! Language picker (boot wizard or Extras).

use longfred_proto::persist::Language;

use super::helpers::{digit_key, list_label_digit, list_star_confirms, page_list, set_list_hint};
use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::screen::{KeyBindings, Screen};
use crate::view::UiView;
use crate::widgets::PagedList;

/// Language picker (boot wizard or Extras).
pub struct LanguageScreen {
    list: PagedList,
}

impl LanguageScreen {
    const LANGS: [Language; 3] = [Language::En, Language::Pl, Language::De];

    /// Unnumbered EN/PL/DE picker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            list: PagedList::new(false).with_footer(true),
        }
    }

    fn labels(cx: &ScreenCtx<'_>) -> [&'static str; 3] {
        Self::LANGS.map(|lang| match lang {
            Language::En => cx.s.lang_en,
            Language::Pl => cx.s.lang_pl,
            Language::De => cx.s.lang_de,
        })
    }

    fn height(cx: &ScreenCtx<'_>) -> u16 {
        cx.env.geometry.height
    }

    fn current_at(&self, labels: &[&str], h: u16) -> Language {
        Self::LANGS
            .get(self.list.global_index(labels, h))
            .copied()
            .unwrap_or(Language::En)
    }

    fn current(&self, cx: &ScreenCtx<'_>) -> Language {
        self.current_at(&Self::labels(cx), Self::height(cx))
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

    /// Language names plus a footer hint.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        let labels = Self::labels(cx);
        self.list
            .draw(&mut g, Some(cx.s.msg_language), &labels, Self::height(cx));
        set_list_hint(&mut g, cx, cx.s.hint_language);
        UiView::Grid(g)
    }

    /// Move the highlighted language row.
    fn on_list_step(&mut self, d: Step, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        match d {
            Step::Prev => self.list.list_prev(&labels, Self::height(cx)),
            Step::Next => self.list.list_next(&labels, Self::height(cx)),
        }
    }

    /// Digit jumps to that row and selects it; `*` builds a 1-based index.
    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let Some(d) = digit_key(c) else { return };
        let labels = Self::labels(cx);
        let h = Self::height(cx);
        if list_label_digit(&mut self.list, d, &labels, h).is_some() {
            nav.emit(Intent::SetLanguage(self.current_at(&labels, h)));
            if cx.session.boot_language {
                cx.session.boot_language = false;
            } else {
                nav.back();
            }
        }
    }

    fn on_star(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        let h = Self::height(cx);
        if list_star_confirms(&mut self.list, &labels, h) {
            nav.emit(Intent::SetLanguage(self.current_at(&labels, h)));
            if cx.session.boot_language {
                cx.session.boot_language = false;
            } else {
                nav.back();
            }
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
            nav.emit(Intent::SetLanguage(self.current_at(&labels, h)));
            if cx.session.boot_language {
                cx.session.boot_language = false;
            } else {
                nav.back();
            }
        }
    }

    /// Persist the language. On first boot stay here so firmware can start the Wi-Fi wizard.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let _ = self.list.clear_index();
        nav.emit(Intent::SetLanguage(self.current(cx)));
        if cx.session.boot_language {
            cx.session.boot_language = false;
        } else {
            nav.back();
        }
    }

    /// Page the language list if it overflows.
    fn on_page(&mut self, d: PageDir, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let labels = Self::labels(cx);
        page_list(&mut self.list, d, &labels, Self::height(cx));
    }

    /// Back is ignored during the boot wizard; otherwise pop to Extras.
    fn on_cancel(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if self.list.clear_index() {
            return;
        }
        if cx.session.boot_language {
            return;
        }
        nav.back();
    }
}
