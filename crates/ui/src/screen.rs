//! Per-screen contract (keys, menu, display, event handlers).

use crate::context::ScreenCtx;
use crate::intent::AppEvent;
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::view::UiView;

/// How hardware events should be interpreted on this screen.
///
/// Profiles map keys differently in text entry vs the drive HUD vs list navigation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMode {
    /// Lists, wizards, and most settings screens.
    Navigation,
    /// Password / address / IP editors (joystick cycles characters).
    Text,
    /// Drive HUD: encoder is speed, digits are functions.
    Throttle,
}

impl InputMode {
    /// Alias for [`Self::Navigation`].
    pub const NAVIGATION: Self = Self::Navigation;
    /// Alias for [`Self::Text`].
    pub const TEXT: Self = Self::Text;
    /// Alias for [`Self::Throttle`].
    pub const THROTTLE: Self = Self::Throttle;
}

/// Alias kept so existing screens can keep returning `KeyBindings::NAVIGATION`.
pub type KeyBindings = InputMode;

/// One UI screen: mapping, rendering, and input.
pub trait Screen {
    /// Stable identity used by the router and back-stack.
    fn id(&self) -> ScreenId;

    /// How the nav profile should interpret hardware events here.
    fn key_bindings(&self, _cx: &ScreenCtx<'_>) -> KeyBindings {
        KeyBindings::NAVIGATION
    }

    /// Current OLED view model.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView;

    /// Called when this screen object is constructed (after nav).
    fn on_enter(&mut self, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}

    /// Confirm / select the highlighted item.
    fn on_select(&mut self, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}

    /// Cancel / back. Default pops the stack.
    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.back();
    }

    /// Physical Stop off the drive HUD. Default cancels.
    fn on_stop(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        self.on_cancel(cx, nav);
    }

    /// Keypad `*` in navigation mode. Default cancels.
    fn on_star(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        self.on_cancel(cx, nav);
    }

    /// Keypad digit (and `0` = tenth numbered row).
    fn on_digit(&mut self, _c: char, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}

    /// Move the list cursor one row.
    fn on_list_step(&mut self, _d: Step, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}

    /// Flip a page or leave the screen.
    fn on_page(&mut self, _d: PageDir, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}

    /// Cycle the character under the text caret.
    fn on_char_cycle(&mut self, _d: i8, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}

    /// Move the text caret.
    fn on_cursor_move(&mut self, _d: i8, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}

    /// Toggle text-entry case.
    fn on_case_toggle(&mut self, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}

    /// Hardware Menu key. Default opens [`ScreenId::Menu`].
    fn on_menu_key(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.go(ScreenId::Menu);
    }

    /// Function key (down/up). Used as digits on text screens.
    fn on_fn_key(&mut self, _k: u8, _down: bool, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}

    /// Idle tick (multitap commit, splash timeout, …).
    fn on_tick(&mut self, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}

    /// Firmware lifecycle event (Wi-Fi ready, scan done, …).
    fn on_app_event(&mut self, _e: AppEvent, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}
}
