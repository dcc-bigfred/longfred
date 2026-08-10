//! Hardware abstraction: raw events, variant descriptors, ControlSurface.

pub mod bridge;
pub mod chord;
pub mod descriptor;
pub mod raw;
pub mod shift_layers;
pub mod variants;

pub use descriptor::{DisplayGeometry, LAYOUT_128X32, LAYOUT_128X64, VariantDescriptor};
pub use raw::{AnalogId, ButtonId, RAW_CHANNEL, RawEvent, SwitchId};
pub use variants::{active, active_variant};

use embassy_time::Instant;

use crate::input::InputEvent;
use crate::ui::view::UiView;

use self::descriptor::VariantDescriptor as VD;
use self::raw::RawEvent as RE;

/// Maps raw hardware events to domain input events.
pub trait ControlSurface {
    fn descriptor(&self) -> &'static VD;
    fn on_raw(&mut self, ev: RE, now: Instant, out: &mut dyn FnMut(InputEvent));
    fn tick(&mut self, now: Instant, out: &mut dyn FnMut(InputEvent));
}

/// Higher-level UI / menu shell over mapped input.
pub trait UiShell {
    fn on_input(&mut self, ev: InputEvent, now: Instant);
    fn tick(&mut self, now: Instant);
}

/// Renders a [`UiView`] to the physical display.
pub trait Presenter {
    fn present(&mut self, view: &UiView);
}
