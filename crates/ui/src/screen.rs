//! Per-screen contract (keys, menu, display, event handlers).

use crate::context::ScreenCtx;
use crate::intent::AppEvent;
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::view::UiView;

/// How hardware events should be interpreted on this screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyBindings {
    pub text_entry: bool,
    pub throttle: bool,
}

impl KeyBindings {
    pub const NAVIGATION: Self = Self {
        text_entry: false,
        throttle: false,
    };
    pub const TEXT: Self = Self {
        text_entry: true,
        throttle: false,
    };
    pub const THROTTLE: Self = Self {
        text_entry: false,
        throttle: true,
    };
}

/// Choice-list description owned by the screen (labels live in the caller).
pub struct MenuModel<'a> {
    pub title: Option<&'a str>,
    pub items: &'a [&'a str],
    pub numbered: bool,
}

/// One UI screen: mapping, rendering, and input.
pub trait Screen {
    fn id(&self) -> ScreenId;

    fn key_bindings(&self, _cx: &ScreenCtx<'_>) -> KeyBindings {
        KeyBindings::NAVIGATION
    }

    fn menu<'a>(&'a self, _cx: &'a ScreenCtx<'_>) -> Option<MenuModel<'a>> {
        None
    }

    fn view(&self, cx: &ScreenCtx<'_>) -> UiView;

    fn on_enter(&mut self, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}

    fn on_select(&mut self, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}

    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.back();
    }

    fn on_digit(&mut self, _c: char, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}

    fn on_list_step(&mut self, _d: Step, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}

    fn on_page(&mut self, _d: PageDir, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}

    fn on_char_cycle(&mut self, _d: i8, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}

    fn on_cursor_move(&mut self, _d: i8, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}

    fn on_case_toggle(&mut self, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}

    fn on_menu_key(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.go(ScreenId::Menu);
    }

    fn on_fn_key(&mut self, _k: u8, _down: bool, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}

    fn on_tick(&mut self, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}

    fn on_app_event(&mut self, _e: AppEvent, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}
}
