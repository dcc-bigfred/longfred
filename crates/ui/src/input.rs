//! Canonical input events (hardware-agnostic).

use longfred_proto::model::Direction;

/// Joystick / extra-button navigation direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavDir {
    /// Up / previous list row.
    Up,
    /// Down / next list row.
    Down,
    /// Left / previous page (or cursor back in text mode).
    Left,
    /// Right / next page (or cursor forward in text mode).
    Right,
}

/// Input event emitted to the UI router.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    /// 5-way / extra-button direction.
    Nav(NavDir),
    /// Confirm / select.
    Ok,
    /// Cancel / back.
    Back,
    /// Open the menu (or confirm in text mode).
    Menu,
    /// Emergency stop.
    EStop,
    /// Physical Stop — screens map to `EStop` or Cancel.
    Stop,
    /// Function key down (`0..=10` on `LongFred`).
    FnPress(u8),
    /// Function key up.
    FnRelease(u8),
    /// Set loco direction.
    DirectionSet(Direction),
    /// Toggle loco direction.
    DirectionToggle,
    /// Encoder clockwise.
    EncoderClockwise,
    /// Encoder counter-clockwise.
    EncoderCounterClockwise,
    /// Encoder push.
    EncoderButton,
    /// Keypad digit or `*` / `#`.
    Digit(char),
    /// Absolute speed from a pot / slider (`0..=126`).
    SpeedAbsolute(u8),
    /// Throttle slot key (`slot`, `down`).
    LocoSlot(u8, bool),
    /// Cycle the character under the text caret.
    CharCycle(i8),
    /// Move the text caret.
    CursorMove(i8),
    /// Toggle text-entry case.
    CaseToggle,
    /// Request DCC programming mode.
    EnterProgrammingMode,
}
