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
    BatteryInfo, DriveInfo, LAYOUT_128X32, LAYOUT_128X64, NetField, NetInfo, Router, ScreenCtx,
    UiEnv, UiSession,
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
    conn: ConnState,
    net_status: NetStatus,
    battery: Option<BatteryInfo>,
    battery_history: heapless::Vec<u8, 30>,
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
            conn: ConnState::Disconnected,
            net_status: NetStatus::Disconnected,
            battery: None,
            battery_history: heapless::Vec::new(),
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
            },
            net: NetInfo {
                status: self.net_status,
                conn: self.conn,
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
            battery: self.battery,
            battery_history: &self.battery_history,
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

fn overlay_joined(view: &UiView) -> heapless::String<64> {
    let mut s = heapless::String::new();
    if let UiView::Overlay(ov) = view {
        for line in &ov.grid.lines {
            let _ = s.push_str(line.as_str());
        }
    }
    s
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
        // items are 1-based numbered: 6 = Extras (index 5)
        let _ = router.handle(InputEvent::Digit('6'), &mut cx);
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
        NavAction::Digit('*')
    );
    assert_eq!(
        p.map(InputEvent::Digit('#'), InputMode::Throttle),
        NavAction::PassThrough(InputEvent::DirectionToggle)
    );
    assert_eq!(
        p.map(InputEvent::Ok, InputMode::Throttle),
        NavAction::Select
    );
    assert_eq!(
        p.map(InputEvent::Digit('#'), InputMode::Navigation),
        NavAction::Select
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
    assert_eq!(router.screen_id(), ScreenId::Throttle);
}

fn acquire_test_loco(fx: &mut Fixture) {
    let mut addr = heapless::String::new();
    let _ = addr.push('3');
    let _ = fx.slots[0].consist.push(addr);
}

#[test]
fn throttle_hash_toggles_direction_and_ok_opens_direct() {
    let mut fx = Fixture::new();
    acquire_test_loco(&mut fx);
    let mut router = Router::new(&MARKWTECH, ScreenId::Throttle);
    let hash = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::Digit('#'), &mut cx)
    };
    assert!(hash.contains(&Intent::Action(Action::DirectionToggle)));
    assert_eq!(router.screen_id(), ScreenId::Throttle);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Ok, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::DirectCommands);
}

#[test]
fn throttle_star_types_function_above_nine() {
    let mut fx = Fixture::new();
    acquire_test_loco(&mut fx);
    let mut router = Router::new(&MARKWTECH, ScreenId::Throttle);
    {
        let mut cx = fx.ctx();
        let empty = router.handle(InputEvent::Digit('*'), &mut cx);
        assert!(empty.is_empty());
        let _ = router.handle(InputEvent::Digit('1'), &mut cx);
        let _ = router.handle(InputEvent::Digit('5'), &mut cx);
        let UiView::Throttle(view) = router.view(&cx) else {
            panic!("expected throttle HUD");
        };
        assert_eq!(view.footer.as_str(), "*15");
    }
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::Digit('*'), &mut cx)
    };
    assert!(intents.contains(&Intent::Function(15)));
    assert_eq!(router.screen_id(), ScreenId::Throttle);
}

#[test]
fn throttle_digit_without_star_toggles_single_function() {
    let mut fx = Fixture::new();
    acquire_test_loco(&mut fx);
    let mut router = Router::new(&MARKWTECH, ScreenId::Throttle);
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::Digit('3'), &mut cx)
    };
    assert!(intents.contains(&Intent::Function(3)));
}

#[test]
fn throttle_star_star_without_digits_cancels() {
    let mut fx = Fixture::new();
    acquire_test_loco(&mut fx);
    let mut router = Router::new(&MARKWTECH, ScreenId::Throttle);
    let intents = {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('*'), &mut cx);
        router.handle(InputEvent::Digit('*'), &mut cx)
    };
    assert!(intents.is_empty());
    assert_eq!(router.screen_id(), ScreenId::Throttle);
}

fn assert_connect_aborted(router: &Router, intents: &[Intent], estop: bool) {
    assert_eq!(router.screen_id(), ScreenId::Menu);
    assert_eq!(router.stack_len(), 0);
    assert!(intents.contains(&Intent::AbortConnect));
    assert_eq!(intents.contains(&Intent::Action(Action::EStop)), estop);
}

