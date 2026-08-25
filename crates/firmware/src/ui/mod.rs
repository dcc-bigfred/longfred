//! UI layer: OLED driver, fonts, strings, adapter to `longfred-ui`.

pub mod adapter;
pub mod display;
pub mod fonts;
pub mod i18n;
#[cfg(feature = "variant-heiko-wifred")]
pub mod led_presenter;
pub mod splash;
pub mod view;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Watch;

use crate::ui::view::UiView;

/// Current OLED view (domain router → renderer).
pub static UI_VIEW: Watch<CriticalSectionRawMutex, UiView, 2> = Watch::new();

/// OLED panel power (domain / sleep → display task). `true` = panel on.
pub static DISPLAY_ON: Watch<CriticalSectionRawMutex, bool, 2> = Watch::new_with(true);
