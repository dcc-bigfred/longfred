//! Router: back-stack + active screen object + nav-profile dispatch.

use crate::context::ScreenCtx;
use crate::input::InputEvent;
use crate::intent::{AppEvent, Intent};
use crate::nav::{Nav, NavCmd, PageDir, ScreenId, Step};
use crate::nav_profile::{NavAction, NavProfile};
use crate::screen::Screen;
use crate::screens::helpers::has_loco;
use crate::screens::{ScreenState, new_screen};
use crate::view::UiView;

const STACK_CAP: usize = 8;

/// Owns the current screen object and a back-stack of [`ScreenId`].
///
/// Navigation always reconstructs the destination with [`new_screen`]: list
/// cursors and keyboards do not survive `Back` / `Go`. Drafts that must persist
/// (password, address, IP digits) live in [`crate::session::UiSession`].
pub struct Router {
    current: ScreenState,
    stack: heapless::Vec<ScreenId, STACK_CAP>,
    profile: &'static dyn NavProfile,
}

impl Router {
    /// Start on `start` with a board nav profile.
    pub fn new(profile: &'static dyn NavProfile, start: ScreenId) -> Self {
        Self {
            current: new_screen(start),
            stack: heapless::Vec::new(),
            profile,
        }
    }

    /// Active [`ScreenId`].
    #[must_use]
    pub fn screen_id(&self) -> ScreenId {
        self.current.id()
    }