#[test]
fn connecting_estop_roots_menu_and_aborts_wizard() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Connecting);
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::EStop, &mut cx)
    };
    assert_connect_aborted(&router, &intents, true);
}

#[test]
fn connecting_stop_and_back_root_menu_without_estop() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Connecting);
    let stop = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::Stop, &mut cx)
    };
    assert_connect_aborted(&router, &stop, false);

    let mut router = Router::new(&LONGFRED, ScreenId::Connecting);
    let back = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::Back, &mut cx)
    };
    assert_connect_aborted(&router, &back, false);
}

#[test]
fn connecting_menu_key_roots_menu() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Connecting);
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::Menu, &mut cx)
    };
    assert_connect_aborted(&router, &intents, false);
}

#[test]
fn connecting_overlay_estop_roots_menu() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Connecting);
    let intents = {
        let mut cx = fx.ctx();
        router.show_overlay("Laczenie dluzsze...", cx.now_ms, 5_000);
        assert!(matches!(router.view(&cx), UiView::Overlay(_)));
        router.handle(InputEvent::EStop, &mut cx)
    };
    assert_connect_aborted(&router, &intents, false);
    {
        let cx = fx.ctx();
        assert!(!matches!(router.view(&cx), UiView::Overlay(_)));
    }
}

#[test]
fn connecting_markwtech_stop_is_estop_and_aborts() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&MARKWTECH, ScreenId::Connecting);
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::EStop, &mut cx)
    };
    assert_connect_aborted(&router, &intents, true);
}

#[test]
fn extras_ok_opens_device() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Extras);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Ok, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Device);
}

#[test]
fn extras_last_row_opens_diagnostics() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Extras);
    for _ in 0..8 {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Nav(NavDir::Down), &mut cx);
    }
    {
        let cx = fx.ctx();
        let view = router.view(&cx);
        assert!(
            grid_line(&view, 3).starts_with("9:"),
            "diagnostics row should be numbered 9, got {}",
            grid_line(&view, 3)
        );
        assert!(
            grid_line(&view, 3).contains("Diagnostics"),
            "got {}",
            grid_line(&view, 3)
        );
    }
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Ok, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Diagnostics);
}

#[test]
fn extras_digit_one_opens_device() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Extras);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('1'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Device);
}

#[test]
fn extras_slot_count_saves_and_overlays() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Extras);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('4'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::SlotCountEdit);
    let intents = {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('3'), &mut cx);
        router.handle(InputEvent::Ok, &mut cx)
    };
    assert!(intents.contains(&Intent::Action(Action::SetMaxThrottles(3))));
    assert_eq!(router.screen_id(), ScreenId::Extras);
    let joined = overlay_joined(&router.view(&fx.ctx()));
    assert!(
        joined.contains('3') && joined.contains("locos"),
        "overlay missing slot count: {joined}"
    );
}

#[test]
fn extras_slot_count_rejects_zero() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Extras);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('4'), &mut cx);
    }
    let intents = {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('0'), &mut cx);
        router.handle(InputEvent::Ok, &mut cx)
    };
    assert!(intents.is_empty());
    assert_eq!(router.screen_id(), ScreenId::SlotCountEdit);
}

#[test]
fn extras_star_nine_opens_diagnostics() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&MARKWTECH, ScreenId::Extras);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('*'), &mut cx);
        let _ = router.handle(InputEvent::Digit('9'), &mut cx);
        let view = router.view(&cx);
        assert!(
            grid_line(&view, 0).contains('*'),
            "index-entry marker missing: {}",
            grid_line(&view, 0)
        );
        let _ = router.handle(InputEvent::Digit('*'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Diagnostics);
}

#[test]
fn extras_fn_nine_opens_diagnostics() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Extras);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::FnPress(9), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Diagnostics);
}

#[test]
fn extras_star_then_back_stays_on_extras() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&MARKWTECH, ScreenId::Extras);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('*'), &mut cx);
        let _ = router.handle(InputEvent::Back, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Extras);
}

