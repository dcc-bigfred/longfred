//! Akcje przypisywane klawiszom/przyciskom. Odpowiednik actions.h,
//! ale z parametryzowanymi wariantami zamiast osobnych stałych.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Nic nie rób (FUNCTION_NULL).
    None,

    /// DCC funkcja 0-31 (FUNCTION_0..FUNCTION_31).
    Function(u8),

    SpeedStop,
    SpeedUp,
    SpeedDown,
    SpeedUpFast,
    SpeedDownFast,
    SpeedMultiplier,
    /// Zatrzymaj jeśli jedzie, w przeciwnym razie zmień kierunek.
    SpeedStopThenToggleDirection,

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
    /// Przełącz na konkretny throttle 1-6 (THROTTLE_1..THROTTLE_6).
    Throttle(u8),

    /// Komenda użytkownika 1-7 (CUSTOM_1..CUSTOM_7).
    Custom(u8),
}

impl Action {
    /// Czy akcja dotyczy loco (odpowiednik "wartości < 500" w actions.h).
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
