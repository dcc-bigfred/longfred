//! Host tests for router, language, menu, keyboard idle commit, nav profiles.

use longfred_proto::action::Action;
use longfred_proto::command::Protocol;
use longfred_proto::model::{RosterEntry, ThrottleSlot, TrackPower};
use longfred_proto::network::{ConnState, NetStatus, PingStatus, WitServer};
use longfred_proto::persist::{Language, PersistRecord, StaticIpConfig};

use longfred_ui::i18n::{HintSet, strings};
use longfred_ui::input::{InputEvent, NavDir};
use longfred_ui::intent::Intent;
use longfred_ui::nav::ScreenId;
use longfred_ui::nav_profile::{LONGFRED, MARKWTECH, NavAction, NavProfile};
use longfred_ui::screen::InputMode;
use longfred_ui::view::UiView;
use longfred_ui::widgets::{KeyboardMode, TextKeyboard};
use longfred_ui::{
    DriveInfo, LAYOUT_128X32, LAYOUT_128X64, NetField, NetInfo, Router, ScreenCtx, UiEnv, UiSession,
};

struct Fixture {
    slots: [ThrottleSlot; 1],
    roster: heapless::Vec<RosterEntry, 4>,
    persist: PersistRecord,
    scanned: heapless::Vec<longfred_proto::network::SsidInfo, 60>,
    servers: heapless::Vec<longfred_proto::network::WitServer, 5>,
    session: UiSession,
    env: UiEnv,
    strings: &'static longfred_ui::i18n::Strings,
}

impl Fixture {
    fn new() -> Self {
        Self {
            slots: [ThrottleSlot::new(4)],
            roster: heapless::Vec::new(),
            persist: PersistRecord::default(),
            scanned: heapless::Vec::new(),
            servers: heapless::Vec::new(),
            session: UiSession::new(),
            env: UiEnv {
                geometry: LAYOUT_128X64,
                has_keypad: false,
                hint_set: HintSet::Joystick,
                app_name: "LongFred",
                fw_version: "0.1.0",
                fn_to_dcc: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
                hash_shows_functions: false,
                compiled_networks: &[],
                default_wit_ip: [192, 168, 4, 1],
                default_wit_port: 2560,
                default_z21_ip: [192, 168, 0, 111],
                default_z21_port: 21105,
                default_prefix_len: 24,
                board_id: "test",
                board_mcu: "host",
                battery_factor: 1.7,
            },
            strings: strings(Language::En, HintSet::Joystick),
        }
    }

    fn ctx(&mut self) -> ScreenCtx<'_> {
        ScreenCtx {
            drive: DriveInfo {
                slots: &self.slots,
                current: 0,
                roster: &self.roster,
                effective_loco_source: longfred_proto::LocoSource::AddressOnly,
                track_power: TrackPower::Unknown,
                persist: &self.persist,
                message: None,
                speed_multiplier: 1,
                max_throttles: 2,
                dead_man_switch_on: true,
                drop_before_acquire: false,
            },
            net: NetInfo {
                status: NetStatus::Disconnected,
                conn: ConnState::Disconnected,
                server: None,
                scanned_ssids: &self.scanned,
                found_servers: &self.servers,
                wifi_link: None,
                sta_net: None,
                ping: PingStatus::Idle,
                sta_ipv4: None,
                http_ota: false,
                http_ota_busy: false,
            },
            env: &self.env,
            s: self.strings,
            now_ms: 0,
            battery: None,
            session: &mut self.session,
        }
    }
}

fn overlay_first_line(view: &UiView) -> &str {
    match view {
        UiView::Overlay(ov) => ov.grid.lines.first().map_or("", |l| l.as_str()),
        _ => "",
    }
}

fn grid_line(view: &UiView, idx: usize) -> &str {
    match view {
        UiView::Grid(g) => g.lines.get(idx).map_or("", |l| l.as_str()),
        _ => "",
    }
}

#[test]
fn splash_select_goes_to_connecting() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Splash);
    assert_eq!(router.screen_id(), ScreenId::Splash);
    {
        let mut cx = fx.ctx();
        assert!(matches!(router.view(&cx), UiView::Splash));
        let _ = router.handle(InputEvent::Ok, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Connecting);
}