#[test]
fn extras_roster_opens_choice_static() {
    use longfred_proto::persist::RosterMode;
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Extras);
    for _ in 0..6 {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Nav(NavDir::Down), &mut cx);
    }
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Ok, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Choice);
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::Digit('2'), &mut cx)
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
fn throttle_hides_list_index_in_address_only_mode() {
    let mut fx = Fixture::new();
    let mut addr = heapless::String::new();
    let _ = addr.push_str("S8");
    let _ = fx.slots[0].consist.push(addr);
    let router = Router::new(&LONGFRED, ScreenId::Throttle);
    let cx = fx.ctx();
    let view = router.view(&cx);
    let UiView::Throttle(view) = view else {
        return;
    };
    assert_eq!(view.list_index, None);
}

#[test]
fn throttle_shows_list_index_from_roster() {
    let mut fx = Fixture::new();
    let mut name = heapless::String::new();
    let _ = name.push_str("Vectron");
    let _ = fx.roster.push(RosterEntry {
        name,
        address: 431,
        length: 'L',
    });
    let mut name = heapless::String::new();
    let _ = name.push_str("SM42");
    let _ = fx.roster.push(RosterEntry {
        name,
        address: 955,
        length: 'L',
    });
    let mut addr = heapless::String::new();
    let _ = addr.push_str("L431");
    let _ = fx.slots[0].consist.push(addr);
    fx.slots[0].list_idx = Some(0);
    let router = Router::new(&LONGFRED, ScreenId::Throttle);
    let view = {
        let mut cx = fx.ctx();
        cx.drive.effective_loco_source = longfred_proto::LocoSource::ServerRoster;
        router.view(&cx)
    };
    let UiView::Throttle(view) = view else {
        return;
    };
    assert_eq!(view.list_index, Some((1, 2)));

    fx.slots[0].list_idx = Some(1);
    let view = {
        let mut cx = fx.ctx();
        cx.drive.effective_loco_source = longfred_proto::LocoSource::ServerRoster;
        router.view(&cx)
    };
    let UiView::Throttle(view) = view else {
        return;
    };
    assert_eq!(view.list_index, Some((2, 2)));
}

#[test]
fn throttle_server_connected_follows_conn_state() {
    let mut fx = Fixture::new();
    let router = Router::new(&LONGFRED, ScreenId::Throttle);
    {
        let cx = fx.ctx();
        let view = router.view(&cx);
        assert!(matches!(view, UiView::Throttle(_)));
        let UiView::Throttle(view) = view else {
            return;
        };
        assert_eq!(view.conn, ConnState::Disconnected);
        assert!(!view.conn_busy);
    }
    fx.conn = ConnState::Connecting;
    {
        let cx = fx.ctx();
        let UiView::Throttle(view) = router.view(&cx) else {
            return;
        };
        assert_eq!(view.conn, ConnState::Connecting);
        assert!(view.conn_busy);
    }
    fx.conn = ConnState::Connected;
    {
        let cx = fx.ctx();
        let view = router.view(&cx);
        assert!(matches!(view, UiView::Throttle(_)));
        let UiView::Throttle(view) = view else {
            return;
        };
        assert_eq!(view.conn, ConnState::Connected);
        assert!(!view.conn_busy);
    }
    fx.net_status = NetStatus::Connecting;
    {
        let cx = fx.ctx();
        let UiView::Throttle(view) = router.view(&cx) else {
            return;
        };
        assert_eq!(view.conn, ConnState::Connected);
        assert!(view.conn_busy);
    }
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
fn server_list_star_stays_on_list() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&MARKWTECH, ScreenId::ServerList);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('*'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::ServerList);
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
        layout_name: heapless::String::new(),
        host: heapless::String::new(),
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
fn server_list_bigfred_uses_layout_name_with_protocol_mark() {
    let mut fx = Fixture::new();
    let mut label = heapless::String::new();
    let _ = label.push_str("BigFred #5");
    let mut layout_name = heapless::String::new();
    let _ = layout_name.push_str("Klubowa");
    let _ = fx.servers.push(WitServer {
        label: label.clone(),
        layout_name: layout_name.clone(),
        host: heapless::String::new(),
        port: 12090,
        ipv4: Some([192, 168, 1, 50]),
        protocol: Protocol::BigFred,
    });
    let _ = fx.servers.push(WitServer {
        label,
        layout_name,
        host: heapless::String::new(),
        port: 21105,
        ipv4: Some([192, 168, 1, 50]),
        protocol: Protocol::Z21,
    });
    let router = Router::new(&LONGFRED, ScreenId::ServerList);
    let cx = fx.ctx();
    let view = router.view(&cx);
    let UiView::Grid(g) = view else {
        panic!("expected grid");
    };
    let joined: heapless::String<128> = {
        let mut s = heapless::String::new();
        for line in &g.lines {
            let _ = s.push_str(line.as_str());
        }
        s
    };
    assert!(
        joined.contains("Klubowa/BigFred B"),
        "BigFred row missing in {joined:?}"
    );
    assert!(
        joined.contains("Klubowa/BigFred Z21"),
        "Z21 row missing in {joined:?}"
    );
    let b = joined.find("Klubowa/BigFred B").expect("B row");
    let z = joined.find("Klubowa/BigFred Z21").expect("Z21 row");
    assert!(b < z, "B must be listed above Z21 in {joined:?}");
}

