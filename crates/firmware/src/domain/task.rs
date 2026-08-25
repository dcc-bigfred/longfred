//! Domain task: screen router + state + network + UI_VIEW publication.

use embassy_futures::select::{Either3, select3};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Watch;
use embassy_time::{Duration, Instant, Timer};
use heapless::String;
use log::{error, info, warn};
use longfred_proto::command::ClientCommand;
use longfred_proto::menu::parse_ip_endpoint;
use longfred_proto::model::Direction;
use longfred_proto::persist::{PersistRecord, SavedServer};
use longfred_ui::nav::{PageDir, ScreenId};
use longfred_ui::view::push_battery_sample;
use longfred_ui::{AppEvent, Intent, UiSession};

use crate::config::{self, power, sizes};
use crate::domain::actions::Action;
use crate::domain::state::{CMD_BUF, DomainState};
use crate::input;
use crate::net::pairing_http::{
    HandsetHttpOp, PAIRING_HTTP_CTRL, PAIRING_HTTP_RESULT, PairingHttpRequest, PairingHttpResult,
};
use crate::net::{
    self, CONN, ConnState, DEVICE, FOUND_SERVERS, MDNS_CTRL, NET_CONFIG_CTRL, NetStatus,
    PROTO_COMMANDS, PROTO_EVENTS, SERVER, STATE, ServerEndpoint, WIFI_CTRL, WIFI_HOSTNAME,
    WIFI_SCAN, WifiCmd,
};
use crate::power::battery::BATTERY;
use crate::power::sleep::{self, SleepReason};
use crate::storage::{PERSIST_LOADED, STORAGE_ACK, STORAGE_CTRL, StorageCmd};
use crate::ui::adapter;
use crate::ui::{UI_VIEW, i18n};

fn drop_stale_speed(out: &mut heapless::Vec<ClientCommand, CMD_BUF>) -> bool {
    let Some(i) = out
        .iter()
        .rposition(|c| matches!(c, ClientCommand::SetSpeed { .. }))
    else {
        return false;
    };
    let _ = out.remove(i);
    true
}

async fn flush_cmds(
    cmd_tx: &embassy_sync::channel::Sender<
        'static,
        CriticalSectionRawMutex,
        ClientCommand,
        { net::PROTO_COMMANDS_DEPTH },
    >,
    out: &mut heapless::Vec<ClientCommand, CMD_BUF>,
    last_cmd: &mut Instant,
) {
    let min_delay = Duration::from_millis(config::network::OUTBOUND_COMMANDS_MIN_DELAY_MS);
    while let Some(cmd) = out.first().cloned() {
        let estop = matches!(cmd, ClientCommand::EStop { .. });
        if !estop {
            let elapsed = last_cmd.elapsed();
            if elapsed < min_delay {
                Timer::after(min_delay - elapsed).await;
            }
        }
        if cmd_tx.try_send(cmd.clone()).is_ok() {
            let _ = out.remove(0);
            *last_cmd = Instant::now();
            continue;
        }
        if drop_stale_speed(out) {
            continue;
        }
        if estop {
            warn!("domain: estop queued, command channel full");
            break;
        }
        warn!("domain: dropping outbound command (channel full)");
        let _ = out.remove(0);
    }
}

fn apply_persist(state: &mut DomainState, rec: PersistRecord) {
    let net = rec.network;
    let net_changed = net != state.persist.network;
    let device = rec.device.clone();
    let hostname = rec.wifi_hostname.clone();
    let language = rec.language;
    state.load_persist(rec);
    crate::net::DEAD_MAN.sender().send(state.dead_man_switch_on);
    i18n::set_language(language);
    DEVICE.sender().send(device);
    if !hostname.is_empty() {
        WIFI_HOSTNAME.sender().send(hostname);
    }
    if net_changed && let Some(cfg) = net {
        NET_CONFIG_CTRL.signal(cfg);
    }
}