    /// Render the active screen.
    #[must_use]
    pub fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        self.current.view(cx)
    }

    /// Force-replace the active screen (boot wizard, Wi-Fi timeout, …).
    pub fn replace_screen(
        &mut self,
        id: ScreenId,
        cx: &mut ScreenCtx<'_>,
    ) -> heapless::Vec<Intent, 4> {
        let mut intents = heapless::Vec::new();
        self.apply(NavCmd::Replace(id), cx, &mut intents);
        intents
    }

    /// Push the current screen and open `id` (same as a screen calling [`crate::nav::Nav::go`]).
    pub fn push_screen(
        &mut self,
        id: ScreenId,
        cx: &mut ScreenCtx<'_>,
    ) -> heapless::Vec<Intent, 4> {
        let mut intents = heapless::Vec::new();
        self.apply(NavCmd::Go(id), cx, &mut intents);
        intents
    }

    /// Number of screens on the back-stack.
    #[must_use]
    pub fn stack_len(&self) -> usize {
        self.stack.len()
    }

    /// Drive one input event through the nav profile into the active screen.
    pub fn handle(&mut self, ev: InputEvent, cx: &mut ScreenCtx<'_>) -> heapless::Vec<Intent, 4> {
        let mode = self.current.key_bindings(cx);
        self.dispatch(self.profile.map(ev, mode), cx)
    }

    /// Idle tick (multitap commit, splash timeout).
    pub fn tick(&mut self, cx: &mut ScreenCtx<'_>) -> heapless::Vec<Intent, 4> {
        self.with_nav(cx, super::screen::Screen::on_tick)
    }

    /// Firmware lifecycle event (Wi-Fi ready, scan done, …).
    pub fn on_app_event(
        &mut self,
        e: AppEvent,
        cx: &mut ScreenCtx<'_>,
    ) -> heapless::Vec<Intent, 4> {
        let mut intents = heapless::Vec::new();
        let nav = match e {
            AppEvent::PairingRequired | AppEvent::PairingFailed => {
                Some(NavCmd::Replace(ScreenId::Pairing))
            }
            AppEvent::PairingStarted => Some(NavCmd::Replace(ScreenId::PairingWait)),
            AppEvent::PairingSucceeded => Some(NavCmd::Root(ScreenId::Throttle)),
            AppEvent::WifiReady
            | AppEvent::ScanDone
            | AppEvent::ServerConnected
            | AppEvent::WifiFailed => None,
        };
        if let Some(cmd) = nav {
            self.apply(cmd, cx, &mut intents);
            return intents;
        }
        self.with_nav(cx, |s, cx, nav| s.on_app_event(e, cx, nav))
    }

    fn dispatch(&mut self, action: NavAction, cx: &mut ScreenCtx<'_>) -> heapless::Vec<Intent, 4> {
        match action {
            NavAction::ListPrev => {
                self.with_nav(cx, |s, cx, nav| s.on_list_step(Step::Prev, cx, nav))
            }
            NavAction::ListNext => {
                self.with_nav(cx, |s, cx, nav| s.on_list_step(Step::Next, cx, nav))
            }
            NavAction::Select => self.with_nav(cx, super::screen::Screen::on_select),
            NavAction::Cancel => self.with_nav(cx, super::screen::Screen::on_cancel),
            NavAction::MenuEnter => self.with_nav(cx, super::screen::Screen::on_menu_key),
            NavAction::CharCycle(d) => self.with_nav(cx, |s, cx, nav| s.on_char_cycle(d, cx, nav)),
            NavAction::CursorMove(d) => {
                self.with_nav(cx, |s, cx, nav| s.on_cursor_move(d, cx, nav))
            }
            NavAction::CaseToggle => self.with_nav(cx, super::screen::Screen::on_case_toggle),
            NavAction::Digit(c) => self.with_nav(cx, |s, cx, nav| s.on_digit(c, cx, nav)),
            NavAction::PagePrev => {
                self.with_nav(cx, |s, cx, nav| s.on_page(PageDir::Prev, cx, nav))
            }
            NavAction::PageNext => {
                self.with_nav(cx, |s, cx, nav| s.on_page(PageDir::Next, cx, nav))
            }
            NavAction::PassThrough(ev) => self.passthrough(ev, cx),
        }
    }

    fn passthrough(&mut self, ev: InputEvent, cx: &mut ScreenCtx<'_>) -> heapless::Vec<Intent, 4> {
        match ev {
            InputEvent::EStop => {
                let mut out = heapless::Vec::new();
                let _ = out.push(Intent::Action(longfred_proto::action::Action::EStop));
                out
            }
            InputEvent::Stop if self.current.id() == ScreenId::Throttle => {
                let mut out = heapless::Vec::new();
                let _ = out.push(Intent::Action(longfred_proto::action::Action::EStop));
                out
            }
            InputEvent::Stop => self.with_nav(cx, super::screen::Screen::on_cancel),
            InputEvent::FnPress(k) => self.with_nav(cx, |s, cx, nav| s.on_fn_key(k, true, cx, nav)),
            InputEvent::FnRelease(k) => {
                self.with_nav(cx, |s, cx, nav| s.on_fn_key(k, false, cx, nav))
            }
            InputEvent::EnterProgrammingMode => {
                let mut out = heapless::Vec::new();
                let _ = out.push(Intent::EnterProgrammingMode);
                out
            }
            InputEvent::DirectionToggle if driving(cx) => {
                let mut out = heapless::Vec::new();
                let _ = out.push(Intent::Action(
                    longfred_proto::action::Action::DirectionToggle,
                ));
                out
            }
            InputEvent::DirectionSet(dir) if driving(cx) => {
                let mut out = heapless::Vec::new();
                let action = if dir == longfred_proto::model::Direction::Forward {
                    longfred_proto::action::Action::DirectionForward
                } else {
                    longfred_proto::action::Action::DirectionReverse
                };
                let _ = out.push(Intent::Action(action));
                out
            }
            InputEvent::EncoderClockwise
                if self.current.id() == ScreenId::Throttle && driving(cx) =>
            {
                let mut out = heapless::Vec::new();
                let _ = out.push(Intent::Action(longfred_proto::action::Action::SpeedUp));
                out
            }
            InputEvent::EncoderCounterClockwise
                if self.current.id() == ScreenId::Throttle && driving(cx) =>
            {
                let mut out = heapless::Vec::new();
                let _ = out.push(Intent::Action(longfred_proto::action::Action::SpeedDown));
                out
            }
            InputEvent::EncoderClockwise if self.current.id() == ScreenId::Throttle => {
                self.with_nav(cx, |s, cx, nav| s.on_char_cycle(1, cx, nav))
            }
            InputEvent::EncoderCounterClockwise if self.current.id() == ScreenId::Throttle => {
                self.with_nav(cx, |s, cx, nav| s.on_char_cycle(-1, cx, nav))
            }
            InputEvent::EncoderButton if self.current.id() == ScreenId::Throttle && driving(cx) => {
                let mut out = heapless::Vec::new();
                let _ = out.push(Intent::Action(
                    longfred_proto::action::Action::SpeedStopThenToggleDirection,
                ));
                out
            }
            InputEvent::SpeedAbsolute(v) if driving(cx) => {
                let mut out = heapless::Vec::new();
                let _ = out.push(Intent::Action(longfred_proto::action::Action::SpeedSet(v)));
                out
            }
            InputEvent::LocoSlot(slot, true) => {
                let mut out = heapless::Vec::new();
                let _ = out.push(Intent::Action(longfred_proto::action::Action::Throttle(
                    slot,
                )));
                out
            }
            _ => heapless::Vec::new(),
        }
    }

    fn with_nav(
        &mut self,
        cx: &mut ScreenCtx<'_>,
        f: impl FnOnce(&mut ScreenState, &mut ScreenCtx<'_>, &mut Nav<'_>),
    ) -> heapless::Vec<Intent, 4> {
        let mut cmd = None;
        let mut intents = heapless::Vec::new();
        {
            let mut nav = Nav::new(&mut cmd, &mut intents);
            f(&mut self.current, cx, &mut nav);
        }
        if let Some(cmd) = cmd {
            self.apply(cmd, cx, &mut intents);
        }
        intents
    }

    const MAX_NAV_HOPS: u8 = 4;

    fn apply(
        &mut self,
        cmd: NavCmd,
        cx: &mut ScreenCtx<'_>,
        intents: &mut heapless::Vec<Intent, 4>,
    ) {
        let mut next = Some(cmd);
        for _ in 0..Self::MAX_NAV_HOPS {
            let Some(cmd) = next.take() else {
                return;
            };
            next = self.step(cmd, cx, intents);
        }
        debug_assert!(next.is_none(), "navigation did not settle");
    }

    fn step(
        &mut self,
        cmd: NavCmd,
        cx: &mut ScreenCtx<'_>,
        intents: &mut heapless::Vec<Intent, 4>,
    ) -> Option<NavCmd> {
        match cmd {
            NavCmd::Go(id) => {
                let from = self.current.id();
                if from == id {
                    return None;
                }
                if self.stack.is_full() {
                    let _ = self.stack.remove(0);
                }
                let _ = self.stack.push(from);
                self.enter_screen(id, cx, intents)
            }
            NavCmd::Replace(id) => {
                if self.current.id() == id {
                    None
                } else {
                    self.enter_screen(id, cx, intents)
                }
            }
            NavCmd::Back => {
                let id = self.stack.pop().unwrap_or(ScreenId::Throttle);
                self.enter_screen(id, cx, intents)
            }
            NavCmd::Root(id) => {
                self.stack.clear();
                self.enter_screen(id, cx, intents)
            }
        }
    }

    fn enter_screen(
        &mut self,
        id: ScreenId,
        cx: &mut ScreenCtx<'_>,
        intents: &mut heapless::Vec<Intent, 4>,
    ) -> Option<NavCmd> {
        self.current = new_screen(id);
        let mut cmd = None;
        {
            let mut nav = Nav::new(&mut cmd, intents);
            self.current.on_enter(cx, &mut nav);
        }
        cmd
    }
}

fn driving(cx: &ScreenCtx<'_>) -> bool {
    has_loco(cx)
}
