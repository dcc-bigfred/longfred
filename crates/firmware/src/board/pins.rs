//! Physical GPIO numbers used by input, I2C, battery, and sleep.
//!
//! Re-exported from the active entry in [`crate::board::variants`].

pub use super::variants::{
    BATTERY_ADC, BATTERY_CONVERSION_FACTOR, ENCODER_A, ENCODER_B, ENCODER_BUTTON, I2C_SCL, I2C_SDA,
    WAKE_PIN,
};