fn interpret(
    session: &mut UiSession,
    screen: ScreenId,
    state: &mut DomainState,
    intent: Intent,
    spdt_direction: Direction,
    out: &mut heapless::Vec<ClientCommand, CMD_BUF>,
    wifi_tx: &embassy_sync::channel::Sender<
        'static,
        CriticalSectionRawMutex,
        WifiCmd,
        { net::WIFI_CTRL_DEPTH },
    >,
    srv_tx: &embassy_sync::watch::Sender<
        'static,
        CriticalSectionRawMutex,
        Option<ServerEndpoint>,
        2,
    >,
    storage_tx: &embassy_sync::channel::Sender<'static, CriticalSectionRawMutex, StorageCmd, 4>,
    servers: &heapless::Vec<longfred_proto::network::WitServer, { sizes::MAX_FOUND_SERVERS }>,
) {
    match intent {
        Intent::Action(Action::ShowHideBattery) => session.cycle_battery_mode(),
        Intent::Action(Action::Sleep) => {
            request_device_sleep(state, out, SleepReason::Command);
        }
        Intent::Action(a) => {
            let _ = state.apply_action(a, true, out);
        }
        Intent::AcquireAddr => {
            let _ = state.acquire_addr(session.addr.as_str(), out);
            session.addr.clear();
            if state.current_slot_has_loco() {
                let dir_action = if spdt_direction == Direction::Forward {
                    Action::DirectionForward
                } else {
                    Action::DirectionReverse
                };
                let _ = state.apply_action(dir_action, true, out);
            }
        }
        Intent::AcquireRoster(i) => {
            let _ = state.acquire_roster(i, out);
            if state.current_slot_has_loco() {
                let dir_action = if spdt_direction == Direction::Forward {
                    Action::DirectionForward
                } else {
                    Action::DirectionReverse
                };
                let _ = state.apply_action(dir_action, true, out);
            }
        }
        Intent::SelectLoco(d) => {
            let _ = state.select_loco(d == PageDir::Next, out);
            if state.current_slot_has_loco() {
                let dir_action = if spdt_direction == Direction::Forward {
                    Action::DirectionForward
                } else {
                    Action::DirectionReverse
                };
                let _ = state.apply_action(dir_action, true, out);
            }
        }
        Intent::Pair(code) => {
            state.persist.bigfred_pairing_code = code.clone();
            let _ = storage_tx.try_send(StorageCmd::SavePairingCode(code.clone()));
            if out.push(ClientCommand::Pair { code }).is_err() {
                warn!("domain: pairing command queue full");
            }
        }
        Intent::ReleaseAll => {
            let _ = state.release_all(out);
        }
        Intent::Function(f) => {
            let _ = state.toggle_function(f, out);
        }
        Intent::WifiScan => {
            let _ = wifi_tx.try_send(WifiCmd::Scan);
        }
        Intent::WifiConnect => {
            if !session.selected_ssid.is_empty() {
                let mut ss = String::<32>::new();
                let mut pp = String::<64>::new();
                let _ = ss.push_str(session.selected_ssid.as_str());
                let _ = pp.push_str(session.password.as_str());
                let _ = wifi_tx.try_send(WifiCmd::Connect {
                    ssid: ss,
                    password: pp,
                });
            }
        }
        Intent::ServerSelect(i) => {
            if let Some(s) = servers.get(i)
                && let Some(ip) = s.ipv4
            {
                let ep = ServerEndpoint {
                    ip,
                    port: s.port,
                    protocol: s.protocol,
                };
                persist_last_server(storage_tx, state, ep);
                srv_tx.send(Some(ep));
            }
        }
        Intent::ServerManual => {
            if let Some((ip, port)) = parse_ip_endpoint(session.server_digits.as_str()) {
                let ep = ServerEndpoint {
                    ip,
                    port,
                    protocol: session.manual_protocol,
                };
                persist_last_server(storage_tx, state, ep);
                srv_tx.send(Some(ep));
            }
        }
        Intent::ServerReconnect => {
            if let Some(saved) = state.persist.last_server {
                srv_tx.send(Some(endpoint_from_saved(saved)));
            }
        }
        Intent::ServerDisconnect => {
            srv_tx.send(None);
        }
        Intent::DeadManSwitchToggle => {
            let _ = state.toggle_dead_man_switch(out);
        }
        Intent::HashFunctionsToggle => {
            session.hash_functions = !session.hash_functions;
        }
        Intent::Sleep => {
            request_device_sleep(state, out, SleepReason::Command);
        }
        Intent::RequestMdns => {
            let _ = MDNS_CTRL.try_send(());
        }
        Intent::SaveNetwork(cfg) => {
            let _ = storage_tx.try_send(StorageCmd::SaveNetwork(cfg));
            NET_CONFIG_CTRL.signal(cfg);
            state.show_message(i18n::tr().saved_net);
        }
        Intent::SaveDevice(device) => {
            let _ = storage_tx.try_send(StorageCmd::SaveDevice(device.clone()));
            DEVICE.sender().send(device.clone());
            state.persist.device = device;
            state.show_message(i18n::tr().saved_device);
        }
        Intent::RegenerateDeviceId => {
            let _ = storage_tx.try_send(StorageCmd::RegenerateDeviceId);
            state.show_message(i18n::tr().saved_new_id);
        }
        Intent::SetLanguage(lang) => {
            i18n::set_language(lang);
            let _ = storage_tx.try_send(StorageCmd::SaveLanguage(lang));
            state.persist.language = lang;
            state.persist.language_chosen = true;
            if screen != ScreenId::Language {
                state.show_message(i18n::tr().saved_language);
            }
        }
        Intent::SetRosterMode(mode) => {
            state.persist.roster_mode = mode;
            let _ = storage_tx.try_send(StorageCmd::SaveRosterMode(mode));
            state.refresh_effective_source();
            state.show_message(i18n::tr().saved_roster);
        }
        Intent::EnterProgrammingMode => {
            log::info!("domain: EnterProgrammingMode intent (already applied)");
        }
        Intent::SetHttpOta(on) => {
            net::set_http_ota_enabled(on);
        }
    }
}

