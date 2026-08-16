#![cfg_attr(not(test), no_std)]
//! Host-testable LongFred UI: router, screens, view model (no HAL).
//!
//! Public item docs are filled incrementally; CI clippy allows `missing_docs` for the
//! same reason. Prefer documenting new public API when adding it.
#![allow(missing_docs)]
#![forbid(unsafe_code)]

pub mod context;
pub mod geometry;
pub mod i18n;
pub mod input;
pub mod intent;
pub mod nav;
pub mod nav_profile;
pub mod router;
pub mod screen;
pub mod screens;
pub mod session;
pub mod view;
pub mod widgets;

pub use context::{BatteryInfo, CompiledNetwork, DriveInfo, NetInfo, ScreenCtx, UiEnv};
pub use geometry::{DisplayGeometry, LAYOUT_128X32, LAYOUT_128X64};
pub use i18n::{HintSet, Strings, strings};
pub use input::{InputEvent, NavDir};
pub use intent::{AppEvent, Intent};
pub use nav::{Nav, PageDir, ScreenId, Step};
pub use nav_profile::{LongFredNav, MarkwtechNav, NavAction, NavProfile};
pub use router::Router;
pub use screen::{InputMode, KeyBindings, Screen};
pub use session::{BatteryMode, NetField, UiSession};
pub use view::{GridView, ThrottleView, UiView};
