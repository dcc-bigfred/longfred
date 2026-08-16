//! Domain task: screen router + state + network + UI_VIEW publication.

use embassy_futures::select::{Either3, select3};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_time::{Duration, Instant, Timer};
use heapless::String;
use log::{info, warn};
use longfred_proto::command::ClientCommand;
use longfred_proto::menu::parse_ip_endpoint;
use longfred_proto::model::Direction;
use longfred_proto::persist::{PersistRecord, SavedServer};
use longfred_ui::nav::ScreenId;
use longfred_ui::{AppEvent, Intent, Router, UiEnv, UiSession};

use crate::config::{self, power, sizes};
use crate::domain::actions::Action;
use crate::domain::state::{CMD_BUF, DomainState};
use crate::input;
use crate::net::{
    self, CONN, ConnState, DEVICE, FOUND_SERVERS, MDNS_CTRL, NET_CONFIG_CTRL, NetStatus,
    PROTO_COMMANDS, PROTO_EVENTS, SERVER, STATE, ServerEndpoint, WIFI_CTRL, WIFI_HOSTNAME,
    WIFI_SCAN, WifiCmd,
};
use crate::power::battery::{BATTERY, BatterySample};
use crate::power::sleep::{SLEEP_CTRL, SleepReason};
use crate::storage::{PERSIST_LOADED, STORAGE_ACK, STORAGE_CTRL, StorageCmd};
use crate::ui::adapter;
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

