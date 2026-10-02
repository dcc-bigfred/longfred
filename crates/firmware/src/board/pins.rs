//! Physical GPIO numbers used by input, I2C, battery, and sleep.
//!
//! LongFred v1 and MarkWTech v1.1 each export their own map.

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
