//! Mapowanie klawiszy 0-9 (i przycisku enkodera) na akcje. Odpowiednik
//! CHOSEN_KEYPAD_*_FUNCTION / ENCODER_BUTTON_ACTION z config_buttons.h.

use crate::domain::actions::Action;

/// Domyślna akcja klawisza numerycznego poza menu (`*` i `#` są sterujące menu).
pub const fn default_action(key: char) -> Action {
    match key {
        '0' => Action::Function(0),
        '1' => Action::Function(1),
        '2' => Action::Function(2),
        '3' => Action::Function(3),
        '4' => Action::Function(4),
        '5' => Action::NextThrottle,
        '6' => Action::SpeedMultiplier,
        '7' => Action::DirectionReverse,
        '8' => Action::EStop,
        '9' => Action::DirectionForward,
        _ => Action::None,
    }
}

pub const ENCODER_BUTTON_ACTION: Action = Action::SpeedStopThenToggleDirection;
pub const TOGGLE_DIRECTION_WHEN_STATIONARY: bool = true;
pub const ENCODER_CLOCKWISE_INCREASES_SPEED: bool = false;
pub const ENCODER_INVERT_WHEN_REVERSED: bool = false;

pub const HASH_SHOWS_FUNCTIONS_INSTEAD_OF_KEY_DEFS: bool = false;

/// Domyślna liczba aktywnych throttli (max = sizes::MAX_THROTTLES).
pub const DEFAULT_THROTTLES: usize = 2;

pub const SPEED_STEP: u8 = 4;
pub const SPEED_STEP_MULTIPLIER: u8 = 3;
pub const SPEED_STEP_ADDITIONAL_MULTIPLIER: u8 = 2;

pub const DROP_BEFORE_ACQUIRE: bool = false;
pub const HEARTBEAT_ENABLED: bool = true;
pub const DEFAULT_HEARTBEAT_PERIOD_S: u32 = 10;