fn push_test_server(fx: &mut Fixture, label: &str, host: &str, proto: Protocol) {
    let mut l = heapless::String::new();
    let _ = l.push_str(label);
    let mut h = heapless::String::new();
    let _ = h.push_str(host);
    let mut layout_name = heapless::String::new();
    let _ = layout_name.push_str("Domowa");
    let _ = fx.servers.push(WitServer {
        label: l,
        layout_name,
        host: h,
        port: 12090,
        ipv4: Some([192, 168, 1, 50]),
        protocol: proto,
    });
}

#[test]
fn server_list_ok_opens_confirm_back_returns() {
    let mut fx = Fixture::new();
    push_test_server(&mut fx, "BigFred #2", "bigfred", Protocol::BigFred);
    let mut router = Router::new(&LONGFRED, ScreenId::ServerList);
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::Ok, &mut cx)
    };
    assert_eq!(router.screen_id(), ScreenId::ServerConfirm);
    assert!(!intents.iter().any(|i| matches!(i, Intent::ServerSelect(_))));
    {
        let cx = fx.ctx();
        let view = router.view(&cx);
        let UiView::Grid(g) = view else {
            panic!("expected grid");
        };
        let joined: heapless::String<128> = {
            let mut s = heapless::String::new();
            for line in &g.lines {
                let _ = s.push_str(line.as_str());
            }
            s
        };
        assert!(
            joined.contains("Domowa/BigFred B"),
            "name missing in {joined:?}"
        );
        assert!(joined.contains("BigFred"), "protocol missing in {joined:?}");
        assert!(
            joined.contains("bigfred.local:12090"),
            "dns addr missing in {joined:?}"
        );
    }
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Back, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::ServerList);
}

#[test]
fn server_confirm_menu_connects() {
    let mut fx = Fixture::new();
    push_test_server(&mut fx, "BigFred #2", "bigfred", Protocol::BigFred);
    let mut router = Router::new(&LONGFRED, ScreenId::ServerList);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Ok, &mut cx);
    }
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::Menu, &mut cx)
    };
    assert_eq!(router.screen_id(), ScreenId::Throttle);
    assert!(intents.contains(&Intent::ServerSelect(0)));
}

#[test]
fn server_confirm_ip_when_host_missing() {
    let mut fx = Fixture::new();
    push_test_server(&mut fx, "RB1110", "", Protocol::WiThrottle);
    fx.servers[0].layout_name.clear();
    let mut router = Router::new(&LONGFRED, ScreenId::ServerList);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Ok, &mut cx);
    }
    let cx = fx.ctx();
    let view = router.view(&cx);
    let UiView::Grid(g) = view else {
        panic!("expected grid");
    };
    let joined: heapless::String<128> = {
        let mut s = heapless::String::new();
        for line in &g.lines {
            let _ = s.push_str(line.as_str());
        }
        s
    };
    assert!(joined.contains("WiThrottle"), "proto missing in {joined:?}");
    assert!(
        joined.contains("192.168.1.50:12090"),
        "ip addr missing in {joined:?}"
    );
}

