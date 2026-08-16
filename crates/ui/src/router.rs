//! Router: back-stack + active screen object + nav-profile dispatch.

use crate::context::ScreenCtx;
use crate::input::InputEvent;
use crate::intent::{AppEvent, Intent};
use crate::nav::{Nav, NavCmd, PageDir, ScreenId, Step};
use crate::nav_profile::{NavAction, NavProfile};
use crate::screen::Screen;
use crate::screens::{ScreenState, new_screen};
use crate::view::UiView;

const STACK_CAP: usize = 8;

/// Owns the current screen object and a back-stack of [`ScreenId`].
pub struct Router {
    current: ScreenState,
    stack: heapless::Vec<ScreenId, STACK_CAP>,
    profile: &'static dyn NavProfile,
}

impl Router {
    pub fn new(profile: &'static dyn NavProfile, start: ScreenId) -> Self {
        Self {
            current: new_screen(start),
            stack: heapless::Vec::new(),
            profile,
        }
    }

    pub fn screen_id(&self) -> ScreenId {
        self.current.id()
    }

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

    /// Drive one input event through the nav profile into the active screen.
    pub fn handle(&mut self, ev: InputEvent, cx: &mut ScreenCtx<'_>) -> heapless::Vec<Intent, 4> {
        let bindings = self.current.key_bindings(cx);
        let Some(action) = self.profile.map(ev, bindings.text_entry, bindings.throttle) else {
            return heapless::Vec::new();
        };
        self.dispatch(action, cx)
    }

    pub fn tick(&mut self, cx: &mut ScreenCtx<'_>) -> heapless::Vec<Intent, 4> {
        self.with_nav(cx, |s, cx, nav| s.on_tick(cx, nav))
    }

    pub fn on_app_event(
        &mut self,
        e: AppEvent,
        cx: &mut ScreenCtx<'_>,
    ) -> heapless::Vec<Intent, 4> {
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
            NavAction::Select => self.with_nav(cx, |s, cx, nav| s.on_select(cx, nav)),
            NavAction::Cancel => self.with_nav(cx, |s, cx, nav| s.on_cancel(cx, nav)),
            NavAction::MenuEnter => self.with_nav(cx, |s, cx, nav| s.on_menu_key(cx, nav)),
            NavAction::CharCycle(d) => self.with_nav(cx, |s, cx, nav| s.on_char_cycle(d, cx, nav)),
            NavAction::CursorMove(d) => {
                self.with_nav(cx, |s, cx, nav| s.on_cursor_move(d, cx, nav))
            }
            NavAction::CaseToggle => self.with_nav(cx, |s, cx, nav| s.on_case_toggle(cx, nav)),
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
            InputEvent::Stop => self.with_nav(cx, |s, cx, nav| s.on_cancel(cx, nav)),
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

    fn apply(
        &mut self,
        cmd: NavCmd,
        cx: &mut ScreenCtx<'_>,
        intents: &mut heapless::Vec<Intent, 4>,
    ) {
        match cmd {
            NavCmd::Go(id) => {
                let from = self.current.id();
                if from != id {
                    let _ = self.stack.push(from);
                    self.enter(id, cx, intents);
                }
            }
            NavCmd::Replace(id) => {
                if self.current.id() != id {
                    self.enter(id, cx, intents);
                }
            }
            NavCmd::Back => {
                if let Some(id) = self.stack.pop() {
                    self.enter(id, cx, intents);
                } else {
                    self.enter(ScreenId::Throttle, cx, intents);
                }
            }
            NavCmd::Root(id) => {
                self.stack.clear();
                self.enter(id, cx, intents);
            }
        }
    }

    fn enter(
        &mut self,
        id: ScreenId,
        cx: &mut ScreenCtx<'_>,
        intents: &mut heapless::Vec<Intent, 4>,
    ) {
        self.current = new_screen(id);
        let mut cmd = None;
        {
            let mut nav = Nav::new(&mut cmd, intents);
            self.current.on_enter(cx, &mut nav);
        }
        if let Some(cmd) = cmd {
            self.apply(cmd, cx, intents);
        }
    }
}

fn driving(cx: &ScreenCtx<'_>) -> bool {
    cx.drive
        .slots
        .get(cx.drive.current)
        .is_some_and(longfred_proto::model::ThrottleSlot::has_loco)
}
