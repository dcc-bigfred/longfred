//! Per-variant navigation profile: InputEvent → canonical NavAction.

use crate::input::{InputEvent, NavDir};

/// Canonical UI navigation vocabulary (screen-agnostic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavAction {
    ListPrev,
    ListNext,
    Select,
    Cancel,
    MenuEnter,
    CharCycle(i8),
    CursorMove(i8),
    CaseToggle,
    Digit(char),
    PagePrev,
    PageNext,
    /// Domain / throttle events not remapped by the profile.
    PassThrough(InputEvent),
}

pub trait NavProfile {
    /// Map an input event to a canonical navigation action.
    ///
    /// `text_entry` is true on password / address / IP edit screens.
    /// `on_throttle` is true on the drive screen (no menu open).
    fn map(&self, ev: InputEvent, text_entry: bool, on_throttle: bool) -> Option<NavAction>;
}

/// LongFred standard / mini: 5-way joystick + Stop + Menu center.
#[derive(Clone, Copy, Debug, Default)]
pub struct LongFredNav;

impl NavProfile for LongFredNav {
    fn map(&self, ev: InputEvent, text_entry: bool, _on_throttle: bool) -> Option<NavAction> {
        match ev {
            InputEvent::Nav(NavDir::Up) => Some(if text_entry {
                NavAction::CharCycle(-1)
            } else {
                NavAction::ListPrev
            }),
            InputEvent::Nav(NavDir::Down) => Some(if text_entry {
                NavAction::CharCycle(1)
            } else {
                NavAction::ListNext
            }),
            InputEvent::Nav(NavDir::Left) => Some(if text_entry {
                NavAction::CursorMove(-1)
            } else {
                NavAction::PagePrev
            }),
            InputEvent::Nav(NavDir::Right) => Some(if text_entry {
                NavAction::CursorMove(1)
            } else {
                NavAction::PageNext
            }),
            InputEvent::Ok => Some(NavAction::Select),
            InputEvent::Back => Some(NavAction::Cancel),
            InputEvent::Menu => Some(if text_entry {
                NavAction::Select
            } else {
                NavAction::MenuEnter
            }),
            InputEvent::EncoderClockwise if text_entry => Some(NavAction::CharCycle(1)),
            InputEvent::EncoderCounterClockwise if text_entry => Some(NavAction::CharCycle(-1)),
            InputEvent::EncoderButton if text_entry => Some(NavAction::Select),
            InputEvent::CaseToggle => Some(NavAction::CaseToggle),
            InputEvent::Digit(c) => Some(NavAction::Digit(c)),
            InputEvent::CharCycle(d) => Some(NavAction::CharCycle(d)),
            InputEvent::CursorMove(d) => Some(NavAction::CursorMove(d)),
            other => Some(NavAction::PassThrough(other)),
        }
    }
}

/// MarkWTech: encoder + keypad (`*` / `#`) + extra tact cluster.
#[derive(Clone, Copy, Debug, Default)]
pub struct MarkwtechNav;

impl NavProfile for MarkwtechNav {
    fn map(&self, ev: InputEvent, text_entry: bool, on_throttle: bool) -> Option<NavAction> {
        match ev {
            InputEvent::EncoderCounterClockwise => Some(if text_entry {
                NavAction::CharCycle(-1)
            } else if on_throttle {
                NavAction::PassThrough(InputEvent::EncoderCounterClockwise)
            } else {
                NavAction::ListPrev
            }),
            InputEvent::EncoderClockwise => Some(if text_entry {
                NavAction::CharCycle(1)
            } else if on_throttle {
                NavAction::PassThrough(InputEvent::EncoderClockwise)
            } else {
                NavAction::ListNext
            }),
            InputEvent::Nav(NavDir::Left) => Some(if text_entry {
                NavAction::CursorMove(-1)
            } else {
                NavAction::PagePrev
            }),
            InputEvent::Nav(NavDir::Right) => Some(if text_entry {
                NavAction::CursorMove(1)
            } else {
                NavAction::PageNext
            }),
            InputEvent::Digit('#') => Some(NavAction::Select),
            InputEvent::Digit('*') => Some(if text_entry {
                NavAction::CaseToggle
            } else if on_throttle {
                NavAction::PassThrough(InputEvent::DirectionToggle)
            } else {
                NavAction::Cancel
            }),
            InputEvent::Digit(c) => Some(NavAction::Digit(c)),
            InputEvent::Menu => Some(if text_entry {
                NavAction::Select
            } else {
                NavAction::MenuEnter
            }),
            InputEvent::EncoderButton if text_entry => Some(NavAction::Select),
            InputEvent::Back => Some(NavAction::Cancel),
            InputEvent::Ok => Some(NavAction::Select),
            other => Some(NavAction::PassThrough(other)),
        }
    }
}

/// Static instances for `Router::new`.
pub static LONGFRED: LongFredNav = LongFredNav;
pub static MARKWTECH: MarkwtechNav = MarkwtechNav;