#[test]
fn language_select_emits_and_leaves_boot_wizard() {
    let mut fx = Fixture::new();
    fx.session.boot_language = true;
    let mut router = Router::new(&LONGFRED, ScreenId::Language);
    let intents = {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Nav(NavDir::Down), &mut cx);
        router.handle(InputEvent::Ok, &mut cx)
    };
    assert!(intents.contains(&Intent::SetLanguage(Language::Pl)));
    assert_eq!(router.screen_id(), ScreenId::Language);
    assert!(!fx.session.boot_language);
}

#[test]
fn menu_back_returns_to_throttle() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Throttle);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Menu, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Menu);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Back, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Throttle);
}

#[test]
fn menu_extras_pushes_stack() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Menu);
    {
        let mut cx = fx.ctx();
        // items are 1-based numbered: 5 = Extras (index 4)
        let _ = router.handle(InputEvent::Digit('5'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Extras);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Back, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Menu);
}

#[test]
fn keyboard_idle_commit_uses_injected_now_ms() {
    let mut kbd = TextKeyboard::<8>::new(KeyboardMode::Text);
    let _ = kbd.key_press(2, 1_000);
    let _ = kbd.key_press(2, 1_100);
    assert!(kbd.pending().is_some());
    kbd.tick(1_500);
    assert!(kbd.pending().is_some());
    kbd.tick(3_200);
    assert!(kbd.pending().is_none());
    assert_eq!(kbd.buffer.as_str(), "a");
}

#[test]
fn markwtech_star_is_cancel_off_throttle() {
    let p = MARKWTECH;
    assert_eq!(
        p.map(InputEvent::Digit('*'), InputMode::Navigation),
        NavAction::Cancel
    );
    assert_eq!(
        p.map(InputEvent::Digit('*'), InputMode::Text),
        NavAction::CaseToggle
    );
    assert_eq!(
        p.map(InputEvent::Digit('*'), InputMode::Throttle),
        NavAction::PassThrough(InputEvent::DirectionToggle)
    );
}

#[test]
fn throttle_estop_passthrough() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Throttle);
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::EStop, &mut cx)
    };
    assert!(intents.contains(&Intent::Action(Action::EStop)));
}

#[test]
fn extras_ok_opens_ip_config() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Extras);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Ok, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::IpConfig);
}

#[test]
fn extras_last_row_opens_diagnostics() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Extras);
    for _ in 0..12 {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Nav(NavDir::Down), &mut cx);
    }
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Ok, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Diagnostics);
}

#[test]
fn extras_roster_row_cycles_preference() {
    use longfred_proto::persist::RosterMode;
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Extras);
    for _ in 0..10 {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Nav(NavDir::Down), &mut cx);
    }
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::Ok, &mut cx)
    };
    assert!(intents.contains(&Intent::SetRosterMode(RosterMode::Static)));
    assert_eq!(router.screen_id(), ScreenId::Extras);
}

#[test]
fn menu_digit_opens_function_list() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Menu);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('1'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::FunctionList);
}

#[test]
fn loco_slot_emits_throttle_action() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Throttle);
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::LocoSlot(2, true), &mut cx)
    };
    assert!(intents.contains(&Intent::Action(Action::Throttle(2))));
}

#[test]
fn throttle_page_walks_catalogue() {
    use longfred_ui::nav::PageDir;
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Throttle);
    {
        let mut cx = fx.ctx();
        cx.drive.effective_loco_source = longfred_proto::LocoSource::ServerRoster;
        let intents = router.handle(InputEvent::Nav(NavDir::Right), &mut cx);
        assert!(intents.contains(&Intent::SelectLoco(PageDir::Next)));
    }
    {
        let mut cx = fx.ctx();
        let intents = router.handle(InputEvent::Nav(NavDir::Right), &mut cx);
        assert!(intents.is_empty());
    }
}

