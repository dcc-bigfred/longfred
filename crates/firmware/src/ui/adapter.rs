//! Bridge: firmware watches / config → `longfred-ui` context.

use longfred_proto::model::{MAX_FOUND_SERVERS, MAX_FOUND_SSIDS};
use longfred_proto::network::SsidInfo;
use longfred_proto::network::WitServer;
use longfred_ui::i18n::{HintSet, Strings, strings};
use longfred_ui::nav::ScreenId;
use longfred_ui::nav_profile::NavProfile;
use longfred_ui::{
    BatteryInfo, BatteryMode, DriveInfo, NetInfo, Router, ScreenCtx, UiEnv, UiSession,
};

#[cfg(not(feature = "variant-markwtech"))]
use longfred_ui::nav_profile::LONGFRED;
#[cfg(feature = "variant-markwtech")]
use longfred_ui::nav_profile::MARKWTECH;

use crate::config::{buttons, network, power};
use crate::domain::state::DomainState;
use crate::input::{self, InputEvent as FwInput};
use crate::net::{self, ConnState, NetStatus, ServerEndpoint};
use crate::power::battery::BatterySample;
use crate::ui::i18n;
use crate::ui::view::UiView;

pub fn nav_profile() -> &'static dyn NavProfile {
    #[cfg(feature = "variant-markwtech")]
    {
        &MARKWTECH
    }
    #[cfg(not(feature = "variant-markwtech"))]
    {
        &LONGFRED
    }
}

pub fn hint_set() -> HintSet {
    if crate::board::active_variant().has_keypad {
        HintSet::Keypad
    } else {
        HintSet::Joystick
    }
}

pub fn ui_env() -> UiEnv {
    let board = crate::board::active_variant();
    let geometry = board.display.map_or(longfred_ui::LAYOUT_128X64, |d| {
        longfred_ui::DisplayGeometry {
            width: d.width,
            height: d.height,
            grid_rows: d.grid_rows,
            grid_cols: d.grid_cols,
            grid_lines: d.grid_lines,
        }
    });
    UiEnv {
        geometry,
        has_keypad: board.has_keypad,
        hint_set: hint_set(),
        app_name: i18n::APP_NAME,
        fw_version: i18n::FW_VERSION,
        fn_to_dcc: buttons::FN_TO_DCC,
        hash_shows_functions: buttons::HASH_SHOWS_FUNCTIONS_INSTEAD_OF_KEY_DEFS,
        compiled_networks: network::NETWORKS,
        default_wit_ip: network::DEFAULT_WIT_IP,
        default_wit_port: network::DEFAULT_WIT_PORT,
        default_z21_ip: network::DEFAULT_Z21_IP,
        default_z21_port: network::DEFAULT_Z21_PORT,
        default_prefix_len: network::DEFAULT_PREFIX_LEN,
        board_id: board.id,
        board_mcu: board.mcu,
        battery_factor: power::BATTERY_CONVERSION_FACTOR,
    }
}

pub fn init_session() -> UiSession {
    let mut session = UiSession::new();
    session.hash_functions = buttons::HASH_SHOWS_FUNCTIONS_INSTEAD_OF_KEY_DEFS;
    session.battery_mode = if power::USE_BATTERY_TEST {
        if power::USE_BATTERY_PERCENT_WITH_ICON {
            BatteryMode::IconPercent
        } else {
            BatteryMode::Icon
        }
    } else {
        BatteryMode::None
    };
    session
}

