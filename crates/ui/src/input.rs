//! Canonical input events (hardware-agnostic).

use longfred_proto::model::Direction;

/// Joystick / extra-button navigation direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavDir {
    Up,
    Down,
    Left,
    Right,
}

/// Input event emitted to the UI router.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    Nav(NavDir),
    Ok,
    Back,
    Menu,
    EStop,
    /// Physical Stop — screens map to `EStop` or Cancel.
    Stop,
    FnPress(u8),
    FnRelease(u8),
    DirectionSet(Direction),
    DirectionToggle,
    EncoderClockwise,
    EncoderCounterClockwise,
    EncoderButton,
    Digit(char),
    SpeedAbsolute(u8),
    LocoSlot(u8, bool),
    CharCycle(i8),
    CursorMove(i8),
    CaseToggle,
    EnterProgrammingMode,
}
