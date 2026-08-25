//! Physical GPIO numbers used by input, I2C, battery, and sleep.
//!
//! LongFred / Heiko read [`crate::config::board`]. MarkWTech overrides encoder,
//! I2C, battery, and wake from the active revision pin map.

#[cfg(feature = "variant-markwtech")]
pub use crate::board::variants::markwtech::{
    BATTERY_ADC, BATTERY_CONVERSION_FACTOR, ENCODER_A, ENCODER_B, ENCODER_BUTTON, I2C_SCL, I2C_SDA,
    WAKE_PIN,
};

#[cfg(not(feature = "variant-markwtech"))]
pub use crate::config::board::{
    BATTERY_ADC, ENCODER_A, ENCODER_B, ENCODER_BUTTON, I2C_SCL, I2C_SDA, WAKE_PIN,
};

#[cfg(not(feature = "variant-markwtech"))]
pub const BATTERY_CONVERSION_FACTOR: f32 = 1.7;
