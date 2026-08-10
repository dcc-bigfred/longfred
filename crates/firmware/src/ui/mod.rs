//! UI layer: OLED driver, fonts, strings (i18n), view and menu (stage 9).

pub mod display;
pub mod fonts;
pub mod headless_shell;
pub mod i18n;
pub mod keyboard;
#[cfg(feature = "variant-heiko-wifred")]
pub mod led_presenter;
pub mod menu;
pub mod nav_profile;
pub mod view;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Watch;

use crate::ui::view::UiView;

/// Current OLED view (domain + menu FSM → renderer).
pub static UI_VIEW: Watch<CriticalSectionRawMutex, UiView, 2> = Watch::new();