fn apply_persist(state: &mut DomainState, rec: PersistRecord) {
    let net = rec.network;
    let net_changed = net != state.persist.network;
    let device = rec.device.clone();
    let hostname = rec.wifi_hostname.clone();
    let language = rec.language;
    state.load_persist(rec);
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
    servers: &heapless::Vec<longfred_proto::mdns::WitServer, { sizes::MAX_FOUND_SERVERS }>,
) {
    match intent {
        Intent::Action(Action::ShowHideBattery) => session.cycle_battery_mode(),
        Intent::Action(Action::Sleep) => {
            net::set_http_ota_enabled(false);
            SLEEP_CTRL.signal(SleepReason::Command);
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
        Intent::HeartbeatToggle => {
            let _ = state.toggle_heartbeat(out);
        }
        Intent::DropBeforeAcquireToggle => state.toggle_drop_before_acquire(),
        Intent::HashFunctionsToggle => {
            session.hash_functions = !session.hash_functions;
        }
        Intent::Sleep => {
            net::set_http_ota_enabled(false);
            SLEEP_CTRL.signal(SleepReason::Command);
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
        Intent::EnterProgrammingMode => {
            log::info!("domain: EnterProgrammingMode intent (already applied)");
        }
        Intent::SetHttpOta(on) => {
            net::set_http_ota_enabled(on);
        }
    }
}

fn run_intents(
    session: &mut UiSession,
    screen: ScreenId,
    state: &mut DomainState,
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
    servers: &heapless::Vec<longfred_proto::mdns::WitServer, { sizes::MAX_FOUND_SERVERS }>,
) {
    for intent in intents {
        interpret(
            session,
            screen,
            state,
            intent,
            spdt_direction,
            out,
            wifi_tx,
            srv_tx,
            storage_tx,
            servers,
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

#[embassy_executor::task]
pub async fn task() {
    let mut state = DomainState::new();
    let env: UiEnv = adapter::ui_env();
    let mut session = adapter::init_session();
    let mut router = Router::new(adapter::nav_profile(), ScreenId::Splash);
    let input_rx = input::INPUT_CHANNEL.receiver();
    let events_rx = PROTO_EVENTS.receiver();
    let cmd_tx = PROTO_COMMANDS.sender();
    let wifi_tx = WIFI_CTRL.sender();
    let srv_tx = SERVER.sender();
    let storage_tx = STORAGE_CTRL.sender();
    let ui_tx = UI_VIEW.sender();

    let mut out: heapless::Vec<ClientCommand, CMD_BUF> = heapless::Vec::new();
    let mut last_cmd = Instant::from_ticks(0);

    let mut net_status = NetStatus::Disconnected;
    let mut conn = ConnState::Disconnected;
    let mut server: Option<ServerEndpoint> = None;
    let mut scanned: heapless::Vec<net::SsidInfo, { sizes::MAX_FOUND_SSIDS }> =
        heapless::Vec::new();
    let mut servers: heapless::Vec<longfred_proto::mdns::WitServer, { sizes::MAX_FOUND_SERVERS }> =
        heapless::Vec::new();
    let mut battery: Option<BatterySample> = None;
    let mut restored_this_session = false;
    let mut last_activity = Instant::now();
    let mut spdt_direction = Direction::Forward;

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
            apply_persist(&mut state, rec);
        } else {
            #[cfg(not(feature = "sim"))]
            {
                let rec = rx.get().await;
                info!(
                    "domain: persist ready lang_chosen={} creds={}",
                    rec.language_chosen,
                    rec.credentials.len()
                );
                apply_persist(&mut state, rec);
            }
        }
    } else {
        warn!("domain: persist watch has no free receiver");
    }

    let mut boot_wait = BootWait::Splash;
    let mut phase_until = Some(Instant::now() + Duration::from_millis(config::network::SPLASH_MS));
    let mut saw_wifi_connecting = false;

    if crate::board::active_variant().display.is_none() {
        let ssid = last_ssid_owned(&state);
        let strings = adapter::strings_for(&state);
        let mut cx = adapter::screen_ctx(
            &state,
            &mut session,
            &env,
            strings,
            net_status,
            conn,
            server,
            &scanned,
            &servers,
            battery,
            Instant::now().as_millis(),
        );
        let intents = adapter::begin_wifi_setup(&mut router, &mut cx, ssid.as_deref());
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
            &mut session,
            router.screen_id(),
            &mut state,
            intents,
            spdt_direction,
            &mut out,
            &wifi_tx,
            &srv_tx,
            &storage_tx,
            &servers,
        );
    }

    adapter::publish(
        &router,
        &state,
        &mut session,
        &env,
        adapter::strings_for(&state),
        net_status,
        conn,
        server,
        &scanned,
        &servers,
        battery,
        Instant::now().as_millis(),
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
                let splash_active =
                    boot_wait == BootWait::Splash || router.screen_id() == ScreenId::Splash;
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
                        #[cfg(not(feature = "sim"))]
                        esp_hal::system::software_reset();
                        #[cfg(feature = "sim")]
                        {
                            warn!("sim: software_reset skipped");
                            continue;
                        }
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
                    if matches!(ev, input::InputEvent::EStop)
                        || (matches!(ev, input::InputEvent::Stop)
                            && router.screen_id() == ScreenId::Throttle)
                    {
                        let _ = state.apply_action(Action::EStop, true, &mut out);
                    }
                } else {
                    let strings = adapter::strings_for(&state);
                    let mut cx = adapter::screen_ctx(
                        &state,
                        &mut session,
                        &env,
                        strings,
                        net_status,
                        conn,
                        server,
                        &scanned,
                        &servers,
                        battery,
                        Instant::now().as_millis(),
                    );
                    let intents = router.handle(adapter::map_input(ev), &mut cx);
                    let screen_after = router.screen_id();
                    let wifi_after_lang =
                        intents.iter().any(|i| matches!(i, Intent::SetLanguage(_)))
                            && screen_after == ScreenId::Language;
                    run_intents(
                        &mut session,
                        screen_after,
                        &mut state,
                        intents,
                        spdt_direction,
                        &mut out,
                        &wifi_tx,
                        &srv_tx,
                        &storage_tx,
                        &servers,
                    );
                    if router.screen_id() == ScreenId::ServerList
                        && boot_wait != BootWait::ServerConnect
                    {
                        boot_wait = BootWait::Done;
                        phase_until = None;
                    }
                    if wifi_after_lang {
                        let ssid = last_ssid_owned(&state);
                        let strings = adapter::strings_for(&state);
                        let mut cx = adapter::screen_ctx(
                            &state,
                            &mut session,
                            &env,
                            strings,
                            net_status,
                            conn,
                            server,
                            &scanned,
                            &servers,
                            battery,
                            Instant::now().as_millis(),
                        );
                        let follow =
                            adapter::begin_wifi_setup(&mut router, &mut cx, ssid.as_deref());
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
                            &mut session,
                            router.screen_id(),
                            &mut state,
                            follow,
                            spdt_direction,
                            &mut out,
                            &wifi_tx,
                            &srv_tx,
                            &storage_tx,
                            &servers,
                        );
                    }
                }
            }
            Either3::Second(sev) => {
                out.clear();
                let _ = state.apply_event(sev, &mut out);
            }
            Either3::Third(_) => {
                let now = Instant::now();
                let timed_out = phase_until.is_some_and(|t| now >= t);
                match boot_wait {
                    BootWait::Splash if timed_out => {
                        if !state.persist.language_chosen {
                            session.splash_done = true;
                            session.boot_language = true;
                            let strings = adapter::strings_for(&state);
                            let mut cx = adapter::screen_ctx(
                                &state,
                                &mut session,
                                &env,
                                strings,
                                net_status,
                                conn,
                                server,
                                &scanned,
                                &servers,
                                battery,
                                now.as_millis(),
                            );
                            let _ = router.replace_screen(ScreenId::Language, &mut cx);
                            boot_wait = BootWait::Language;
                            phase_until = None;
                        } else {
                            let ssid = last_ssid_owned(&state);
                            let strings = adapter::strings_for(&state);
                            let mut cx = adapter::screen_ctx(
                                &state,
                                &mut session,
                                &env,
                                strings,
                                net_status,
                                conn,
                                server,
                                &scanned,
                                &servers,
                                battery,
                                now.as_millis(),
                            );
                            let follow =
                                adapter::begin_wifi_setup(&mut router, &mut cx, ssid.as_deref());
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
                                &mut session,
                                router.screen_id(),
                                &mut state,
                                follow,
                                spdt_direction,
                                &mut out,
                                &wifi_tx,
                                &srv_tx,
                                &storage_tx,
                                &servers,
                            );
                        }
                    }
                    BootWait::WifiConnect if timed_out => {
                        info!("domain: Wi-Fi connect timed out");
                        let strings = adapter::strings_for(&state);
                        let mut cx = adapter::screen_ctx(
                            &state,
                            &mut session,
                            &env,
                            strings,
                            net_status,
                            conn,
                            server,
                            &scanned,
                            &servers,
                            battery,
                            now.as_millis(),
                        );
                        let _ = router.replace_screen(ScreenId::WifiFailed, &mut cx);
                        boot_wait = BootWait::WifiFailed;
                        phase_until =
                            Some(now + Duration::from_millis(config::network::WIFI_FAIL_MSG_MS));
                    }
                    BootWait::WifiFailed if timed_out => {
                        let strings = adapter::strings_for(&state);
                        let mut cx = adapter::screen_ctx(
                            &state,
                            &mut session,
                            &env,
                            strings,
                            net_status,
                            conn,
                            server,
                            &scanned,
                            &servers,
                            battery,
                            now.as_millis(),
                        );
                        let follow = router.replace_screen(ScreenId::SsidScanning, &mut cx);
                        boot_wait = BootWait::Done;
                        phase_until = None;
                        run_intents(
                            &mut session,
                            router.screen_id(),
                            &mut state,
                            follow,
                            spdt_direction,
                            &mut out,
                            &wifi_tx,
                            &srv_tx,
                            &storage_tx,
                            &servers,
                        );
                    }
                    BootWait::ServerConnect if timed_out => {
                        info!("domain: server connect timed out");
                        srv_tx.send(None);
                        let strings = adapter::strings_for(&state);
                        let mut cx = adapter::screen_ctx(
                            &state,
                            &mut session,
                            &env,
                            strings,
                            net_status,
                            conn,
                            server,
                            &scanned,
                            &servers,
                            battery,
                            now.as_millis(),
                        );
                        let follow = router.replace_screen(ScreenId::ServerList, &mut cx);
                        boot_wait = BootWait::Done;
                        phase_until = None;
                        run_intents(
                            &mut session,
                            router.screen_id(),
                            &mut state,
                            follow,
                            spdt_direction,
                            &mut out,
                            &wifi_tx,
                            &srv_tx,
                            &storage_tx,
                            &servers,
                        );
                    }
                    _ => {}
                }
                if !net::http_ota_busy()
                    && power::AUTO_SLEEP_INACTIVITY_MS > 0
                    && conn != ConnState::Connected
                    && boot_wait == BootWait::Done
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
                if s == NetStatus::Connecting {
                    saw_wifi_connecting = true;
                }
                if s == NetStatus::Ready {
                    if let Some((ssid, pw)) = adapter::take_pending_password_save(&mut session) {
                        let _ =
                            storage_tx.try_send(StorageCmd::SavePassword { ssid, password: pw });
                    }
                    if on_wifi_wizard(router.screen_id()) {
                        if let Some(saved) = state.persist.last_server {
                            srv_tx.send(Some(endpoint_from_saved(saved)));
                            let strings = adapter::strings_for(&state);
                            let mut cx = adapter::screen_ctx(
                                &state,
                                &mut session,
                                &env,
                                strings,
                                net_status,
                                conn,
                                server,
                                &scanned,
                                &servers,
                                battery,
                                Instant::now().as_millis(),
                            );
                            let _ = router.replace_screen(ScreenId::Connecting, &mut cx);
                            boot_wait = BootWait::ServerConnect;
                            phase_until = Some(
                                Instant::now()
                                    + Duration::from_millis(
                                        config::network::SERVER_CONNECTION_TIMEOUT_MS,
                                    ),
                            );
                        } else {
                            let strings = adapter::strings_for(&state);
                            let mut cx = adapter::screen_ctx(
                                &state,
                                &mut session,
                                &env,
                                strings,
                                net_status,
                                conn,
                                server,
                                &scanned,
                                &servers,
                                battery,
                                Instant::now().as_millis(),
                            );
                            let follow = router.replace_screen(ScreenId::ServerList, &mut cx);
                            boot_wait = BootWait::Done;
                            phase_until = None;
                            run_intents(
                                &mut session,
                                router.screen_id(),
                                &mut state,
                                follow,
                                spdt_direction,
                                &mut out,
                                &wifi_tx,
                                &srv_tx,
                                &storage_tx,
                                &servers,
                            );
                        }
                    } else {
                        let strings = adapter::strings_for(&state);
                        let mut cx = adapter::screen_ctx(
                            &state,
                            &mut session,
                            &env,
                            strings,
                            net_status,
                            conn,
                            server,
                            &scanned,
                            &servers,
                            battery,
                            Instant::now().as_millis(),
                        );
                        let follow = router.on_app_event(AppEvent::WifiReady, &mut cx);
                        run_intents(
                            &mut session,
                            router.screen_id(),
                            &mut state,
                            follow,
                            spdt_direction,
                            &mut out,
                            &wifi_tx,
                            &srv_tx,
                            &storage_tx,
                            &servers,
                        );
                        if router.screen_id() == ScreenId::ServerList {
                            let _ = MDNS_CTRL.try_send(());
                        }
                    }
                } else if s == NetStatus::Disconnected
                    && boot_wait == BootWait::WifiConnect
                    && saw_wifi_connecting
                {
                    let strings = adapter::strings_for(&state);
                    let mut cx = adapter::screen_ctx(
                        &state,
                        &mut session,
                        &env,
                        strings,
                        net_status,
                        conn,
                        server,
                        &scanned,
                        &servers,
                        battery,
                        Instant::now().as_millis(),
                    );
                    let _ = router.replace_screen(ScreenId::WifiFailed, &mut cx);
                    boot_wait = BootWait::WifiFailed;
                    phase_until = Some(
                        Instant::now() + Duration::from_millis(config::network::WIFI_FAIL_MSG_MS),
                    );
                }
            }
        }

        if let Some(w) = conn_rx.as_mut().and_then(|r| r.try_get()) {
            if w != conn {
                conn = w;
                if w == ConnState::Connected {
                    last_activity = Instant::now();
                    boot_wait = BootWait::Done;
                    phase_until = None;
                    let strings = adapter::strings_for(&state);
                    let mut cx = adapter::screen_ctx(
                        &state,
                        &mut session,
                        &env,
                        strings,
                        net_status,
                        conn,
                        server,
                        &scanned,
                        &servers,
                        battery,
                        Instant::now().as_millis(),
                    );
                    let follow = router.on_app_event(AppEvent::ServerConnected, &mut cx);
                    run_intents(
                        &mut session,
                        router.screen_id(),
                        &mut state,
                        follow,
                        spdt_direction,
                        &mut out,
                        &wifi_tx,
                        &srv_tx,
                        &storage_tx,
                        &servers,
                    );
                    if let Some(ep) = SERVER.sender().try_get().flatten() {
                        persist_last_server(&storage_tx, &mut state, ep);
                    }
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
            let strings = adapter::strings_for(&state);
            let mut cx = adapter::screen_ctx(
                &state,
                &mut session,
                &env,
                strings,
                net_status,
                conn,
                server,
                &scanned,
                &servers,
                battery,
                Instant::now().as_millis(),
            );
            let follow = router.on_app_event(AppEvent::ScanDone, &mut cx);
            run_intents(
                &mut session,
                router.screen_id(),
                &mut state,
                follow,
                spdt_direction,
                &mut out,
                &wifi_tx,
                &srv_tx,
                &storage_tx,
                &servers,
            );
        }

        if let Some(v) = FOUND_SERVERS.try_take() {
            servers = v;
        }

        if let Some(rec) = persist_rx.as_mut().and_then(|r| r.try_changed()) {
            apply_persist(&mut state, rec);
        }

        if let Some(b) = battery_rx.as_mut().and_then(|r| r.try_get()) {
            battery = b;
        }

        {
            let strings = adapter::strings_for(&state);
            let mut cx = adapter::screen_ctx(
                &state,
                &mut session,
                &env,
                strings,
                net_status,
                conn,
                server,
                &scanned,
                &servers,
                battery,
                Instant::now().as_millis(),
            );
            let follow = router.tick(&mut cx);
            run_intents(
                &mut session,
                router.screen_id(),
                &mut state,
                follow,
                spdt_direction,
                &mut out,
                &wifi_tx,
                &srv_tx,
                &storage_tx,
                &servers,
            );
        }

        state.flush_pending_speed(&mut out);

        adapter::publish(
            &router,
            &state,
            &mut session,
            &env,
            adapter::strings_for(&state),
            net_status,
            conn,
            server,
            &scanned,
            &servers,
            battery,
            Instant::now().as_millis(),
            &ui_tx,
        );

        flush_cmds(&cmd_tx, &mut out, &mut last_cmd).await;
    }
}