fn run_intents(
    ui: &mut adapter::UiWorld,
    intents: heapless::Vec<Intent, 4>,
    spdt_direction: Direction,
    out: &mut heapless::Vec<ClientCommand, CMD_BUF>,
    wifi_tx: &embassy_sync::channel::Sender<
        'static,
        CriticalSectionRawMutex,
        WifiCmd,
        { net::WIFI_CTRL_DEPTH },
    >,
    srv_tx: &embassy_sync::watch::Sender<
        'static,
        CriticalSectionRawMutex,
        Option<ServerEndpoint>,
        2,
    >,
    storage_tx: &embassy_sync::channel::Sender<'static, CriticalSectionRawMutex, StorageCmd, 4>,
) {
    let screen = ui.router.screen_id();
    for intent in intents {
        interpret(
            &mut ui.session,
            screen,
            &mut ui.state,
            intent,
            spdt_direction,
            out,
            wifi_tx,
            srv_tx,
            storage_tx,
            &ui.servers,
        );
    }
}

fn persist_last_server(
    storage_tx: &embassy_sync::channel::Sender<'static, CriticalSectionRawMutex, StorageCmd, 4>,
    state: &mut DomainState,
    ep: ServerEndpoint,
) {
    let saved = SavedServer {
        ip: ep.ip,
        port: ep.port,
        protocol: ep.protocol,
    };
    state.persist.last_server = Some(saved);
    let _ = storage_tx.try_send(StorageCmd::SaveServer(saved));
}

