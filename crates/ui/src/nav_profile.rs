//! Per-variant navigation profile: `InputEvent` → canonical `NavAction`.

use crate::input::{InputEvent, NavDir};
use crate::screen::InputMode;

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
    fn map(&self, ev: InputEvent, mode: InputMode) -> NavAction;
}

/// `LongFred` standard / mini: 5-way joystick + Stop + Menu center.
#[derive(Clone, Copy, Debug, Default)]
pub struct LongFredNav;

impl NavProfile for LongFredNav {
    fn map(&self, ev: InputEvent, mode: InputMode) -> NavAction {
        let text = mode == InputMode::Text;
        match ev {
            InputEvent::Nav(NavDir::Up) => {
                if text {
                    NavAction::CharCycle(-1)
                } else {
                    NavAction::ListPrev
                }
            }
            InputEvent::Nav(NavDir::Down) => {
                if text {
                    NavAction::CharCycle(1)
                } else {
                    NavAction::ListNext
                }
            }
            InputEvent::Nav(NavDir::Left) => {
                if text {
                    NavAction::CursorMove(-1)
                } else {
                    NavAction::PagePrev
                }
            }
            InputEvent::Nav(NavDir::Right) => {
                if text {
                    NavAction::CursorMove(1)
                } else {
                    NavAction::PageNext
                }
            }
            InputEvent::Ok => NavAction::Select,
            InputEvent::Back => NavAction::Cancel,
            InputEvent::Menu => {
                if text {
                    NavAction::Select
                } else {
                    NavAction::MenuEnter
                }
            }
            InputEvent::EncoderClockwise if text => NavAction::CharCycle(1),
            InputEvent::EncoderCounterClockwise if text => NavAction::CharCycle(-1),
            InputEvent::EncoderButton if text => NavAction::Select,
            InputEvent::CaseToggle => NavAction::CaseToggle,
            InputEvent::Digit(c) => NavAction::Digit(c),
            InputEvent::CharCycle(d) => NavAction::CharCycle(d),
            InputEvent::CursorMove(d) => NavAction::CursorMove(d),
            other => NavAction::PassThrough(other),
        }
    }
}

/// `MarkWTech`: encoder + keypad (`*` / `#`) + extra tact cluster.
#[derive(Clone, Copy, Debug, Default)]
pub struct MarkwtechNav;

impl NavProfile for MarkwtechNav {
    fn map(&self, ev: InputEvent, mode: InputMode) -> NavAction {
        match ev {
            InputEvent::EncoderCounterClockwise => match mode {
                InputMode::Text => NavAction::CharCycle(-1),
                InputMode::Throttle => NavAction::PassThrough(InputEvent::EncoderCounterClockwise),
                InputMode::Navigation => NavAction::ListPrev,
            },
            InputEvent::EncoderClockwise => match mode {
                InputMode::Text => NavAction::CharCycle(1),
                InputMode::Throttle => NavAction::PassThrough(InputEvent::EncoderClockwise),
                InputMode::Navigation => NavAction::ListNext,
            },
            InputEvent::Nav(NavDir::Left) => {
                if mode == InputMode::Text {
                    NavAction::CursorMove(-1)
                } else {
                    NavAction::PagePrev
                }
            }
            InputEvent::Nav(NavDir::Right) => {
                if mode == InputMode::Text {
                    NavAction::CursorMove(1)
                } else {
                    NavAction::PageNext
                }
            }
            InputEvent::Digit('#') | InputEvent::Ok => NavAction::Select,
            InputEvent::Digit('*') => match mode {
                InputMode::Text => NavAction::CaseToggle,
                InputMode::Throttle => NavAction::PassThrough(InputEvent::DirectionToggle),
                InputMode::Navigation => NavAction::Cancel,
            },
            InputEvent::Digit(c) => NavAction::Digit(c),
            InputEvent::Menu => {
                if mode == InputMode::Text {
                    NavAction::Select
                } else {
                    NavAction::MenuEnter
                }
            }
            InputEvent::EncoderButton if mode == InputMode::Text => NavAction::Select,
            InputEvent::Back => NavAction::Cancel,
            other => NavAction::PassThrough(other),
        }
    }
}

/// Static instances for `Router::new`.
pub static LONGFRED: LongFredNav = LongFredNav;
pub static MARKWTECH: MarkwtechNav = MarkwtechNav;