pub fn map_input(ev: FwInput) -> longfred_ui::InputEvent {
    match ev {
        FwInput::Nav(d) => longfred_ui::InputEvent::Nav(match d {
            input::NavDir::Up => longfred_ui::NavDir::Up,
            input::NavDir::Down => longfred_ui::NavDir::Down,
            input::NavDir::Left => longfred_ui::NavDir::Left,
            input::NavDir::Right => longfred_ui::NavDir::Right,
        }),
        FwInput::Ok => longfred_ui::InputEvent::Ok,
        FwInput::Back => longfred_ui::InputEvent::Back,
        FwInput::Menu => longfred_ui::InputEvent::Menu,
        FwInput::EStop => longfred_ui::InputEvent::EStop,
        FwInput::Stop => longfred_ui::InputEvent::Stop,
        FwInput::FnPress(k) => longfred_ui::InputEvent::FnPress(k),
        FwInput::FnRelease(k) => longfred_ui::InputEvent::FnRelease(k),
        FwInput::DirectionSet(dir) => longfred_ui::InputEvent::DirectionSet(dir),
        FwInput::DirectionToggle => longfred_ui::InputEvent::DirectionToggle,
        FwInput::EncoderClockwise => longfred_ui::InputEvent::EncoderClockwise,
        FwInput::EncoderCounterClockwise => longfred_ui::InputEvent::EncoderCounterClockwise,
        FwInput::EncoderButton => longfred_ui::InputEvent::EncoderButton,
        FwInput::Digit(c) => longfred_ui::InputEvent::Digit(c),
        FwInput::SpeedAbsolute(v) => longfred_ui::InputEvent::SpeedAbsolute(v),
        FwInput::LocoSlot(s, on) => longfred_ui::InputEvent::LocoSlot(s, on),
        FwInput::CharCycle(d) => longfred_ui::InputEvent::CharCycle(d),
        FwInput::CursorMove(d) => longfred_ui::InputEvent::CursorMove(d),
        FwInput::CaseToggle => longfred_ui::InputEvent::CaseToggle,
        FwInput::EnterProgrammingMode => longfred_ui::InputEvent::EnterProgrammingMode,
    }
}

pub fn strings_for(state: &DomainState) -> &'static Strings {
    strings(state.persist.language, hint_set())
}

/// Domain state, UI session, and live net snapshots used to build [`ScreenCtx`].
pub struct UiWorld {
    pub state: DomainState,
    pub session: UiSession,
    pub env: UiEnv,
    pub router: Router,
    pub net_status: NetStatus,
    pub conn: ConnState,
    pub server: Option<ServerEndpoint>,
    pub scanned: heapless::Vec<SsidInfo, MAX_FOUND_SSIDS>,
    pub servers: heapless::Vec<WitServer, MAX_FOUND_SERVERS>,
    pub battery: Option<BatterySample>,
}

impl UiWorld {
    pub fn new() -> Self {
        Self {
            state: DomainState::new(),
            session: init_session(),
            env: ui_env(),
            router: Router::new(nav_profile(), ScreenId::Splash),
            net_status: NetStatus::Disconnected,
            conn: ConnState::Disconnected,
            server: None,
            scanned: heapless::Vec::new(),
            servers: heapless::Vec::new(),
            battery: None,
        }
    }

