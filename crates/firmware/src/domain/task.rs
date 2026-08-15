//! Domain task: menu FSM + state + network + UI_VIEW publication.

use embassy_futures::select::{Either3, select3};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_time::{Duration, Instant, Timer};
use heapless::String;
use log::info;
#[cfg(feature = "sim")]
use log::warn;
use longfred_proto::command::ClientCommand;
use longfred_proto::model::Direction;
use longfred_proto::persist::PersistRecord;

use crate::config::{self, power, sizes};
use crate::domain::actions::Action;
use crate::domain::state::{CMD_BUF, DomainState};
use crate::input;
use crate::net::{
    self, CONN, ConnState, DEVICE, FOUND_SERVERS, MDNS_CTRL, NET_CONFIG_CTRL, NetStatus,
    PROTO_COMMANDS, PROTO_EVENTS, SERVER, STATE, ServerEndpoint, WIFI_CTRL, WIFI_HOSTNAME,
    WIFI_SCAN, WifiCmd,
};
use crate::power::battery::BATTERY;
use crate::power::sleep::{SLEEP_CTRL, SleepReason};
use crate::storage::{PERSIST_LOADED, STORAGE_ACK, STORAGE_CTRL, StorageCmd};
use crate::ui::menu::{Intent, ListRef, MenuFsm, Screen};
use crate::ui::view::ViewCtx;
use crate::ui::{UI_VIEW, i18n};

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
        let elapsed = last_cmd.elapsed();
        if elapsed < min_delay {
            Timer::after(min_delay - elapsed).await;
        }
        let _ = out.remove(0);
        cmd_tx.send(cmd).await;
        *last_cmd = Instant::now();
    }
}

fn publish_view(
    fsm: &MenuFsm,
    state: &DomainState,
    net_status: NetStatus,
    conn: ConnState,
    server: Option<ServerEndpoint>,
    scanned: &heapless::Vec<net::SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
    servers: &heapless::Vec<longfred_proto::mdns::WitServer, { sizes::MAX_FOUND_SERVERS }>,
    pw_preview: &str,
    ip_formatted: &str,
    battery: Option<u8>,
    ui_tx: &embassy_sync::watch::Sender<
        'static,
        CriticalSectionRawMutex,
        crate::ui::view::UiView,
        2,
    >,
) {
    let (ssid, _) = fsm.ssid_for_connect(scanned, state);
    let ctx = ViewCtx {
        domain: state,
        net_status,
        conn,
        server,
        scanned_ssids: scanned,
        found_servers: servers,
        selected_ssid: ssid,
        password_preview: pw_preview,
        pw_picker_char: fsm.pw_picker_char(),
        ip_formatted,
        broadcast: state.active_broadcast(),
        battery,
        sta_ipv4: net::sta_ipv4(),
        http_ota: net::http_ota_enabled(),
        http_ota_busy: net::http_ota_busy(),
    };
    ui_tx.send(fsm.view(&ctx));
}

fn apply_persist(state: &mut DomainState, rec: PersistRecord) {
    let net = rec.network;
    let device = rec.device.clone();
    let hostname = rec.wifi_hostname.clone();
    let language = rec.language;
    state.load_persist(rec);
    i18n::set_language(language);
    DEVICE.sender().send(device);
    if !hostname.is_empty() {
        WIFI_HOSTNAME.sender().send(hostname);
    }
    if let Some(cfg) = net {
        NET_CONFIG_CTRL.signal(cfg);
    }
}