#[test]
fn throttle_title_shows_dcc_address_and_roster_name() {
    let mut fx = Fixture::new();
    let mut addr = heapless::String::new();
    let _ = addr.push_str("S8");
    let _ = fx.slots[0].consist.push(addr);
    let _ = fx.slots[0].name.push_str("SM42");

    let router = Router::new(&LONGFRED, ScreenId::Throttle);
    let cx = fx.ctx();
    let view = router.view(&cx);
    assert!(matches!(view, UiView::Throttle(_)));
    let UiView::Throttle(view) = view else {
        return;
    };
    assert_eq!(view.loco.as_str(), "8: SM42");
}

#[test]
fn throttle_title_shows_only_dcc_address_without_name() {
    let mut fx = Fixture::new();
    let mut addr = heapless::String::new();
    let _ = addr.push_str("S8");
    let _ = fx.slots[0].consist.push(addr);

    let router = Router::new(&LONGFRED, ScreenId::Throttle);
    let cx = fx.ctx();
    let view = router.view(&cx);
    assert!(matches!(view, UiView::Throttle(_)));
    let UiView::Throttle(view) = view else {
        return;
    };
    assert_eq!(view.loco.as_str(), "8");
}

#[test]
fn speed_absolute_emits_speed_set_when_loco_acquired() {
    let mut fx = Fixture::new();
    let mut addr = heapless::String::new();
    let _ = addr.push('3');
    let _ = fx.slots[0].consist.push(addr);
    let mut router = Router::new(&LONGFRED, ScreenId::Throttle);
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::SpeedAbsolute(40), &mut cx)
    };
    assert!(intents.contains(&Intent::Action(Action::SpeedSet(40))));
}

#[test]
fn wifi_scan_menu_opens_scanning_then_scan_list() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::SsidList);
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::Menu, &mut cx)
    };
    assert_eq!(router.screen_id(), ScreenId::SsidScanning);
    assert!(intents.contains(&Intent::WifiScan));
    {
        let mut cx = fx.ctx();
        let _ = router.on_app_event(longfred_ui::AppEvent::ScanDone, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::SsidScan);
    let rescan = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::Menu, &mut cx)
    };
    assert_eq!(router.screen_id(), ScreenId::SsidScanning);
    assert!(rescan.contains(&Intent::WifiScan));
}

#[test]
fn server_list_page_right_stays_on_list() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::ServerList);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Nav(NavDir::Right), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::ServerList);
}

#[test]
fn server_list_star_opens_proto_on_markwtech() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&MARKWTECH, ScreenId::ServerList);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('*'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::ServerProto);
}

#[test]
fn server_list_stop_opens_proto_without_keypad() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::ServerList);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Stop, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::ServerProto);
}

#[test]
fn server_list_stop_opens_ip_with_keypad() {
    let mut fx = Fixture::new();
    fx.env.has_keypad = true;
    let mut router = Router::new(&MARKWTECH, ScreenId::ServerList);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Stop, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::ServerEntry);
}

#[test]
fn server_list_menu_rescans_in_place() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::ServerList);
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::Menu, &mut cx)
    };
    assert_eq!(router.screen_id(), ScreenId::ServerList);
    assert!(intents.contains(&Intent::RequestMdns));
}

#[test]
fn server_list_truncates_long_label_and_keeps_glyph() {
    let mut fx = Fixture::new();
    let mut label = heapless::String::new();
    let _ = label.push_str("ABCDEFGHIJKLMNOPQRSTUVWXYZ012345");
    let _ = fx.servers.push(WitServer {
        label,
        port: 12090,
        ipv4: Some([192, 168, 1, 50]),
        protocol: Protocol::WiThrottle,
    });
    let router = Router::new(&LONGFRED, ScreenId::ServerList);
    let cx = fx.ctx();
    let view = router.view(&cx);
    assert!(matches!(view, UiView::Grid(_)));
    let UiView::Grid(g) = view else {
        return;
    };
    let joined: heapless::String<128> = {
        let mut s = heapless::String::new();
        for line in &g.lines {
            let _ = s.push_str(line.as_str());
        }
        s
    };
    assert!(
        joined.contains("ABCDEF"),
        "visible name missing in {joined:?}"
    );
    assert!(
        g.lines.iter().any(|l| l.as_str().contains('W')),
        "glyph missing in {joined:?}"
    );
}

