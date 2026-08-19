//! Function key to DCC function mapping and encoder options.

use crate::domain::actions::Action;

/// DCC function number for each Fn key (F0..F10).
pub const FN_TO_DCC: [u8; 11] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

pub fn fn_action(key: u8) -> Action {
    if let Some(&dcc) = FN_TO_DCC.get(key as usize) {
        Action::Function(dcc)
    } else {
        Action::None
    }
}

pub const ENCODER_BUTTON_ACTION: Action = Action::SpeedStopThenToggleDirection;
pub const TOGGLE_DIRECTION_WHEN_STATIONARY: bool = true;
pub const ENCODER_CLOCKWISE_INCREASES_SPEED: bool = false;
pub const ENCODER_INVERT_WHEN_REVERSED: bool = false;

pub const HASH_SHOWS_FUNCTIONS_INSTEAD_OF_KEY_DEFS: bool = false;

/// Default number of active throttles (max = sizes::MAX_THROTTLES).
pub const DEFAULT_THROTTLES: usize = 2;

pub const SPEED_STEP: u8 = 4;
pub const SPEED_STEP_MULTIPLIER: u8 = 3;
pub const SPEED_STEP_ADDITIONAL_MULTIPLIER: u8 = 2;

pub const DEAD_MAN_SWITCH_ENABLED: bool = true;
pub const DEFAULT_HEARTBEAT_PERIOD_S: u32 = 10;
