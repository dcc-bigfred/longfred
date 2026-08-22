//! Shared blocking I2C bus for OLED and MCP23017 expanders.
//!
//! Blocking (not async) on purpose. SoftwareTimeout still bounds NACKs
//! so a missing MCP23017 cannot stall the bus.

use core::cell::RefCell;

use embassy_embedded_hal::shared_bus::blocking::i2c::I2cDevice;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use esp_hal::Blocking;
use esp_hal::gpio::AnyPin;
use esp_hal::i2c::master::{Config, I2c, Instance, SoftwareTimeout};
use esp_hal::time::{Duration, Rate};
use static_cell::StaticCell;

use crate::config::board;

pub type SharedI2cBus = Mutex<CriticalSectionRawMutex, RefCell<I2c<'static, Blocking>>>;
pub type SharedI2cDevice = I2cDevice<'static, CriticalSectionRawMutex, I2c<'static, Blocking>>;

static I2C_BUS: StaticCell<SharedI2cBus> = StaticCell::new();

/// Builds blocking I2C on `I2C0`, stores it in a static mutex, returns two device handles.
///
/// # Safety
///
/// Steals `I2C_SDA` / `I2C_SCL`. Call once from `main` before OLED/MCP tasks start.
#[allow(clippy::unwrap_used, unsafe_code)]
pub fn init(i2c: impl Instance + 'static) -> (SharedI2cDevice, SharedI2cDevice) {
    let cfg = Config::default()
        .with_frequency(Rate::from_khz(board::I2C_FREQ_KHZ))
        .with_software_timeout(SoftwareTimeout::Transaction(Duration::from_millis(50)));
    let bus = I2c::new(i2c, cfg)
        .unwrap()
        // SAFETY: I2C pins are reserved for this shared bus; single init from `main`.
        .with_sda(unsafe { AnyPin::steal(crate::board::pins::I2C_SDA) })
        .with_scl(unsafe { AnyPin::steal(crate::board::pins::I2C_SCL) });

    let mutex = I2C_BUS.init(Mutex::new(RefCell::new(bus)));
    let oled = I2cDevice::new(mutex);
    let expander = I2cDevice::new(mutex);
    (oled, expander)
}
