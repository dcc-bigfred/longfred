//! UI layer: OLED driver, fonts, strings (i18n), view and menu (stage 9).

pub mod display;
pub mod fonts;
pub mod i18n;
pub mod keyboard;
pub mod menu;
pub mod view;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Watch;

use crate::ui::view::UiView;

/// Current OLED view (domain + menu FSM → renderer).
pub static UI_VIEW: Watch<CriticalSectionRawMutex, UiView, 2> = Watch::new();