    /// Run `f` with a [`ScreenCtx`] and a disjoint borrow of the router.
    pub fn with_ctx<R>(
        &mut self,
        now_ms: u64,
        f: impl FnOnce(&mut Router, &mut ScreenCtx<'_>) -> R,
    ) -> R {
        let strings = strings_for(&self.state);
        let mut cx = screen_ctx(
            &self.state,
            &mut self.session,
            &self.env,
            strings,
            self.net_status,
            self.conn,
            self.server,
            &self.scanned,
            &self.servers,
            self.battery,
            now_ms,
        );
        f(&mut self.router, &mut cx)
    }

    pub fn publish_view(
        &mut self,
        now_ms: u64,
        ui_tx: &embassy_sync::watch::Sender<
            'static,
            embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
            UiView,
            2,
        >,
    ) {
        publish(
            &self.router,
            &self.state,
            &mut self.session,
            &self.env,
            strings_for(&self.state),
            self.net_status,
            self.conn,
            self.server,
            &self.scanned,
            &self.servers,
            self.battery,
            now_ms,
            ui_tx,
        );
    }
}

pub fn publish(
    router: &Router,
    state: &DomainState,
    session: &mut UiSession,
    env: &UiEnv,
    s: &Strings,
    net_status: NetStatus,
    conn: ConnState,
    server: Option<ServerEndpoint>,
    scanned: &heapless::Vec<SsidInfo, MAX_FOUND_SSIDS>,
    servers: &heapless::Vec<WitServer, MAX_FOUND_SERVERS>,
    battery: Option<BatterySample>,
    now_ms: u64,
    ui_tx: &embassy_sync::watch::Sender<
        'static,
        embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
        UiView,
        2,
    >,
) {
    let cx = screen_ctx(
        state, session, env, s, net_status, conn, server, scanned, servers, battery, now_ms,
    );
    let view = router.view(&cx);
    if ui_tx.try_get().as_ref() != Some(&view) {
        ui_tx.send(view);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn screen_ctx<'a>(
    state: &'a DomainState,
    session: &'a mut UiSession,
    env: &'a UiEnv,
    s: &'a Strings,
    net_status: NetStatus,
    conn: ConnState,
    server: Option<ServerEndpoint>,
    scanned: &'a heapless::Vec<SsidInfo, MAX_FOUND_SSIDS>,
    servers: &'a heapless::Vec<WitServer, MAX_FOUND_SERVERS>,
    battery: Option<BatterySample>,
    now_ms: u64,
) -> ScreenCtx<'a> {
    ScreenCtx {
        drive: DriveInfo {
            slots: state.throttles.as_slice(),
            current: state.current,
            roster: state.roster.as_slice(),
            effective_loco_source: state.effective_loco_source,
            track_power: state.track_power,
            persist: &state.persist,
            message: state.active_broadcast(),
            speed_multiplier: state.speed_multiplier,
            heartbeat_on: state.heartbeat_on,
            drop_before_acquire: state.drop_before_acquire,
        },
        net: NetInfo {
            status: net_status,
            conn,
            server,
            scanned_ssids: scanned,
            found_servers: servers,
            wifi_link: net::WIFI_LINK.try_get().flatten(),
            sta_net: net::STA_NET.try_get().flatten(),
            ping: net::PING.try_get().unwrap_or(net::PingStatus::Idle),
            sta_ipv4: net::sta_ipv4(),
            http_ota: net::http_ota_enabled(),
            http_ota_busy: net::http_ota_busy(),
        },
        env,
        s,
        now_ms,
        battery: battery.map(|b| BatteryInfo {
            percent: b.percent,
            millivolts: b.millivolts,
            raw: b.raw,
        }),
        session,
    }
}

/// Auto-connect with last SSID, or open a live scan.
pub fn begin_wifi_setup(
    router: &mut Router,
    cx: &mut ScreenCtx<'_>,
    last_ssid: Option<&str>,
) -> heapless::Vec<longfred_ui::Intent, 4> {
    cx.session.splash_done = true;
    cx.session.boot_language = false;
    if let Some(ssid) = last_ssid {
        cx.session.selected_ssid.clear();
        let _ = cx.session.selected_ssid.push_str(ssid);
        cx.session.selected_from_scan = false;
        let stored = {
            let s = cx.drive.persist.find_password(ssid).unwrap_or("");
            let mut buf = heapless::String::<64>::new();
            let _ = buf.push_str(s);
            buf
        };
        cx.session.password.clear();
        let _ = cx.session.password.push_str(stored.as_str());
        let mut intents = router.replace_screen(ScreenId::Connecting, cx);
        let _ = intents.push(longfred_ui::Intent::WifiConnect);
        intents
    } else {
        router.replace_screen(ScreenId::SsidScanning, cx)
    }
}

pub fn take_pending_password_save(
    session: &mut UiSession,
) -> Option<(heapless::String<32>, heapless::String<64>)> {
    if session.pending_password_save {
        session.pending_password_save = false;
        Some((session.selected_ssid.clone(), session.password.clone()))
    } else {
        None
    }
}