#[test]
fn extras_server_manual_opens_proto_then_z21_port() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Extras);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Nav(NavDir::Down), &mut cx);
        let _ = router.handle(InputEvent::Ok, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::ServerProto);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Nav(NavDir::Down), &mut cx);
        let _ = router.handle(InputEvent::Ok, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::ServerEntry);
    let cx = fx.ctx();
    let view = router.view(&cx);
    assert!(matches!(view, UiView::Grid(_)));
    let UiView::Grid(g) = view else {
        return;
    };
    let joined: heapless::String<128> = {
        let mut s = heapless::String::new();
        for line in &g.lines {
            let _ = s.push_str(line.as_str());
        }
        s
    };
    assert!(
        joined.contains("21105"),
        "Z21 default port missing in {joined:?}"
    );
}

#[test]
fn keypad_strings_are_static_and_differ_from_joystick() {
    let joy = strings(Language::En, HintSet::Joystick);
    let pad = strings(Language::En, HintSet::Keypad);
    assert!(core::ptr::eq(joy, strings(Language::En, HintSet::Joystick)));
    assert!(pad.hint_enter_password.contains("*Caps"));
    assert_ne!(joy.hint_enter_password, pad.hint_enter_password);
}

#[test]
fn diagnostics_pages_wrap() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Diagnostics);
    for _ in 0..6 {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Nav(NavDir::Right), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Diagnostics);
    {
        let cx = fx.ctx();
        assert!(matches!(router.view(&cx), UiView::Grid(_)));
    }
}

#[test]
fn menu_digit_index_opens_expected_screen() {
    for (digit, want) in [
        ('1', ScreenId::FunctionList),
        ('2', ScreenId::AddrEdit),
        ('5', ScreenId::Extras),
    ] {
        let mut fx = Fixture::new();
        let mut router = Router::new(&LONGFRED, ScreenId::Menu);
        {
            let mut cx = fx.ctx();
            let _ = router.handle(InputEvent::Digit(digit), &mut cx);
        }
        assert_eq!(router.screen_id(), want, "digit {digit}");
    }
}

#[test]
fn menu_digit_two_opens_roster_when_server_source() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Menu);
    {
        let mut cx = fx.ctx();
        cx.drive.effective_loco_source = longfred_proto::LocoSource::ServerRoster;
        let _ = router.handle(InputEvent::Digit('2'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::RosterList);
}

#[test]
fn addr_edit_ok_acquires_typed_address() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::AddrEdit);
    let intents = {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('4'), &mut cx);
        let _ = router.handle(InputEvent::Digit('2'), &mut cx);
        router.handle(InputEvent::Ok, &mut cx)
    };
    assert!(intents.contains(&Intent::AcquireAddr));
    assert_eq!(fx.session.addr.as_str(), "42");
    assert_eq!(router.screen_id(), ScreenId::Throttle);
}

#[test]
fn pairing_screen_submits_six_digits_and_waits() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Pairing);
    let intents = {
        let mut cx = fx.ctx();
        for digit in "120945".chars() {
            let _ = router.handle(InputEvent::Digit(digit), &mut cx);
        }
        router.handle(InputEvent::Ok, &mut cx)
    };
    assert!(
        intents
            .iter()
            .any(|i| matches!(i, Intent::Pair(code) if code.as_str() == "120945"))
    );
    assert_eq!(router.screen_id(), ScreenId::PairingWait);
}

#[test]
fn pairing_lifecycle_routes_globally() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Throttle);
    {
        let mut cx = fx.ctx();
        let _ = router.on_app_event(longfred_ui::AppEvent::PairingRequired, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Pairing);
    {
        let mut cx = fx.ctx();
        let _ = router.on_app_event(longfred_ui::AppEvent::PairingStarted, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::PairingWait);
    {
        let mut cx = fx.ctx();
        let _ = router.on_app_event(longfred_ui::AppEvent::PairingFailed, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Pairing);
    {
        let mut cx = fx.ctx();
        let _ = router.on_app_event(longfred_ui::AppEvent::PairingSucceeded, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Throttle);
}

#[test]
fn back_with_empty_stack_goes_to_throttle() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Extras);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Back, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Throttle);
}

