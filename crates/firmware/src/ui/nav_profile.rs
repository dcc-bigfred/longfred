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
                NavAction::PassThrough(InputEvent::Nav(NavDir::Left))
            }),
            InputEvent::Nav(NavDir::Right) => Some(if text_entry {
                NavAction::CursorMove(1)
            } else {
                NavAction::PassThrough(InputEvent::Nav(NavDir::Right))
            }),
            InputEvent::Ok => Some(NavAction::Select),
            InputEvent::Back => Some(NavAction::Cancel),
            InputEvent::Menu => Some(NavAction::MenuEnter),
            InputEvent::CaseToggle => Some(NavAction::CaseToggle),
            InputEvent::Digit(c) => Some(NavAction::Digit(c)),
            InputEvent::CharCycle(d) => Some(NavAction::CharCycle(d)),
            InputEvent::CursorMove(d) => Some(NavAction::CursorMove(d)),
            // Stop / EStop / encoder / Fn / direction stay domain-visible.
            other => Some(NavAction::PassThrough(other)),
        }
    }
}

/// MarkWTech: encoder + keypad (`*` / `#`) + extra tact cluster.
///
/// - Encoder → ListPrev/ListNext (or CharCycle in text entry)
/// - `#` → Select
/// - `*` → MenuEnter on throttle; Cancel / backspace in menus & text
/// - Digits 0–9 → Digit
/// - Extra Back → Cancel
/// - Extra Menu → MenuEnter
/// - Extra Left/Right → CursorMove in text entry; otherwise list paging
#[derive(Clone, Copy, Debug, Default)]
pub struct MarkwtechNav;

impl NavProfile for MarkwtechNav {
    fn map(&self, ev: InputEvent, text_entry: bool, on_throttle: bool) -> Option<NavAction> {
        match ev {
            InputEvent::EncoderCounterClockwise => Some(if text_entry {
                NavAction::CharCycle(-1)
            } else {
                NavAction::ListPrev
            }),
            InputEvent::EncoderClockwise => Some(if text_entry {
                NavAction::CharCycle(1)
            } else {
                NavAction::ListNext
            }),
            InputEvent::Nav(NavDir::Left) => Some(if text_entry {
                NavAction::CursorMove(-1)
            } else {
                NavAction::PassThrough(InputEvent::Nav(NavDir::Left))
            }),
            InputEvent::Nav(NavDir::Right) => Some(if text_entry {
                NavAction::CursorMove(1)
            } else {
                NavAction::PassThrough(InputEvent::Nav(NavDir::Right))
            }),
            InputEvent::Digit('#') => Some(NavAction::Select),
            InputEvent::Digit('*') => Some(if text_entry {
                NavAction::Cancel
            } else if on_throttle {
                // On the drive screen, `*` opens the menu.
                NavAction::MenuEnter
            } else {
                // Inside a menu, `*` acts as Cancel / back.
                NavAction::Cancel
            }),
            InputEvent::Digit(c) => Some(NavAction::Digit(c)),
            InputEvent::Menu => Some(NavAction::MenuEnter),
            InputEvent::Back => Some(NavAction::Cancel),
            InputEvent::Ok => Some(NavAction::Select),
            other => Some(NavAction::PassThrough(other)),
        }
    }
}

/// Active nav profile for this build (type differs per variant feature).
#[cfg(any(
    feature = "variant-longfred-standard",
    feature = "variant-longfred-mini"
))]
pub fn active() -> LongFredNav {
    LongFredNav
}

#[cfg(feature = "variant-markwtech")]
pub fn active() -> MarkwtechNav {
    MarkwtechNav
}

#[cfg(feature = "variant-heiko-wifred")]
pub fn active() -> LongFredNav {
    // Heiko-wifred is headless: its ControlSurface never emits Nav/Ok/Menu
    // events, so the profile is effectively dead code. We still return a
    // LongFredNav so `domain::task` (which always constructs a MenuFsm)
    // compiles uniformly across variants.
    LongFredNav
}
