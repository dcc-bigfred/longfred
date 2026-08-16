//! Actions assigned to keys/buttons (shared by domain reducer and UI intents).

/// Command issued by a key, menu item, or screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Do nothing (FUNCTION_NULL).
    None,

    /// DCC function 0-31 (FUNCTION_0..FUNCTION_31).
    Function(u8),

    SpeedStop,
    SpeedUp,
    SpeedDown,
    SpeedUpFast,
    SpeedDownFast,
    SpeedMultiplier,
    /// Stop if moving, otherwise toggle direction.
    SpeedStopThenToggleDirection,

    /// Set throttle speed 0..=126 from an analog pot.
    SpeedSet(u8),

    EStop,
    EStopCurrentLoco,

    DirectionToggle,
    DirectionForward,
    DirectionReverse,

    MaxThrottleIncrease,
    MaxThrottleDecrease,

    PowerToggle,
    PowerOn,
    PowerOff,

    ShowHideBattery,
    Sleep,

    NextThrottle,
    /// Switch to throttle 1-6 (THROTTLE_1..THROTTLE_6).
    Throttle(u8),

    /// User command 1-7 (CUSTOM_1..CUSTOM_7).
    Custom(u8),
}

impl Action {
    /// Whether the action affects a loco (equivalent of "value < 500" in actions.h).
    pub const fn is_loco_action(self) -> bool {
        !matches!(
            self,
            Action::None
                | Action::PowerToggle
                | Action::PowerOn
                | Action::PowerOff
                | Action::ShowHideBattery
                | Action::Sleep
                | Action::NextThrottle
                | Action::Throttle(_)
                | Action::Custom(_)
        )
    }
}
