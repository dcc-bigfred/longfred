//! Active MarkWTech GPIO map (v1.0 DevKitC-1 or v1.1 TinyC6).

#[cfg(not(feature = "variant-markwtech-v1-1"))]
pub use super::pins_v1_0::*;
#[cfg(feature = "variant-markwtech-v1-1")]
pub use super::pins_v1_1::*;

const fn pins_unique(pins: &[u8]) -> bool {
    let mut i = 0;
    while i < pins.len() {
        let mut j = i + 1;
        while j < pins.len() {
            if pins[i] == pins[j] {
                return false;
            }
            j += 1;
        }
        i += 1;
    }
    true
}

const USED: [u8; 18] = [
    KEYPAD_ROW_PINS[0],
    KEYPAD_ROW_PINS[1],
    KEYPAD_ROW_PINS[2],
    KEYPAD_ROW_PINS[3],
    KEYPAD_COL_PINS[0],
    KEYPAD_COL_PINS[1],
    KEYPAD_COL_PINS[2],
    EXTRA_BUTTON_PINS[0],
    EXTRA_BUTTON_PINS[1],
    EXTRA_BUTTON_PINS[2],
    EXTRA_BUTTON_PINS[3],
    EXTRA_BUTTON_PINS[4],
    ENCODER_A,
    ENCODER_B,
    ENCODER_BUTTON,
    I2C_SDA,
    I2C_SCL,
    BATTERY_ADC,
];

const _: () = assert!(
    pins_unique(&USED),
    "duplicate GPIO in the active MarkWTech pin map"
);
const _: () = assert!(
    WAKE_PIN == ENCODER_BUTTON,
    "deep-sleep wake must be the encoder SW pin"
);