#[test]
fn stack_overflow_drops_oldest() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Throttle);
    let ids = [
        ScreenId::Menu,
        ScreenId::Extras,
        ScreenId::Device,
        ScreenId::Language,
        ScreenId::Diagnostics,
        ScreenId::FirmwareUpdate,
        ScreenId::IpConfig,
        ScreenId::FunctionList,
        ScreenId::RosterList,
        ScreenId::DirectCommands,
        ScreenId::Diagnostics,
    ];
    {
        let mut cx = fx.ctx();
        for id in ids {
            let _ = router.push_screen(id, &mut cx);
        }
        assert_eq!(router.stack_len(), 8);
        assert_eq!(router.screen_id(), ScreenId::Diagnostics);
        let _ = router.handle(InputEvent::Back, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::DirectCommands);
}

fn handle(router: &mut Router, fx: &mut Fixture, ev: InputEvent) -> heapless::Vec<Intent, 4> {
    let mut cx = fx.ctx();
    router.handle(ev, &mut cx)
}

#[test]
fn ip_wizard_dhcp_saves_and_returns_to_throttle() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::IpConfig);
    let _ = handle(&mut router, &mut fx, InputEvent::Ok);
    assert_eq!(router.screen_id(), ScreenId::IpEdit);
    let intents = handle(&mut router, &mut fx, InputEvent::Ok);
    assert!(
        intents
            .iter()
            .any(|i| matches!(i, Intent::SaveNetwork(StaticIpConfig { dhcp: true, .. })))
    );
    assert_eq!(router.screen_id(), ScreenId::Throttle);
}

#[test]
fn ip_wizard_static_walks_all_fields() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::IpConfig);
    let _ = handle(&mut router, &mut fx, InputEvent::Ok);
    let _ = handle(&mut router, &mut fx, InputEvent::Nav(NavDir::Left));
    let _ = handle(&mut router, &mut fx, InputEvent::Digit('1'));
    let _ = handle(&mut router, &mut fx, InputEvent::Ok);
    assert_eq!(fx.session.ip_field, NetField::Ip);
    assert_eq!(router.screen_id(), ScreenId::IpEdit);

    let _ = handle(&mut router, &mut fx, InputEvent::Ok);
    assert_eq!(fx.session.ip_field, NetField::Prefix);
    assert_eq!(fx.session.net_cfg.prefix_len, 24);
    assert_eq!(fx.session.net_cfg.gateway, Some([0, 0, 0, 1]));

    let _ = handle(&mut router, &mut fx, InputEvent::Ok);
    assert_eq!(fx.session.ip_field, NetField::Gateway);

    let _ = handle(&mut router, &mut fx, InputEvent::Ok);
    assert_eq!(fx.session.ip_field, NetField::Dns);

    let intents = handle(&mut router, &mut fx, InputEvent::Ok);
    assert!(
        intents
            .iter()
            .any(|i| matches!(i, Intent::SaveNetwork(StaticIpConfig { dhcp: false, .. })))
    );
    assert_eq!(router.screen_id(), ScreenId::Throttle);
}

#[test]
fn overlay_estop_dismisses_without_estop_intent() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Throttle);
    {
        let mut cx = fx.ctx();
        router.show_overlay("Not authorized", cx.now_ms, 5_000);
        assert!(matches!(router.view(&cx), UiView::Overlay(_)));
        let intents = router.handle(InputEvent::EStop, &mut cx);
        assert!(intents.is_empty());
        assert!(matches!(router.view(&cx), UiView::Throttle(_)));
    }
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::EStop, &mut cx)
    };
    assert!(intents.contains(&Intent::Action(Action::EStop)));
}

