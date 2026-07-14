//! Teksty UI (na start EN). Odpowiednik `static.h` z oryginału — tu tylko ekran startowy.

use crate::config;

/// Wersja firmware zgłaszana na ekranie startowym.
pub const FW_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Nazwa aplikacji (z config::DEVICE_NAME, compile-time).
pub const APP_NAME: &str = config::DEVICE_NAME;

pub const MSG_BOOTING: &str = "booting...";
pub const MSG_READY: &str = "ready";

pub const MSG_WIFI_DISCONNECTED: &str = "wifi: ---";
pub const MSG_WIFI_CONNECTING: &str = "wifi: ...";
pub const MSG_WIFI_CONNECTED: &str = "wifi: link";
pub const MSG_NET_READY: &str = "wifi: online";

pub const MSG_SRV_SEARCHING: &str = "srv: search";
pub const MSG_SRV_NONE: &str = "srv: none";

pub const MSG_WIT_CONNECTING: &str = "wit: ...";
pub const MSG_WIT_CONNECTED: &str = "wit: ok";
pub const MSG_WIT_DISCONNECTED: &str = "wit: off";

pub const MSG_ACQUIRE_HINT: &str = "addr+#";