#[test]
fn server_menu_manual_opens_proto_then_z21_port() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Menu);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('3'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::ServerMenu);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('3'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Choice);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('2'), &mut cx);
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
fn server_proto_digits_select_wit_and_z21() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::ServerProto);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('1'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::ServerEntry);
    assert_eq!(fx.session.manual_protocol, Protocol::Z21);
    fx = Fixture::new();
    router = Router::new(&LONGFRED, ScreenId::ServerProto);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('0'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::ServerEntry);
    assert_eq!(fx.session.manual_protocol, Protocol::WiThrottle);
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
    {
        let cx = fx.ctx();
        assert!(grid_line(&router.view(&cx), 0).contains("Battery"));
    }
    for _ in 0..3 {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Nav(NavDir::Right), &mut cx);
    }
    {
        let cx = fx.ctx();
        let view = router.view(&cx);
        assert!(
            grid_line(&view, 0).contains("range") || grid_line(&view, 0).contains("WiFi"),
            "RF+ping page title: {}",
            grid_line(&view, 0)
        );
        assert!(
            grid_line(&view, 1).contains("---") || grid_line(&view, 2).contains("---"),
            "range/ping body: {} / {}",
            grid_line(&view, 1),
            grid_line(&view, 2)
        );
    }
    for _ in 0..4 {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Nav(NavDir::Right), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Diagnostics);
    {
        let cx = fx.ctx();
        assert!(grid_line(&router.view(&cx), 0).contains("Battery"));
    }
}

#[test]
fn diagnostics_battery_page_shows_pack_and_suggested() {
    let mut fx = Fixture::new();
    fx.battery = Some(BatteryInfo {
        percent: 50,
        millivolts: 3700,
        raw: 2144,
        charging: false,
    });
    let router = Router::new(&LONGFRED, ScreenId::Diagnostics);
    let view = router.view(&fx.ctx());
    let body: String = (0..8)
        .map(|i| grid_line(&view, i))
        .collect::<Vec<_>>()
        .join("|");
    assert!(body.contains("50%"), "{body}");
    assert!(body.contains("3700 mV"), "{body}");
    assert!(body.contains("ADC 2144"), "{body}");
    assert!(body.contains("sug 1.96"), "{body}");
}

#[test]
fn diagnostics_rssi_and_ping_chart_pages() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Diagnostics);
    for _ in 0..5 {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Nav(NavDir::Right), &mut cx);
    }
    {
        let cx = fx.ctx();
        let view = router.view(&cx);
        assert!(matches!(view, UiView::Chart(_)));
        let UiView::Chart(chart) = view else {
            return;
        };
        assert_eq!(chart.title, "WiFi signal");
        assert_eq!(chart.threshold, None);
    }
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Nav(NavDir::Right), &mut cx);
    }
    {
        let cx = fx.ctx();
        let view = router.view(&cx);
        assert!(matches!(view, UiView::Chart(_)));
        let UiView::Chart(chart) = view else {
            return;
        };
        assert_eq!(chart.title, "WiFi ping");
        assert_eq!(chart.threshold, Some(50));
        assert_eq!(chart.y_max, 250);
    }
}

#[test]
fn menu_digit_index_opens_expected_screen() {
    for (digit, want) in [
        ('1', ScreenId::FunctionList),
        ('2', ScreenId::AddrEdit),
        ('3', ScreenId::ServerMenu),
        ('6', ScreenId::Extras),
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
        ScreenId::WifiSettings,
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
    let mut router = Router::new(&LONGFRED, ScreenId::WifiSettings);
    let _ = handle(&mut router, &mut fx, InputEvent::Digit('2'));
    assert_eq!(router.screen_id(), ScreenId::Choice);
    let intents = handle(&mut router, &mut fx, InputEvent::Digit('1'));
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
    let mut router = Router::new(&LONGFRED, ScreenId::WifiSettings);
    let _ = handle(&mut router, &mut fx, InputEvent::Digit('2'));
    let _ = handle(&mut router, &mut fx, InputEvent::Digit('2'));
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
fn overlay_estop_with_loco_emits_estop_and_dismisses() {
    let mut fx = Fixture::new();
    let mut addr = heapless::String::new();
    let _ = addr.push('3');
    let _ = fx.slots[0].consist.push(addr);
    let mut router = Router::new(&LONGFRED, ScreenId::Throttle);
    let intents = {
        let mut cx = fx.ctx();
        router.show_overlay("Net saved", cx.now_ms, 5_000);
        assert!(matches!(router.view(&cx), UiView::Overlay(_)));
        router.handle(InputEvent::EStop, &mut cx)
    };
    assert!(intents.contains(&Intent::Action(Action::EStop)));
    {
        let cx = fx.ctx();
        assert!(matches!(router.view(&cx), UiView::Throttle(_)));
    }
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
fn overlay_skips_empty_and_whitespace() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Throttle);
    let cx = fx.ctx();
    router.show_overlay("", 0, 5_000);
    assert!(matches!(router.view(&cx), UiView::Throttle(_)));
    router.show_overlay("   ", 0, 5_000);
    assert!(matches!(router.view(&cx), UiView::Throttle(_)));
}

#[test]
fn menu_speed_mult_shows_overlay_and_returns_to_throttle() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Menu);
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::Digit('4'), &mut cx)
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
        router.handle(InputEvent::Digit('5'), &mut cx)
    };
    assert!(intents.contains(&Intent::Action(Action::PowerToggle)));
    let cx = fx.ctx();
    assert_eq!(overlay_first_line(&router.view(&cx)), "Trk power ON");
}

