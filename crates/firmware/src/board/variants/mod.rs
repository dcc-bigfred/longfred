//! Hardware variant selection (compile-time features).
//!
//! Cargo features `variant-*` are named only in this module. The rest of the
//! firmware reads [`ACTIVE`] or calls [`surface`] / [`spawn_inputs`].

#[cfg(all(feature = "variant-longfred-v1", feature = "variant-markwtech"))]
compile_error!("enable only one hardware variant feature");

#[cfg(not(any(feature = "variant-longfred-v1", feature = "variant-markwtech")))]
compile_error!("select a hardware variant feature");

#[cfg(feature = "variant-longfred-v1")]
pub mod longfred_family;

#[cfg(feature = "variant-longfred-v1")]
pub mod longfred_v1;

#[cfg(feature = "variant-markwtech")]
pub mod markwtech;

// Driver sources stay under `input/`. They are declared here so the feature
// cfg that compiles them in does not appear outside this module.
#[cfg(feature = "variant-longfred-v1")]
#[path = "../../input/expander.rs"]
mod expander;

#[cfg(feature = "variant-markwtech")]
#[path = "../../input/keypad.rs"]
mod keypad;

#[cfg(feature = "variant-markwtech")]
#[path = "../../input/extra_buttons.rs"]
mod extra_buttons;

use embassy_executor::Spawner;

use crate::board::descriptor::VariantDescriptor;
use crate::board::raw::RawSender;
use crate::input::i2c_bus::SharedI2cDevice;

#[cfg(feature = "variant-longfred-v1")]
pub use longfred_v1::{
    BATTERY_ADC, BATTERY_CONVERSION_FACTOR, ENCODER_A, ENCODER_B, ENCODER_BUTTON, I2C_SCL, I2C_SDA,
    WAKE_PIN,
};

#[cfg(feature = "variant-markwtech")]
pub use markwtech::{
    BATTERY_ADC, BATTERY_CONVERSION_FACTOR, ENCODER_A, ENCODER_B, ENCODER_BUTTON, I2C_SCL, I2C_SDA,
    WAKE_PIN,
};

/// Descriptor of the board selected by the Cargo feature.
#[cfg(feature = "variant-longfred-v1")]
pub const ACTIVE: VariantDescriptor = longfred_family::V1;

/// Descriptor of the board selected by the Cargo feature.
#[cfg(feature = "variant-markwtech")]
pub const ACTIVE: VariantDescriptor = markwtech::DESCRIPTOR;

/// SSD1306 framebuffer size for the active board.
///
/// A type alias, not a runtime flag: the `ssd1306` buffer is sized by this type.
#[cfg(feature = "variant-longfred-v1")]
pub type PanelSize = ssd1306::size::DisplaySize128x32;

/// SSD1306 framebuffer size for the active board.
///
/// A type alias, not a runtime flag: the `ssd1306` buffer is sized by this type.
#[cfg(feature = "variant-markwtech")]
pub type PanelSize = ssd1306::size::DisplaySize128x64;

/// Active build variant descriptor.
#[must_use]
pub const fn active() -> &'static VariantDescriptor {
    &ACTIVE
}

/// Alias for [`active`].
#[must_use]
pub const fn active_variant() -> &'static VariantDescriptor {
    active()
}

/// Control surface for the active board.
#[cfg(feature = "variant-longfred-v1")]
#[must_use]
pub const fn surface() -> longfred_family::LongFredSurface {
    longfred_family::LongFredSurface::v1()
}

/// Control surface for the active board.
#[cfg(feature = "variant-markwtech")]
#[must_use]
pub const fn surface() -> markwtech::MarkwtechSurface {
    markwtech::MarkwtechSurface::new()
}

/// Spawn the input drivers compiled for this board.
///
/// Keypad, extra buttons, and MCP expanders are embassy tasks, so the drivers
/// this board does not have are not linked. The encoder is common to both boards.
///
/// `expander_i2c` is moved into the MCP task when that driver is compiled in.
/// Boards without an expander still receive the handle so `main` has one call.
#[cfg_attr(feature = "variant-markwtech", allow(clippy::needless_pass_by_value))]
pub fn spawn_inputs(spawner: &Spawner, expander_i2c: SharedI2cDevice, raw_sender: RawSender) {
    #[cfg(feature = "variant-markwtech")]
    {
        let keypad_pins = keypad::build();
        crate::spawn_or_reset!(spawner, keypad::task(keypad_pins, raw_sender), "keypad");
        let extras = extra_buttons::build();
        crate::spawn_or_reset!(
            spawner,
            extra_buttons::task(extras, raw_sender),
            "extra-buttons"
        );
        let _ = expander_i2c;
    }

    #[cfg(feature = "variant-longfred-v1")]
    crate::spawn_or_reset!(
        spawner,
        expander::task(expander_i2c, raw_sender),
        "expander"
    );

    let enc = crate::input::encoder::build();
    crate::spawn_or_reset!(
        spawner,
        crate::input::encoder::task(enc.a, enc.b, raw_sender),
        "encoder"
    );
    crate::spawn_or_reset!(
        spawner,
        crate::input::encoder::button_task(enc.button, raw_sender),
        "encoder-btn"
    );
}