fn interpret(
    fsm: &mut MenuFsm,
    state: &mut DomainState,
    intent: Intent,
    spdt_direction: Direction,
    out: &mut heapless::Vec<ClientCommand, CMD_BUF>,
    scanned: &heapless::Vec<net::SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
    servers: &heapless::Vec<longfred_proto::mdns::WitServer, { sizes::MAX_FOUND_SERVERS }>,
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
    match intent {
        Intent::None => {}
        Intent::Action(Action::ShowHideBattery) => fsm.cycle_battery_mode(),
        Intent::Action(Action::Sleep) => {
            net::set_http_ota_enabled(false);
            SLEEP_CTRL.signal(SleepReason::Command);
        }
        Intent::Action(a) => {
            let _ = state.apply_action(a, true, out);
        }
        Intent::AcquireAddr => {
            let _ = state.acquire_addr(fsm.addr.as_str(), out);
            fsm.addr.clear();
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
        Intent::ReleaseAll => {
            let _ = state.release_all(out);
        }
        Intent::Function(f, on) => {
            let _ = state.apply_function(f, on, out);
        }
        Intent::Turnout(action, ListRef::Addr) => {
            let _ = state.turnout_by_addr(action, fsm.addr.as_str(), out);
            fsm.addr.clear();
        }
        Intent::Turnout(action, ListRef::Index(i)) => {
            let _ = state.turnout_by_index(action, i, out);
        }
        Intent::Route(ListRef::Addr) => {
            let _ = state.route_by_addr(fsm.addr.as_str(), out);
            fsm.addr.clear();
        }
        Intent::Route(ListRef::Index(i)) => {
            let _ = state.route_by_index(i, out);
        }
        Intent::WifiScan => {
            let _ = wifi_tx.try_send(WifiCmd::Scan);
        }
        Intent::WifiSelect(_, _) => {}
        Intent::WifiConnect => {
            let (ssid, pw) = fsm.ssid_for_connect(scanned, state);
            if !ssid.is_empty() {
                let mut ss = String::<32>::new();
                let mut pp = String::<64>::new();
                let _ = ss.push_str(ssid);
                let _ = pp.push_str(pw);
                let _ = wifi_tx.try_send(WifiCmd::Connect {
                    ssid: ss,
                    password: pp,
                });
            }
        }
        Intent::ServerSelect(i) => {
            if let Some(s) = servers.get(i) {
                if let Some(ip) = s.ipv4 {
                    srv_tx.send(Some(ServerEndpoint {
                        ip,
                        port: s.port,
                        protocol: s.protocol,
                    }));
                    fsm.screen = Screen::Throttle;
                }
            }
        }
        Intent::ServerManual => {
            if let Some((ip, port)) = fsm.ip_endpoint() {
                srv_tx.send(Some(ServerEndpoint {
                    ip,
                    port,
                    protocol: fsm.manual_protocol(),
                }));
                fsm.screen = Screen::Throttle;
            }
        }
        Intent::HeartbeatToggle => {
            let _ = state.toggle_heartbeat(out);
        }
        Intent::DropBeforeAcquireToggle => state.toggle_drop_before_acquire(),
        Intent::HashFunctionsToggle => fsm.toggle_hash_functions(),
        Intent::Sleep => {
            net::set_http_ota_enabled(false);
            SLEEP_CTRL.signal(SleepReason::Command);
        }
        Intent::SaveLocos => {
            let locos = state.collect_saved_locos();
            let _ = storage_tx.try_send(StorageCmd::SaveLocos(locos));
            state.show_message(i18n::tr().saved_locos);
        }
        Intent::RequestMdns => {
            let _ = MDNS_CTRL.try_send(());
        }
        Intent::NetConfig => {
            fsm.screen = Screen::IpConfig;
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
            state.show_message(i18n::tr().saved_language);
        }
        Intent::EnterProgrammingMode => {
            // Handled eagerly in the input loop (persist + software_reset).
            log::info!("domain: EnterProgrammingMode intent (already applied)");
        }
        Intent::SetHttpOta(on) => {
            net::set_http_ota_enabled(on);
        }
    }
}

#[embassy_executor::task]
pub async fn task() {
    let mut state = DomainState::new();
    let mut fsm = MenuFsm::new();
    let input_rx = input::INPUT_CHANNEL.receiver();
    let events_rx = PROTO_EVENTS.receiver();
    let cmd_tx = PROTO_COMMANDS.sender();
    let wifi_tx = WIFI_CTRL.sender();
    let srv_tx = SERVER.sender();
    let storage_tx = STORAGE_CTRL.sender();
    let ui_tx = UI_VIEW.sender();

    let mut out: heapless::Vec<ClientCommand, CMD_BUF> = heapless::Vec::new();
    // Epoch: do not use `now - 1s` — Instant wraps/panics when uptime < 1s (Wokwi, cold boot).
    let mut last_cmd = Instant::from_ticks(0);

    let mut net_status = NetStatus::Disconnected;
    let mut conn = ConnState::Disconnected;
    let mut server: Option<ServerEndpoint> = None;
    let mut scanned: heapless::Vec<net::SsidInfo, { sizes::MAX_FOUND_SSIDS }> =
        heapless::Vec::new();
    let mut servers: heapless::Vec<longfred_proto::mdns::WitServer, { sizes::MAX_FOUND_SERVERS }> =
        heapless::Vec::new();
    let mut battery: Option<u8> = None;
    let mut restored_this_session = false;
    let mut last_activity = Instant::now();
    let mut spdt_direction = Direction::Forward;

    let mut net_rx = STATE.receiver();
    let mut conn_rx = CONN.receiver();
    let mut srv_rx = SERVER.receiver();
    let mut battery_rx = BATTERY.receiver();

    let mut pw_buf = heapless::String::<36>::new();
    let mut ip_buf = heapless::String::<24>::new();

    if let Some(rec) = PERSIST_LOADED.try_take() {
        apply_persist(&mut state, rec);
    }

    let splash_intent = fsm.tick_splash();
    if splash_intent != Intent::None {
        interpret(
            &mut fsm,
            &mut state,
            splash_intent,
            spdt_direction,
            &mut out,
            &scanned,
            &servers,
            &wifi_tx,
            &srv_tx,
            &storage_tx,
        );
        flush_cmds(&cmd_tx, &mut out, &mut last_cmd).await;
    }

    publish_view(
        &fsm,
        &state,
        net_status,
        conn,
        server,
        &scanned,
        &servers,
        pw_buf.as_str(),
        ip_buf.as_str(),
        battery,
        &ui_tx,
    );

    loop {
        match select3(
            input_rx.receive(),
            events_rx.receive(),
            Timer::after(Duration::from_millis(config::network::DOMAIN_TICK_MS)),
        )
        .await
        {
            Either3::First(ev) => {
                last_activity = Instant::now();
                if matches!(ev, input::InputEvent::EnterProgrammingMode) {
                    info!("domain: enter programming mode — saving flag and resetting");
                    let _ = storage_tx.try_send(StorageCmd::SetProgrammingMode(true));
                    STORAGE_ACK.wait().await;
                    Timer::after(Duration::from_millis(50)).await;
                    #[cfg(not(feature = "sim"))]
                    esp_hal::system::software_reset();
                    #[cfg(feature = "sim")]
                    {
                        warn!("sim: software_reset skipped");
                        continue;
                    }
                }
                if let input::InputEvent::DirectionSet(dir) = ev {
                    spdt_direction = dir;
                }
                out.clear();
                if net::http_ota_busy() {
                    // Ignore navigation while an image is streaming to flash.
                } else {
                    let intent = fsm.handle(ev, &state, &scanned);
                    interpret(
                        &mut fsm,
                        &mut state,
                        intent,
                        spdt_direction,
                        &mut out,
                        &scanned,
                        &servers,
                        &wifi_tx,
                        &srv_tx,
                        &storage_tx,
                    );
                }
            }
            Either3::Second(sev) => {
                out.clear();
                let _ = state.apply_event(sev, &mut out);
            }
            Either3::Third(_) => {
                if !net::http_ota_busy()
                    && power::AUTO_SLEEP_INACTIVITY_MS > 0
                    && conn != ConnState::Connected
                    && last_activity.elapsed().as_millis() > power::AUTO_SLEEP_INACTIVITY_MS
                {
                    net::set_http_ota_enabled(false);
                    SLEEP_CTRL.signal(SleepReason::Inactivity);
                }
            }
        }

        if let Some(s) = net_rx.as_mut().and_then(|r| r.try_get()) {
            if s != net_status {
                net_status = s;
                if s == NetStatus::Ready {
                    fsm.on_wifi_ready();
                    if fsm.screen == Screen::ServerList {
                        let _ = MDNS_CTRL.try_send(());
                    }
                    if let Some((ssid, pw)) = fsm.take_pending_password_save() {
                        let _ =
                            storage_tx.try_send(StorageCmd::SavePassword { ssid, password: pw });
                    }
                }
            }
        }

        if let Some(w) = conn_rx.as_mut().and_then(|r| r.try_get()) {
            if w != conn {
                conn = w;
                if w == ConnState::Connected {
                    last_activity = Instant::now();
                    fsm.on_server_connected();
                    if !restored_this_session {
                        out.clear();
                        state.restore_locos(&mut out);
                        restored_this_session = true;
                    }
                } else if w == ConnState::Disconnected {
                    restored_this_session = false;
                }
            }
        }

        if let Some(ep) = srv_rx.as_mut().and_then(|r| r.try_get()) {
            server = ep;
        }

        if let Some(v) = WIFI_SCAN.try_take() {
            scanned = v;
            fsm.on_scan_done();
        }

        if let Some(v) = FOUND_SERVERS.try_take() {
            servers = v;
        }

        if let Some(rec) = PERSIST_LOADED.try_take() {
            apply_persist(&mut state, rec);
        }

        if let Some(b) = battery_rx.as_mut().and_then(|r| r.try_get()) {
            battery = b;
        }

        pw_buf.clear();
        if fsm.screen == Screen::DeviceNameEdit {
            let _ = pw_buf.push_str(fsm.device_name_preview().as_str());
        } else {
            let _ = pw_buf.push_str(fsm.password_preview().as_str());
        }
        ip_buf.clear();
        if fsm.screen == Screen::IpEdit {
            let _ = ip_buf.push_str(fsm.format_net_display().as_str());
        } else if fsm.screen == Screen::DeviceIdEdit {
            let _ = ip_buf.push_str(fsm.format_device_id_display().as_str());
        } else {
            let _ = ip_buf.push_str(fsm.format_ip_display().as_str());
        };

        state.flush_pending_speed(&mut out);

        publish_view(
            &fsm,
            &state,
            net_status,
            conn,
            server,
            &scanned,
            &servers,
            pw_buf.as_str(),
            ip_buf.as_str(),
            battery,
            &ui_tx,
        );

        flush_cmds(&cmd_tx, &mut out, &mut last_cmd).await;
    }
}