#[test]
fn extras_dead_man_opens_choice_and_off_toggles() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Extras);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('3'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Choice);
    {
        let cx = fx.ctx();
        let view = router.view(&cx);
        assert_eq!(grid_line(&view, 0), "Dead-man");
        assert!(grid_line(&view, 1).starts_with("1:"));
        assert!(grid_line(&view, 2).starts_with("2:"));
        assert!(grid_line(&view, 3).is_empty());
    }
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::Digit('2'), &mut cx)
    };
    assert!(intents.contains(&Intent::DeadManSwitchToggle));
    assert_eq!(router.screen_id(), ScreenId::Extras);
}

#[test]
fn extras_digit_shortcuts_match_label_numbers() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Extras);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('3'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Choice);
    fx = Fixture::new();
    router = Router::new(&LONGFRED, ScreenId::Extras);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('4'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::SlotCountEdit);
    fx = Fixture::new();
    router = Router::new(&LONGFRED, ScreenId::Extras);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('6'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Language);
}

#[test]
fn extras_digit_zero_opens_battery() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Extras);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('0'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Battery);
    let view = router.view(&fx.ctx());
    assert!(matches!(view, UiView::Chart(_)));
}

#[test]
fn extras_fn_zero_opens_battery() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Extras);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::FnPress(0), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Battery);
}

#[test]
fn battery_chart_shows_eta_and_volts() {
    let mut fx = Fixture::new();
    fx.battery = Some(BatteryInfo {
        percent: 50,
        millivolts: 3850,
        raw: 2000,
        charging: false,
    });
    let _ = fx.battery_history.push(100);
    let _ = fx.battery_history.push(50);
    let router = Router::new(&LONGFRED, ScreenId::Battery);
    let view = router.view(&fx.ctx());
    assert!(matches!(view, UiView::Chart(_)));
    let UiView::Chart(chart) = view else {
        return;
    };
    assert_eq!(chart.title, "Battery");
    assert_eq!(chart.samples.as_slice(), &[100, 50]);
    assert_eq!(chart.footer[0].as_str(), "1m");
    assert_eq!(chart.footer[1].as_str(), "3.85 V");
}

#[test]
fn battery_left_returns_to_extras() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Extras);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('0'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Battery);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Nav(NavDir::Left), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Extras);
}

#[test]
fn menu_pl_footer_keeps_extras_visible_on_64() {
    let mut fx = Fixture::new();
    fx.strings = strings(Language::Pl, HintSet::Joystick);
    let mut router = Router::new(&LONGFRED, ScreenId::Menu);
    let view = router.view(&fx.ctx());
    assert_eq!(grid_line(&view, 5), "5:Zasilanie");
    assert_eq!(grid_line(&view, 6), "Nav OK  Fn+cyfry  Wst");
    assert!(grid_line(&view, 1).starts_with("1:"));
    assert!(grid_line(&view, 4).starts_with("4:"));
    for _ in 0..5 {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Nav(NavDir::Down), &mut cx);
    }
    let view = router.view(&fx.ctx());
    assert_eq!(grid_line(&view, 1), "6:Dodatkowe");
    assert_eq!(grid_line(&view, 6), "Nav OK  Fn+cyfry  Wst");
}