fn last_ssid_owned(state: &DomainState) -> Option<heapless::String<32>> {
    state.persist.last_credential().map(|c| {
        let mut s = heapless::String::new();
        let _ = s.push_str(c.ssid.as_str());
        s
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BootWait {
    Splash,
    Language,
    WifiConnect,
    WifiFailed,
    ServerConnect,
    Done,
}

fn endpoint_from_saved(s: SavedServer) -> ServerEndpoint {
    ServerEndpoint {
        ip: s.ip,
        port: s.port,
        protocol: s.protocol,
    }
}

fn on_wifi_wizard(id: ScreenId) -> bool {
    matches!(
        id,
        ScreenId::Connecting
            | ScreenId::Password
            | ScreenId::SsidList
            | ScreenId::SsidScan
            | ScreenId::SsidScanning
            | ScreenId::WifiFailed
    )
}

fn has_pairing_creds(state: &DomainState) -> bool {
    !state.persist.bigfred_login.is_empty() && !state.persist.bigfred_pin.is_empty()
}

fn start_handset_http(state: &DomainState, op: HandsetHttpOp) -> bool {
    let Some(endpoint) = SERVER.sender().try_get().flatten() else {
        return false;
    };
    if !endpoint.protocol.caps().supports_pairing() || !has_pairing_creds(state) {
        return false;
    }
    PAIRING_HTTP_CTRL
        .try_send(PairingHttpRequest {
            op,
            endpoint,
            login: state.persist.bigfred_login.clone(),
            pin: state.persist.bigfred_pin.clone(),
            device_id: state.persist.device.id_wire(),
        })
        .is_ok()
}

fn start_pairing_http(state: &DomainState) -> bool {
    start_handset_http(state, HandsetHttpOp::Pair)
}

fn start_session_http(state: &DomainState) -> bool {
    start_handset_http(state, HandsetHttpOp::Session)
}

fn show_pairing_overlay(ui: &mut adapter::UiWorld) {
    ui.state.show_message_for(
        adapter::strings_for(&ui.state).msg_pairing,
        longfred_ui::i18n::PAIRING_OVERLAY_TIMEOUT_MS,
    );
}

enum PairingStart {
    Busy,
    Overlay,
    CodeDialog,
}

fn begin_pairing_flow(
    state: &DomainState,
    pairing_active: &mut bool,
    pairing_http_tried: &mut bool,
    pairing_user_initiated: &mut bool,
    out: &mut heapless::Vec<ClientCommand, CMD_BUF>,
) -> PairingStart {
    if *pairing_active {
        return PairingStart::Busy;
    }
    *pairing_user_initiated = false;
    if !state.persist.bigfred_pairing_code.is_empty() {
        *pairing_active = true;
        let code = state.persist.bigfred_pairing_code.clone();
        let _ = out.push(ClientCommand::Pair { code });
        return PairingStart::Overlay;
    }
    if !*pairing_http_tried && start_pairing_http(state) {
        *pairing_active = true;
        *pairing_http_tried = true;
        return PairingStart::Overlay;
    }
    *pairing_active = true;
    PairingStart::CodeDialog
}

fn request_device_sleep(
    state: &mut DomainState,
    out: &mut heapless::Vec<ClientCommand, CMD_BUF>,
    reason: SleepReason,
) {
    let _ = state.apply_action(Action::EStop, true, out);
    net::set_http_ota_enabled(false);
    sleep::begin_sleep(reason);
}

fn has_oled() -> bool {
    crate::board::active().display.is_some()
}

/// TinyC6 VBUS sense (`BatterySample.charging`). Other variants stay false.
fn usb_powered(battery: Option<crate::power::battery::BatterySample>) -> bool {
    battery.is_some_and(|b| b.charging)
}

/// Last domain-loop pulse; [`watchdog_task`] resets the MCU if this stalls.
pub static DOMAIN_PULSE: Watch<CriticalSectionRawMutex, Option<Instant>, 1> = Watch::new_with(None);

fn pulse_domain() {
    DOMAIN_PULSE.sender().send(Some(Instant::now()));
}

/// Software watchdog for the domain loop (frozen UI / blocked command path).
#[embassy_executor::task]
pub async fn watchdog_task() {
    Timer::after(Duration::from_secs(15)).await;
    loop {
        Timer::after(Duration::from_secs(2)).await;
        match DOMAIN_PULSE.sender().try_get().flatten() {
            Some(t) if t.elapsed() < Duration::from_secs(5) => {}
            _ => {
                error!("domain watchdog: stall — reset");
                esp_hal::system::software_reset();
            }
        }
    }
}

#[embassy_executor::task]
pub async fn task() {
    let mut ui = adapter::UiWorld::new();
    let input_rx = input::INPUT_CHANNEL.receiver();
    let events_rx = PROTO_EVENTS.receiver();
    let cmd_tx = PROTO_COMMANDS.sender();
    let wifi_tx = WIFI_CTRL.sender();
    let srv_tx = SERVER.sender();
    let storage_tx = STORAGE_CTRL.sender();
    let ui_tx = UI_VIEW.sender();
    let pairing_http_rx = PAIRING_HTTP_RESULT.receiver();

    let mut out: heapless::Vec<ClientCommand, CMD_BUF> = heapless::Vec::new();
    let mut last_cmd = Instant::from_ticks(0);

    let mut restored_this_session = false;
    let mut last_activity = Instant::now();
    let mut display_blanked = false;
    let mut sleep_requested = false;
    let mut spdt_direction = Direction::Forward;
    let mut pairing_active = false;
    let mut pairing_http_tried = false;
    let mut pairing_user_initiated = false;
    let mut handset_session_paired = false;

    let mut net_rx = STATE.receiver();
    let mut conn_rx = CONN.receiver();
    let mut srv_rx = SERVER.receiver();
    let mut battery_rx = BATTERY.receiver();
    let mut persist_rx = PERSIST_LOADED.receiver();

    if let Some(rx) = persist_rx.as_mut() {
        if let Some(rec) = rx.try_get() {
            info!(
                "domain: persist ready lang_chosen={} creds={}",
                rec.language_chosen,
                rec.credentials.len()
            );
            apply_persist(&mut ui.state, rec);
        } else {
            let rec = rx.get().await;
            info!(
                "domain: persist ready lang_chosen={} creds={}",
                rec.language_chosen,
                rec.credentials.len()
            );
            apply_persist(&mut ui.state, rec);
        }
    } else {
        warn!("domain: persist watch has no free receiver");
    }

    let mut boot_wait = BootWait::Splash;
    let mut phase_until = Some(Instant::now() + Duration::from_millis(config::network::SPLASH_MS));
    let mut saw_wifi_connecting = false;

    if crate::board::active_variant().display.is_none() {
        let ssid = last_ssid_owned(&ui.state);
        let intents = ui.with_ctx(Instant::now().as_millis(), |router, cx| {
            adapter::begin_wifi_setup(router, cx, ssid.as_deref())
        });
        let follow_wifi = intents.iter().any(|i| *i == Intent::WifiConnect);
        if follow_wifi {
            boot_wait = BootWait::WifiConnect;
            phase_until = Some(
                Instant::now() + Duration::from_millis(config::network::SSID_CONNECTION_TIMEOUT_MS),
            );
        } else {
            boot_wait = BootWait::Done;
            phase_until = None;
        }
        run_intents(
            &mut ui,
            intents,
            spdt_direction,
            &mut out,
            &wifi_tx,
            &srv_tx,
            &storage_tx,
        );
    }

    ui.publish_view(Instant::now().as_millis(), &ui_tx);
    pulse_domain();

    loop {
        pulse_domain();
        match select3(
            input_rx.receive(),
            events_rx.receive(),
            Timer::after(Duration::from_millis(config::network::DOMAIN_TICK_MS)),
        )
        .await
        {
            Either3::First(ev) => {
                last_activity = Instant::now();
                if display_blanked {
                    crate::ui::DISPLAY_ON.sender().send(true);
                    display_blanked = false;
                    if matches!(ev, input::InputEvent::EStop | input::InputEvent::Stop) {
                        out.clear();
                        let _ = ui.state.apply_action(Action::EStop, true, &mut out);
                    } else {
                        continue;
                    }
                } else {
                    let splash_active =
                        boot_wait == BootWait::Splash || ui.router.screen_id() == ScreenId::Splash;
                    if splash_active {
                        if matches!(
                            ev,
                            input::InputEvent::Stop
                                | input::InputEvent::EStop
                                | input::InputEvent::EnterProgrammingMode
                        ) {
                            info!("domain: splash STOP — programming mode");
                            let _ = storage_tx.try_send(StorageCmd::SetProgrammingMode(true));
                            if !STORAGE_ACK.wait().await {
                                warn!("domain: programming_mode persist failed — not resetting");
                                continue;
                            }
                            Timer::after(Duration::from_millis(50)).await;
                            esp_hal::system::software_reset();
                        }
                        continue;
                    }
                    if matches!(ev, input::InputEvent::EnterProgrammingMode) {
                        info!("domain: enter programming mode — saving flag and resetting");
                        let _ = storage_tx.try_send(StorageCmd::SetProgrammingMode(true));
                        if !STORAGE_ACK.wait().await {
                            warn!("domain: programming_mode persist failed — not resetting");
                            continue;
                        }
                        Timer::after(Duration::from_millis(50)).await;
                        esp_hal::system::software_reset();
                    }
                    if let input::InputEvent::DirectionSet(dir) = ev {
                        spdt_direction = dir;
                    }
                    flush_cmds(&cmd_tx, &mut out, &mut last_cmd).await;
                    out.clear();
                    if net::http_ota_busy() {
                        if matches!(ev, input::InputEvent::EStop)
                            || (matches!(ev, input::InputEvent::Stop)
                                && ui.router.screen_id() == ScreenId::Throttle)
                        {
                            let _ = ui.state.apply_action(Action::EStop, true, &mut out);
                        }
                    } else {
                        let intents = ui.with_ctx(Instant::now().as_millis(), |router, cx| {
                            router.handle(adapter::map_input(ev), cx)
                        });
                        let screen_after = ui.router.screen_id();
                        let wifi_after_lang =
                            intents.iter().any(|i| matches!(i, Intent::SetLanguage(_)))
                                && screen_after == ScreenId::Language;
                        if intents.iter().any(|i| matches!(i, Intent::Pair(_))) {
                            pairing_active = true;
                            pairing_user_initiated = true;
                        }
                        run_intents(
                            &mut ui,
                            intents,
                            spdt_direction,
                            &mut out,
                            &wifi_tx,
                            &srv_tx,
                            &storage_tx,
                        );
                        if ui.router.screen_id() == ScreenId::ServerList
                            && boot_wait != BootWait::ServerConnect
                        {
                            boot_wait = BootWait::Done;
                            phase_until = None;
                        }
                        if wifi_after_lang {
                            let ssid = last_ssid_owned(&ui.state);
                            let follow = ui.with_ctx(Instant::now().as_millis(), |router, cx| {
                                adapter::begin_wifi_setup(router, cx, ssid.as_deref())
                            });
                            if follow.iter().any(|i| *i == Intent::WifiConnect) {
                                boot_wait = BootWait::WifiConnect;
                                saw_wifi_connecting = false;
                                phase_until = Some(
                                    Instant::now()
                                        + Duration::from_millis(
                                            config::network::SSID_CONNECTION_TIMEOUT_MS,
                                        ),
                                );
                            } else {
                                boot_wait = BootWait::Done;
                                phase_until = None;
                            }
                            run_intents(
                                &mut ui,
                                follow,
                                spdt_direction,
                                &mut out,
                                &wifi_tx,
                                &srv_tx,
                                &storage_tx,
                            );
                        }
                    }
                }
            }
            Either3::Second(sev) => {
                flush_cmds(&cmd_tx, &mut out, &mut last_cmd).await;
                out.clear();
                let app_event = match &sev {
                    longfred_proto::ServerEvent::Alert(text) if text.as_str() == "Not paired" => {
                        handset_session_paired = false;
                        match begin_pairing_flow(
                            &ui.state,
                            &mut pairing_active,
                            &mut pairing_http_tried,
                            &mut pairing_user_initiated,
                            &mut out,
                        ) {
                            PairingStart::Busy => None,
                            PairingStart::Overlay => {
                                show_pairing_overlay(&mut ui);
                                None
                            }
                            PairingStart::CodeDialog => Some(AppEvent::PairingRequired),
                        }
                    }
                    longfred_proto::ServerEvent::PairingRequired
                        if pairing_active || handset_session_paired =>
                    {
                        None
                    }
                    longfred_proto::ServerEvent::PairingRequired => {
                        match begin_pairing_flow(
                            &ui.state,
                            &mut pairing_active,
                            &mut pairing_http_tried,
                            &mut pairing_user_initiated,
                            &mut out,
                        ) {
                            PairingStart::Busy => None,
                            PairingStart::Overlay => {
                                show_pairing_overlay(&mut ui);
                                None
                            }
                            PairingStart::CodeDialog => Some(AppEvent::PairingRequired),
                        }
                    }
                    longfred_proto::ServerEvent::PairingSucceeded(_) => {
                        pairing_active = false;
                        pairing_http_tried = false;
                        handset_session_paired = true;
                        ui.router.dismiss_overlay();
                        ui.state.reacquire_session_locos(&mut out);
                        restored_this_session = true;
                        let app_event =
                            pairing_user_initiated.then_some(AppEvent::PairingSucceeded);
                        pairing_user_initiated = false;
                        app_event
                    }
                    longfred_proto::ServerEvent::PairingFailed => {
                        handset_session_paired = false;
                        ui.state.persist.bigfred_pairing_code.clear();
                        let _ = storage_tx
                            .try_send(StorageCmd::SavePairingCode(heapless::String::new()));
                        if has_pairing_creds(&ui.state) {
                            if !pairing_http_tried && start_pairing_http(&ui.state) {
                                pairing_active = true;
                                pairing_http_tried = true;
                                show_pairing_overlay(&mut ui);
                                None
                            } else {
                                pairing_active = false;
                                pairing_http_tried = false;
                                pairing_user_initiated = false;
                                None
                            }
                        } else {
                            pairing_active = true;
                            let app_event = if pairing_user_initiated {
                                AppEvent::PairingFailed
                            } else {
                                AppEvent::PairingRequired
                            };
                            pairing_user_initiated = false;
                            Some(app_event)
                        }
                    }
                    _ => None,
                };
                let _ = ui.state.apply_event(sev, &mut out);
                if let Some(event) = app_event {
                    let follow = ui.with_ctx(Instant::now().as_millis(), |router, cx| {
                        router.on_app_event(event, cx)
                    });
                    run_intents(
                        &mut ui,
                        follow,
                        spdt_direction,
                        &mut out,
                        &wifi_tx,
                        &srv_tx,
                        &storage_tx,
                    );
                }
            }
            Either3::Third(_) => {
                let now = Instant::now();
                let timed_out = phase_until.is_some_and(|t| now >= t);
                match boot_wait {
                    BootWait::Splash if timed_out => {
                        if !ui.state.persist.language_chosen {
                            ui.session.splash_done = true;
                            ui.session.boot_language = true;
                            let _ = ui.with_ctx(now.as_millis(), |router, cx| {
                                router.replace_screen(ScreenId::Language, cx)
                            });
                            boot_wait = BootWait::Language;
                            phase_until = None;
                        } else {
                            let ssid = last_ssid_owned(&ui.state);
                            let follow = ui.with_ctx(now.as_millis(), |router, cx| {
                                adapter::begin_wifi_setup(router, cx, ssid.as_deref())
                            });
                            if follow.iter().any(|i| *i == Intent::WifiConnect) {
                                boot_wait = BootWait::WifiConnect;
                                saw_wifi_connecting = false;
                                phase_until = Some(
                                    now + Duration::from_millis(
                                        config::network::SSID_CONNECTION_TIMEOUT_MS,
                                    ),
                                );
                            } else {
                                boot_wait = BootWait::Done;
                                phase_until = None;
                            }
                            run_intents(
                                &mut ui,
                                follow,
                                spdt_direction,
                                &mut out,
                                &wifi_tx,
                                &srv_tx,
                                &storage_tx,
                            );
                        }
                    }
                    BootWait::WifiConnect if timed_out => {
                        info!("domain: Wi-Fi connect timed out");
                        let _ = ui.with_ctx(now.as_millis(), |router, cx| {
                            router.replace_screen(ScreenId::WifiFailed, cx)
                        });
                        boot_wait = BootWait::WifiFailed;
                        phase_until =
                            Some(now + Duration::from_millis(config::network::WIFI_FAIL_MSG_MS));
                    }
                    BootWait::WifiFailed if timed_out => {
                        let follow = ui.with_ctx(now.as_millis(), |router, cx| {
                            router.replace_screen(ScreenId::SsidScanning, cx)
                        });
                        boot_wait = BootWait::Done;
                        phase_until = None;
                        run_intents(
                            &mut ui,
                            follow,
                            spdt_direction,
                            &mut out,
                            &wifi_tx,
                            &srv_tx,
                            &storage_tx,
                        );
                    }
                    BootWait::ServerConnect if timed_out => {
                        info!("domain: server connect still pending");
                        ui.state.show_message(i18n::tr().msg_connect_slow);
                        phase_until = None;
                    }
                    _ => {}
                }
                let usb = usb_powered(ui.battery);
                if usb && display_blanked {
                    crate::ui::DISPLAY_ON.sender().send(true);
                    display_blanked = false;
                }
                if !net::http_ota_busy()
                    && boot_wait == BootWait::Done
                    && has_oled()
                    && !display_blanked
                    && !usb
                    && power::DISPLAY_BLANK_INACTIVITY_MS > 0
                    && last_activity.elapsed().as_millis() > power::DISPLAY_BLANK_INACTIVITY_MS
                {
                    crate::ui::DISPLAY_ON.sender().send(false);
                    display_blanked = true;
                }
                if !net::http_ota_busy()
                    && power::AUTO_SLEEP_INACTIVITY_MS > 0
                    && boot_wait == BootWait::Done
                    && !sleep_requested
                    && !usb
                    && !ui.state.any_loco_moving()
                    && last_activity.elapsed().as_millis() > power::AUTO_SLEEP_INACTIVITY_MS
                {
                    display_blanked = false;
                    sleep_requested = true;
                    request_device_sleep(&mut ui.state, &mut out, SleepReason::Inactivity);
                }
            }
        }

        if let Some(s) = net_rx.as_mut().and_then(|r| r.try_get()) {
            if s != ui.net_status {
                ui.net_status = s;
                if s == NetStatus::Connecting {
                    saw_wifi_connecting = true;
                }
                if s == NetStatus::Ready {
                    if let Some((ssid, pw)) = adapter::take_pending_password_save(&mut ui.session) {
                        let _ =
                            storage_tx.try_send(StorageCmd::SavePassword { ssid, password: pw });
                    }
                    if on_wifi_wizard(ui.router.screen_id()) {
                        if let Some(saved) = ui.state.persist.last_server {
                            srv_tx.send(Some(endpoint_from_saved(saved)));
                            let _ = ui.with_ctx(Instant::now().as_millis(), |router, cx| {
                                router.replace_screen(ScreenId::Connecting, cx)
                            });
                            boot_wait = BootWait::ServerConnect;
                            phase_until = Some(
                                Instant::now()
                                    + Duration::from_millis(
                                        config::network::SERVER_CONNECTION_TIMEOUT_MS,
                                    ),
                            );
                        } else {
                            let follow = ui.with_ctx(Instant::now().as_millis(), |router, cx| {
                                router.replace_screen(ScreenId::ServerList, cx)
                            });
                            boot_wait = BootWait::Done;
                            phase_until = None;
                            run_intents(
                                &mut ui,
                                follow,
                                spdt_direction,
                                &mut out,
                                &wifi_tx,
                                &srv_tx,
                                &storage_tx,
                            );
                        }
                    } else {
                        let follow = ui.with_ctx(Instant::now().as_millis(), |router, cx| {
                            router.on_app_event(AppEvent::WifiReady, cx)
                        });
                        run_intents(
                            &mut ui,
                            follow,
                            spdt_direction,
                            &mut out,
                            &wifi_tx,
                            &srv_tx,
                            &storage_tx,
                        );
                        if ui.router.screen_id() == ScreenId::ServerList {
                            let _ = MDNS_CTRL.try_send(());
                        }
                    }
                } else if s == NetStatus::Disconnected
                    && boot_wait == BootWait::WifiConnect
                    && saw_wifi_connecting
                {
                    let _ = ui.with_ctx(Instant::now().as_millis(), |router, cx| {
                        router.replace_screen(ScreenId::WifiFailed, cx)
                    });
                    boot_wait = BootWait::WifiFailed;
                    phase_until = Some(
                        Instant::now() + Duration::from_millis(config::network::WIFI_FAIL_MSG_MS),
                    );
                }
            }
        }

        if let Some(w) = conn_rx.as_mut().and_then(|r| r.try_get()) {
            if w != ui.conn {
                let prev = ui.conn;
                ui.conn = w;
                if w == ConnState::Connected {
                    last_activity = Instant::now();
                    boot_wait = BootWait::Done;
                    phase_until = None;
                    let follow = ui.with_ctx(Instant::now().as_millis(), |router, cx| {
                        router.on_app_event(AppEvent::ServerConnected, cx)
                    });
                    run_intents(
                        &mut ui,
                        follow,
                        spdt_direction,
                        &mut out,
                        &wifi_tx,
                        &srv_tx,
                        &storage_tx,
                    );
                    if let Some(ep) = SERVER.sender().try_get().flatten() {
                        persist_last_server(&storage_tx, &mut ui.state, ep);
                        ui.state.ensure_session(ep.protocol.caps());
                    }
                    let defer_reacquire = SERVER.sender().try_get().flatten().is_some_and(|ep| {
                        ep.protocol.caps().supports_pairing() && has_pairing_creds(&ui.state)
                    });
                    if defer_reacquire && start_session_http(&ui.state) {
                        pairing_active = true;
                    } else if !restored_this_session {
                        out.clear();
                        ui.state.reacquire_session_locos(&mut out);
                        restored_this_session = true;
                    }
                } else if w == ConnState::Disconnected
                    || (w == ConnState::Connecting && prev == ConnState::Connected)
                {
                    ui.state.end_session();
                    pairing_active = false;
                    pairing_http_tried = false;
                    pairing_user_initiated = false;
                    handset_session_paired = false;
                    restored_this_session = false;
                }
            }
        }

        if let Some(ep) = srv_rx.as_mut().and_then(|r| r.try_get()) {
            ui.server = ep;
            if ui.conn == ConnState::Connected
                && let Some(ep) = ep
            {
                ui.state.ensure_session(ep.protocol.caps());
            }
        }

        if let Some(v) = WIFI_SCAN.try_take() {
            ui.scanned = v;
            let follow = ui.with_ctx(Instant::now().as_millis(), |router, cx| {
                router.on_app_event(AppEvent::ScanDone, cx)
            });
            run_intents(
                &mut ui,
                follow,
                spdt_direction,
                &mut out,
                &wifi_tx,
                &srv_tx,
                &storage_tx,
            );
        }

        if let Some(v) = FOUND_SERVERS.try_take() {
            ui.servers = v;
        }

        if let Some(rec) = persist_rx.as_mut().and_then(|r| r.try_changed()) {
            apply_persist(&mut ui.state, rec);
        }

        if let Some(b) = battery_rx.as_mut().and_then(|r| r.try_changed()) {
            ui.battery = b;
            if let Some(sample) = b {
                push_battery_sample(&mut ui.battery_history, sample.percent);
            }
        }

        if let Ok(result) = pairing_http_rx.try_receive() {
            let endpoint = match &result {
                PairingHttpResult::Code { endpoint, .. }
                | PairingHttpResult::Session { endpoint, .. }
                | PairingHttpResult::Failed { endpoint, .. } => *endpoint,
            };
            let still_current = CONN.sender().try_get() == Some(ConnState::Connected)
                && SERVER.sender().try_get().flatten() == Some(endpoint);
            if still_current {
                let app_event = match result {
                    PairingHttpResult::Code { code, .. } => {
                        ui.state.persist.bigfred_pairing_code = code.clone();
                        let _ = storage_tx.try_send(StorageCmd::SavePairingCode(code.clone()));
                        let _ = out.push(ClientCommand::Pair { code });
                        show_pairing_overlay(&mut ui);
                        None
                    }
                    PairingHttpResult::Session { paired: true, .. } => {
                        pairing_active = false;
                        pairing_http_tried = false;
                        handset_session_paired = true;
                        ui.router.dismiss_overlay();
                        if !restored_this_session {
                            ui.state.reacquire_session_locos(&mut out);
                            restored_this_session = true;
                        }
                        None
                    }
                    PairingHttpResult::Session { paired: false, .. } => {
                        handset_session_paired = false;
                        pairing_http_tried = false;
                        if start_pairing_http(&ui.state) {
                            pairing_active = true;
                            pairing_http_tried = true;
                            show_pairing_overlay(&mut ui);
                        } else {
                            pairing_active = false;
                        }
                        None
                    }
                    PairingHttpResult::Failed { error, .. } => {
                        pairing_active = false;
                        pairing_http_tried = false;
                        if !error.is_empty() {
                            ui.state.show_message(error.as_str());
                        }
                        if has_pairing_creds(&ui.state) {
                            None
                        } else {
                            Some(AppEvent::PairingRequired)
                        }
                    }
                };
                if let Some(event) = app_event {
                    let follow = ui.with_ctx(Instant::now().as_millis(), |router, cx| {
                        router.on_app_event(event, cx)
                    });
                    run_intents(
                        &mut ui,
                        follow,
                        spdt_direction,
                        &mut out,
                        &wifi_tx,
                        &srv_tx,
                        &storage_tx,
                    );
                }
            }
        }

        ui.state.poll_roster_timeout(Instant::now());

        {
            let follow = ui.with_ctx(Instant::now().as_millis(), |router, cx| router.tick(cx));
            run_intents(
                &mut ui,
                follow,
                spdt_direction,
                &mut out,
                &wifi_tx,
                &srv_tx,
                &storage_tx,
            );
        }

        ui.state.flush_pending_speed(&mut out);

        net::set_ping_enabled(ui.router.screen_id() == ScreenId::Diagnostics);

        ui.apply_pending_overlay(Instant::now().as_millis());
        ui.publish_view(Instant::now().as_millis(), &ui_tx);

        flush_cmds(&cmd_tx, &mut out, &mut last_cmd).await;
    }
}
