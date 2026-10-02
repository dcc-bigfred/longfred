//! Physical GPIO numbers used by input, I2C, battery, and sleep.
//!
//! LongFred / Heiko read [`crate::config::board`]. MarkWTech and longfred-v1
//! override encoder / I2C / battery / wake from the active pin map.

#[cfg(feature = "variant-markwtech")]
pub use crate::board::variants::markwtech::{
    BATTERY_ADC, BATTERY_CONVERSION_FACTOR, ENCODER_A, ENCODER_B, ENCODER_BUTTON, I2C_SCL, I2C_SDA,
    WAKE_PIN,
};

#[cfg(feature = "variant-longfred-v1")]
pub use crate::board::variants::longfred_v1::{
    BATTERY_ADC, BATTERY_CONVERSION_FACTOR, ENCODER_A, ENCODER_B, ENCODER_BUTTON, I2C_SCL, I2C_SDA,
    WAKE_PIN,
};

#[cfg(not(any(feature = "variant-markwtech", feature = "variant-longfred-v1")))]
pub use crate::config::board::{
    BATTERY_ADC, ENCODER_A, ENCODER_B, ENCODER_BUTTON, I2C_SCL, I2C_SDA, WAKE_PIN,
};

#[cfg(not(any(feature = "variant-markwtech", feature = "variant-longfred-v1")))]
/// ADC counts → pack millivolts (1:2 divider, WiTcontroller scale).
pub const BATTERY_CONVERSION_FACTOR: f32 = 1.7;