#[test]
fn menu_pl_footer_pages_on_32_and_digit_five_selects_power() {
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
    assert_eq!(router.screen_id(), ScreenId::Throttle);
    fx = Fixture::new();
    fx.env.geometry = LAYOUT_128X32;
    fx.strings = strings(Language::Pl, HintSet::Joystick);
    router = Router::new(&LONGFRED, ScreenId::Menu);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('6'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Extras);
}

#[test]
fn extras_no_footer_three_rows_on_32_and_digit_zero_opens_language() {
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

#[test]
fn server_menu_reconnect_without_saved_shows_overlay() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::Menu);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('3'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::ServerMenu);
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::Digit('1'), &mut cx)
    };
    assert!(intents.is_empty());
    assert_eq!(router.screen_id(), ScreenId::ServerMenu);
    let cx = fx.ctx();
    assert_eq!(overlay_first_line(&router.view(&cx)), "No saved server");
}

#[test]
fn server_menu_reconnect_emits_intent_when_saved() {
    use longfred_proto::persist::SavedServer;
    let mut fx = Fixture::new();
    fx.persist.last_server = Some(SavedServer {
        ip: [192, 168, 4, 1],
        port: 2560,
        protocol: Protocol::WiThrottle,
    });
    let mut router = Router::new(&LONGFRED, ScreenId::ServerMenu);
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::Digit('1'), &mut cx)
    };
    assert!(intents.contains(&Intent::ServerReconnect));
    assert_eq!(router.screen_id(), ScreenId::ServerMenu);
}

#[test]
fn server_menu_change_find_opens_server_list() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::ServerMenu);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('3'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Choice);
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::Digit('1'), &mut cx)
    };
    assert!(intents.contains(&Intent::RequestMdns));
    assert_eq!(router.screen_id(), ScreenId::ServerList);
}

#[test]
fn server_menu_pair_and_disconnect() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::ServerMenu);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('2'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Pairing);
    fx = Fixture::new();
    router = Router::new(&LONGFRED, ScreenId::ServerMenu);
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::Digit('4'), &mut cx)
    };
    assert!(intents.contains(&Intent::ServerDisconnect));
    assert_eq!(router.screen_id(), ScreenId::ServerMenu);
}

#[test]
fn server_menu_wifi_settings_opens_submenu() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::ServerMenu);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('5'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::WifiSettings);
    let cx = fx.ctx();
    let view = router.view(&cx);
    assert!(grid_line(&view, 0).contains("WiFi"));
    assert!(grid_line(&view, 1).contains("Search"));
    assert!(grid_line(&view, 2).contains("Address"));
}

#[test]
fn wifi_settings_search_starts_scan() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::WifiSettings);
    let intents = {
        let mut cx = fx.ctx();
        router.handle(InputEvent::Digit('1'), &mut cx)
    };
    assert_eq!(router.screen_id(), ScreenId::SsidScanning);
    assert!(intents.contains(&Intent::WifiScan));
    assert!(fx.session.wifi_from_settings);
    {
        let mut cx = fx.ctx();
        let _ = router.on_app_event(longfred_ui::AppEvent::ScanDone, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::SsidScan);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Back, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::WifiSettings);
}

#[test]
fn wifi_scan_back_stays_on_boot_without_compiled_ssids() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::SsidScanning);
    {
        let mut cx = fx.ctx();
        let _ = router.on_app_event(longfred_ui::AppEvent::ScanDone, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::SsidScan);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Back, &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::SsidScan);
}

#[test]
fn wifi_settings_address_opens_dhcp_static() {
    let mut fx = Fixture::new();
    let mut router = Router::new(&LONGFRED, ScreenId::WifiSettings);
    {
        let mut cx = fx.ctx();
        let _ = router.handle(InputEvent::Digit('2'), &mut cx);
    }
    assert_eq!(router.screen_id(), ScreenId::Choice);
    let cx = fx.ctx();
    let view = router.view(&cx);
    assert!(grid_line(&view, 0).contains("Address"));
    assert!(grid_line(&view, 1).contains("DHCP"));
    assert!(grid_line(&view, 2).contains("Static"));
}