#[test]
fn overlay_times_out_on_tick() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Throttle);
    {
        let cx = fx.ctx();
        router.show_overlay("Net saved", 0, 5_000);
        assert!(matches!(router.view(&cx), UiView::Overlay(_)));
    }
    {
        let mut cx = fx.ctx();
        cx.now_ms = 5_000;
        let _ = router.tick(&mut cx);
        assert!(matches!(router.view(&cx), UiView::Throttle(_)));
    }
}

#[test]
fn overlay_replaces_previous_message() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Throttle);
    let cx = fx.ctx();
    router.show_overlay("first", 0, 5_000);
    router.show_overlay("second", 0, 5_000);
    assert_eq!(overlay_first_line(&router.view(&cx)), "second");
}

#[test]
fn menu_speed_mult_shows_overlay_and_returns_to_throttle() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Menu);
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::Digit('3'), &mut cx)
    };
    assert!(intents.contains(&Intent::Action(Action::SpeedMultiplier)));
    assert_eq!(router.screen_id(), ScreenId::Throttle);
    let cx = fx.ctx();
    assert_eq!(overlay_first_line(&router.view(&cx)), "Speed x2");
}

#[test]
fn menu_power_shows_overlay_on() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Menu);
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::Digit('4'), &mut cx)
    };
    assert!(intents.contains(&Intent::Action(Action::PowerToggle)));
    let cx = fx.ctx();
    assert_eq!(overlay_first_line(&router.view(&cx)), "Trk power ON");
}

#[test]
fn extras_dead_man_switch_shows_overlay_and_returns_to_throttle() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Extras);
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::Digit('3'), &mut cx)
    };
    assert!(intents.contains(&Intent::DeadManSwitchToggle));
    assert_eq!(router.screen_id(), ScreenId::Throttle);
    let cx = fx.ctx();
    assert_eq!(overlay_first_line(&router.view(&cx)), "Dead-man OFF");
}

#[test]
fn extras_digit_shortcuts_match_label_numbers() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Extras);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('5'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Throttle);
    let view = router.view(&fx.ctx());
    let overlay = overlay_first_line(&view);
    assert!(
        overlay.contains("Throttles"),
        "expected throttles minus overlay, got {overlay}"
    );
    fx = Fixture::new();
    router = Router::new(&LONGFRED, ScreenId::Extras);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('6'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Language);
}

#[test]
fn menu_pl_footer_keeps_extras_visible_on_64() {
    let mut fx = Fixture::new();
    fx.strings = strings(Language::Pl, HintSet::Joystick);
    let router = Router::new(&LONGFRED, ScreenId::Menu);
    let view = router.view(&fx.ctx());
    assert_eq!(grid_line(&view, 5), "5:Dodatkowe");
    assert_eq!(grid_line(&view, 6), "Nav OK  Fn+cyfry  Wst");
    assert!(grid_line(&view, 1).starts_with("1:"));
    assert!(grid_line(&view, 4).starts_with("4:"));
}

#[test]
fn menu_pl_footer_pages_on_32_and_digit_five_selects_extras() {
    let mut fx = Fixture::new();
    fx.env.geometry = LAYOUT_128X32;
    fx.strings = strings(Language::Pl, HintSet::Joystick);
    let mut router = Router::new(&LONGFRED, ScreenId::Menu);
    let view = router.view(&fx.ctx());
    assert!(grid_line(&view, 1).starts_with("1:"));
    assert!(grid_line(&view, 2).starts_with("2:"));
    assert_eq!(grid_line(&view, 3), "Nav OK  Fn+cyfry  Wst");
    assert!(!grid_line(&view, 1).contains("Dodatkowe"));
    assert!(!grid_line(&view, 2).contains("Dodatkowe"));
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('5'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Extras);
}

#[test]
fn extras_no_footer_three_rows_on_32_and_digit_six_opens_language() {
    let mut fx = Fixture::new();
    fx.env.geometry = LAYOUT_128X32;
    let mut router = Router::new(&LONGFRED, ScreenId::Extras);
    let view = router.view(&fx.ctx());
    assert!(!grid_line(&view, 1).is_empty());
    assert!(!grid_line(&view, 2).is_empty());
    assert!(!grid_line(&view, 3).is_empty());
    assert!(grid_line(&view, 4).is_empty());
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('6'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Language);
}
