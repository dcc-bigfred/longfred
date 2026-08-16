//! Joystick / tact-switch navigation handlers for MenuFsm.

use longfred_proto::command::Protocol;
use longfred_proto::mdns::WitServer;
use longfred_proto::persist::{DEVICE_ID_MAX, DEVICE_ID_MIN, Language};

use crate::config::{self, buttons, sizes};
use crate::domain::actions::Action;
use crate::domain::state::DomainState;
use crate::input::{InputEvent, NavDir};
use crate::net::SsidInfo;
use crate::ui::keyboard::KeyboardMode;
use crate::ui::menu::{DIAG_PAGES, Intent, MenuFsm, Screen};
use crate::ui::nav_profile::{self, NavAction, NavProfile};
use crate::ui::view::{
    Line, digit_display_num, has_next_page, item_display_num, page_item_count, page_start,
};

impl MenuFsm {
    pub fn handle_input(
        &mut self,
        ev: InputEvent,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) -> Intent {
        let text_entry = self.is_text_entry_screen();
        let on_throttle = self.screen == Screen::Throttle;
        let profile = nav_profile::active();
        let Some(action) = profile.map(ev, text_entry, on_throttle) else {
            return Intent::None;
        };
        match action {
            NavAction::ListPrev => self.on_list_step(NavDir::Up, domain, scanned, servers),
            NavAction::ListNext => self.on_list_step(NavDir::Down, domain, scanned, servers),
            NavAction::Select => self.on_ok(domain, scanned, servers),
            NavAction::Cancel => self.on_back(domain),
            NavAction::MenuEnter => self.on_menu_enter(domain, scanned, servers),
            NavAction::CharCycle(d) => self.on_char_cycle(d, domain),
            NavAction::CursorMove(d) => self.on_cursor_move(d, domain),
            NavAction::CaseToggle => self.on_case_toggle(),
            NavAction::Digit(c) => self.on_digit(c, domain, scanned, servers),
            NavAction::PagePrev => self.on_page_prev(domain, scanned, servers),
            NavAction::PageNext => self.on_page_next(domain, scanned, servers),
            NavAction::PassThrough(ev) => self.handle_passthrough(ev, domain, scanned, servers),
        }
    }

    fn handle_passthrough(
        &mut self,
        ev: InputEvent,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) -> Intent {
        if let Some(intent) = self.handle_global(ev, domain) {
            return intent;
        }
        match ev {
            InputEvent::Nav(dir) => self.on_nav(dir, domain, scanned, servers),
            InputEvent::Ok => self.on_ok(domain, scanned, servers),
            InputEvent::Back => self.on_back(domain),
            InputEvent::Menu => self.on_menu_key(domain),
            InputEvent::FnPress(k) => self.on_fn_press(k, domain, scanned, servers),
            InputEvent::FnRelease(_) => Intent::None,
            InputEvent::EncoderClockwise => self.encoder(true, domain),
            InputEvent::EncoderCounterClockwise => self.encoder(false, domain),
            InputEvent::EncoderButton => self.encoder_button(domain),
            InputEvent::Stop if self.screen == Screen::Diagnostics => {
                self.list.page = (self.list.page + 1) % DIAG_PAGES;
                Intent::None
            }
            InputEvent::Stop => self.on_back(domain),
            InputEvent::EnterProgrammingMode => Intent::EnterProgrammingMode,
            InputEvent::EStop
            | InputEvent::DirectionSet(_)
            | InputEvent::DirectionToggle
            | InputEvent::Digit(_)
            | InputEvent::SpeedAbsolute(_)
            | InputEvent::LocoSlot(_, _)
            | InputEvent::CharCycle(_)
            | InputEvent::CursorMove(_)
            | InputEvent::CaseToggle => Intent::None,
        }
    }

    fn on_menu_enter(
        &mut self,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) -> Intent {
        if self.screen == Screen::Throttle {
            return self.on_menu_key(domain);
        }
        if matches!(
            self.screen,
            Screen::SsidScan | Screen::SsidList | Screen::SsidScanning
        ) {
            self.list.page = 0;
            self.list.cursor = 0;
            self.screen = Screen::SsidScanning;
            return Intent::WifiScan;
        }
        if self.is_text_entry_screen()
            || self.is_list_screen()
            || matches!(
                self.screen,
                Screen::IpConfig | Screen::IpEdit | Screen::ServerEntry | Screen::FirmwareUpdate
            )
        {
            return self.on_ok(domain, scanned, servers);
        }
        self.on_menu_key(domain)
    }

    fn on_list_step(
        &mut self,
        dir: NavDir,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) -> Intent {
        if self.is_list_screen() {
            return self.list_nav(dir, domain, scanned, servers);
        }
        if self.screen == Screen::Throttle && !domain.current_slot_has_loco() {
            let delta = if dir == NavDir::Up { -1i8 } else { 1 };
            let _ = self.addr_kbd.char_cycle(delta);
            return Intent::None;
        }
        Intent::None
    }

    fn on_char_cycle(&mut self, delta: i8, domain: &DomainState) -> Intent {
        match self.screen {
            Screen::Password | Screen::DeviceNameEdit => {
                let _ = self.text_kbd.char_cycle(delta);
            }
            Screen::ServerEntry => {
                let _ = self.ip_kbd.char_cycle(delta);
            }
            Screen::IpEdit => {
                let _ = self.net_kbd.char_cycle(delta);
            }
            Screen::DeviceIdEdit => {
                let _ = self.id_kbd.char_cycle(delta);
            }
            Screen::Throttle if !domain.current_slot_has_loco() => {
                let _ = self.addr_kbd.char_cycle(delta);
            }
            _ => {}
        }
        Intent::None
    }

    fn on_cursor_move(&mut self, delta: i8, domain: &DomainState) -> Intent {
        let left = delta < 0;
        match self.screen {
            Screen::Password | Screen::DeviceNameEdit => {
                if left {
                    let _ = self.text_kbd.nav_left();
                } else {
                    let _ = self.text_kbd.nav_right();
                }
            }
            Screen::ServerEntry => {
                if left {
                    let _ = self.ip_kbd.nav_left();
                } else {
                    let _ = self.ip_kbd.nav_right();
                }
            }
            Screen::IpEdit => {
                if left {
                    let _ = self.net_kbd.nav_left();
                } else {
                    let _ = self.net_kbd.nav_right();
                }
            }
            Screen::DeviceIdEdit => {
                if left {
                    let _ = self.id_kbd.nav_left();
                } else {
                    let _ = self.id_kbd.nav_right();
                }
            }
            Screen::Throttle if !domain.current_slot_has_loco() => {
                if left {
                    let _ = self.addr_kbd.nav_left();
                } else {
                    let _ = self.addr_kbd.nav_right();
                }
            }
            _ => {}
        }
        Intent::None
    }

    fn on_case_toggle(&mut self) -> Intent {
        if matches!(self.screen, Screen::Password | Screen::DeviceNameEdit) {
            let _ = self.text_kbd.case_toggle();
        }
        Intent::None
    }

    fn on_digit(
        &mut self,
        c: char,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) -> Intent {
        match self.screen {
            Screen::Throttle if domain.current_slot_has_loco() && c.is_ascii_digit() => {
                Intent::Function(c as u8 - b'0')
            }
            Screen::Throttle if !domain.current_slot_has_loco() && c.is_ascii_digit() => {
                if self.addr_kbd.buffer.len() < 5 {
                    let _ = self.addr_kbd.buffer.push(c);
                }
                Intent::None
            }
            Screen::Password | Screen::DeviceNameEdit if c.is_ascii_digit() => {
                let _ = self.text_kbd.key_press(c as u8 - b'0');
                Intent::None
            }
            Screen::ServerEntry if c.is_ascii_digit() => {
                let _ = self.ip_kbd.key_press(c as u8 - b'0');
                Intent::None
            }
            Screen::IpEdit if c.is_ascii_digit() => {
                let _ = self.net_kbd.key_press(c as u8 - b'0');
                Intent::None
            }
            Screen::DeviceIdEdit if c.is_ascii_digit() => {
                let _ = self.id_kbd.key_press(c as u8 - b'0');
                Intent::None
            }
            _ if c.is_ascii_digit() => {
                if self.should_accumulate_list_num(domain, scanned, servers) {
                    if self.list_num.len() < 3 {
                        let _ = self.list_num.push(c);
                    }
                    return Intent::None;
                }
                self.select_numbered_item(c as u8 - b'0', domain, scanned, servers)
            }
            _ => Intent::None,
        }
    }

    /// Keypad digit / Fn key: pick the numbered row on a choice list (language, scan, …).
    fn select_numbered_item(
        &mut self,
        n: u8,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) -> Intent {
        if !self.is_list_screen() {
            return Intent::None;
        }
        if self.is_choice_list() {
            if self.choice_select_digit(n, scanned, servers).is_some() {
                return self.on_ok(domain, scanned, servers);
            }
            return Intent::None;
        }
        let count = self.list_count(domain, scanned, servers);
        if matches!(self.screen, Screen::RosterList | Screen::FunctionList) {
            let want = digit_display_num(n);
            let start = match self.screen {
                Screen::FunctionList => page_start(&function_names(domain), self.fn_page, true),
                _ => page_start(&roster_names(domain), self.list.page, true),
            };
            for local in 0..count {
                if item_display_num(start + local) == want {
                    self.list.cursor = local;
                    return self.on_ok(domain, scanned, servers);
                }
            }
            return Intent::None;
        }
        let idx = n as usize;
        if idx < count {
            self.list.cursor = idx;
            self.on_ok(domain, scanned, servers)
        } else {
            Intent::None
        }
    }

    fn handle_global(&mut self, ev: InputEvent, domain: &DomainState) -> Option<Intent> {
        match ev {
            InputEvent::EStop => Some(Intent::Action(Action::EStop)),
            // Physical Stop: EStop on throttle, otherwise fall through to Back.
            InputEvent::Stop if self.screen == Screen::Throttle => {
                Some(Intent::Action(Action::EStop))
            }
            InputEvent::DirectionSet(dir)
                if self.screen == Screen::Throttle && domain.current_slot_has_loco() =>
            {
                Some(if dir == longfred_proto::model::Direction::Forward {
                    Intent::Action(Action::DirectionForward)
                } else {
                    Intent::Action(Action::DirectionReverse)
                })
            }
            InputEvent::DirectionToggle
                if self.screen == Screen::Throttle && domain.current_slot_has_loco() =>
            {
                Some(Intent::Action(Action::DirectionToggle))
            }
            InputEvent::Menu if self.screen == Screen::Throttle => {
                self.list_num.clear();
                self.screen = Screen::Menu;
                self.list.page = 0;
                self.list.cursor = 0;
                Some(Intent::None)
            }
            InputEvent::FnPress(k)
                if self.screen == Screen::Throttle
                    && domain.current_slot_has_loco()
                    && !self.is_text_entry_screen() =>
            {
                Some(Intent::Function(buttons::FN_TO_DCC[k.min(10) as usize]))
            }
            InputEvent::EnterProgrammingMode => Some(Intent::EnterProgrammingMode),
            _ => None,
        }
    }

    fn is_text_entry_screen(&self) -> bool {
        matches!(
            self.screen,
            Screen::Password
                | Screen::ServerEntry
                | Screen::IpEdit
                | Screen::DeviceNameEdit
                | Screen::DeviceIdEdit
        )
    }

    fn on_menu_key(&mut self, _domain: &DomainState) -> Intent {
        let leave_fw = self.screen == Screen::FirmwareUpdate;
        self.list_num.clear();
        self.screen = Screen::Menu;
        self.list.reset();
        if leave_fw {
            Intent::SetHttpOta(false)
        } else {
            Intent::None
        }
    }

    fn on_nav(
        &mut self,
        dir: NavDir,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) -> Intent {
        match self.screen {
            Screen::Password | Screen::DeviceNameEdit => {
                match dir {
                    NavDir::Up => {
                        let _ = self.text_kbd.nav_up();
                    }
                    NavDir::Down => {
                        let _ = self.text_kbd.nav_down();
                    }
                    NavDir::Right => {
                        let _ = self.text_kbd.nav_right();
                    }
                    NavDir::Left => {
                        let _ = self.text_kbd.nav_left();
                    }
                }
                Intent::None
            }
            Screen::ServerEntry => self.nav_ip_kbd(dir),
            Screen::IpEdit => self.nav_net_kbd(dir),
            Screen::DeviceIdEdit => self.nav_kbd_id(dir),
            Screen::ServerList if dir == NavDir::Left => {
                self.begin_manual_server_from_list();
                Intent::None
            }
            Screen::Throttle if !domain.current_slot_has_loco() => match dir {
                NavDir::Up | NavDir::Down => {
                    let _ = if dir == NavDir::Up {
                        self.addr_kbd.nav_up()
                    } else {
                        self.addr_kbd.nav_down()
                    };
                    Intent::None
                }
                _ => Intent::None,
            },
            _ if self.is_list_screen() => self.list_nav(dir, domain, scanned, servers),
            _ => Intent::None,
        }
    }

    fn nav_kbd_id(&mut self, dir: NavDir) -> Intent {
        match dir {
            NavDir::Up => {
                let _ = self.id_kbd.nav_up();
            }
            NavDir::Down => {
                let _ = self.id_kbd.nav_down();
            }
            NavDir::Right => {
                let _ = self.id_kbd.nav_right();
            }
            NavDir::Left => {
                let _ = self.id_kbd.nav_left();
            }
        }
        Intent::None
    }

    fn nav_ip_kbd(&mut self, dir: NavDir) -> Intent {
        match dir {
            NavDir::Up => {
                let _ = self.ip_kbd.nav_up();
            }
            NavDir::Down => {
                let _ = self.ip_kbd.nav_down();
            }
            NavDir::Right => {
                let _ = self.ip_kbd.nav_right();
            }
            NavDir::Left => {
                let _ = self.ip_kbd.nav_left();
            }
        }
        Intent::None
    }

    fn nav_net_kbd(&mut self, dir: NavDir) -> Intent {
        match dir {
            NavDir::Up => {
                let _ = self.net_kbd.nav_up();
            }
            NavDir::Down => {
                let _ = self.net_kbd.nav_down();
            }
            NavDir::Right => {
                let _ = self.net_kbd.nav_right();
            }
            NavDir::Left => {
                let _ = self.net_kbd.nav_left();
            }
        }
        Intent::None
    }

    fn on_ok(
        &mut self,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) -> Intent {
        if !self.list_num.is_empty() && !self.apply_list_num(domain, scanned, servers) {
            return Intent::None;
        }
        match self.screen {
            Screen::Throttle => self.ok_throttle(domain),
            Screen::Menu => self.ok_menu(),
            Screen::Extras => self.ok_extras(),
            Screen::SsidList => self.ok_ssid_list(domain),
            Screen::SsidScan => self.ok_ssid_scan(domain, scanned, servers),
            Screen::Password => self.ok_password(),
            Screen::ServerList => self.ok_server_list(servers),
            Screen::ServerProto => self.ok_server_proto(),
            Screen::ServerEntry => self.ok_server_entry(),
            Screen::RosterList => self.ok_roster(domain),
            Screen::FunctionList => self.ok_fn_list(domain),
            Screen::IpConfig => {
                self.begin_ip_edit(domain);
                Intent::None
            }
            Screen::IpEdit => self.ok_ip_edit(),
            Screen::Device => self.ok_device(domain),
            Screen::DeviceNameEdit => self.ok_device_name(domain),
            Screen::DeviceIdEdit => self.ok_device_id(domain),
            Screen::Language => self.ok_language(),
            Screen::FirmwareUpdate => Intent::SetHttpOta(!crate::net::http_ota_enabled()),
            Screen::DirectCommands => {
                let idx = page_start(&direct_labels(), self.list.page, false) + self.list.cursor;
                self.ok_direct(idx)
            }
            Screen::Diagnostics => Intent::None,
            _ => Intent::None,
        }
    }

    fn on_back(&mut self, _domain: &DomainState) -> Intent {
        if !self.list_num.is_empty() {
            self.list_num.clear();
            return Intent::None;
        }
        match self.screen {
            Screen::Throttle => Intent::None,
            Screen::Menu => {
                self.screen = Screen::Throttle;
                Intent::None
            }
            Screen::Extras => {
                self.screen = Screen::Menu;
                self.list.reset();
                Intent::None
            }
            Screen::Password => {
                self.screen = if self.selected_from_scan {
                    Screen::SsidScan
                } else {
                    Screen::SsidList
                };
                Intent::None
            }
            Screen::SsidScanning => {
                self.screen = Screen::SsidScan;
                Intent::None
            }
            Screen::SsidScan => {
                if !compiled_ssids().is_empty() {
                    self.screen = Screen::SsidList;
                }
                Intent::None
            }
            Screen::SsidList => {
                self.screen = Screen::Throttle;
                Intent::None
            }
            Screen::WifiFailed => {
                self.screen = Screen::SsidScan;
                Intent::None
            }
            Screen::ServerList => {
                self.screen = if compiled_ssids().is_empty() {
                    Screen::SsidScan
                } else {
                    Screen::SsidList
                };
                Intent::None
            }
            Screen::ServerProto => {
                self.screen = Screen::ServerList;
                Intent::None
            }
            Screen::ServerEntry if self.server_entry_from_list => {
                self.server_entry_from_list = false;
                self.screen = Screen::ServerList;
                Intent::None
            }
            Screen::ServerEntry => {
                self.screen = Screen::ServerProto;
                Intent::None
            }
            Screen::IpConfig | Screen::IpEdit => {
                self.screen = Screen::Extras;
                Intent::None
            }
            Screen::Device | Screen::DeviceNameEdit | Screen::DeviceIdEdit => {
                self.screen = Screen::Extras;
                Intent::None
            }
            Screen::Language if self.boot_language => Intent::None,
            Screen::Language => {
                self.screen = Screen::Extras;
                Intent::None
            }
            Screen::Diagnostics => {
                self.screen = Screen::Extras;
                Intent::None
            }
            Screen::FirmwareUpdate => {
                self.screen = Screen::Extras;
                Intent::SetHttpOta(false)
            }
            Screen::RosterList | Screen::FunctionList | Screen::DirectCommands => {
                self.screen = Screen::Throttle;
                Intent::None
            }
            _ => {
                self.screen = Screen::Throttle;
                Intent::None
            }
        }
    }

    fn on_fn_press(
        &mut self,
        k: u8,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) -> Intent {
        match self.screen {
            Screen::Throttle if !domain.current_slot_has_loco() => {
                let _ = self.addr_kbd.fn_press(k);
                Intent::None
            }
            Screen::Password | Screen::DeviceNameEdit => {
                let _ = self.text_kbd.fn_press(k);
                Intent::None
            }
            Screen::ServerEntry => {
                let _ = self.ip_kbd.fn_press(k);
                Intent::None
            }
            Screen::IpEdit => {
                let _ = self.net_kbd.fn_press(k);
                Intent::None
            }
            Screen::DeviceIdEdit => {
                let _ = self.id_kbd.fn_press(k);
                Intent::None
            }
            _ => {
                if k <= 9 && self.should_accumulate_list_num(domain, scanned, servers) {
                    if self.list_num.len() < 3 {
                        let _ = self.list_num.push((b'0' + k) as char);
                    }
                    return Intent::None;
                }
                self.select_numbered_item(k, domain, scanned, servers)
            }
        }
    }

    fn is_choice_list(&self) -> bool {
        matches!(
            self.screen,
            Screen::Menu
                | Screen::SsidList
                | Screen::SsidScan
                | Screen::ServerList
                | Screen::Language
        )
    }

    fn choice_numbered(&self) -> bool {
        !matches!(self.screen, Screen::Language)
    }

    fn choice_select_digit(
        &mut self,
        n: u8,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) -> Option<usize> {
        let numbered = self.choice_numbered();
        match self.screen {
            Screen::Menu => self.list.select_digit(n, &menu_labels(), numbered),
            Screen::SsidList => self.list.select_digit(n, &compiled_ssids(), numbered),
            Screen::SsidScan => self
                .list
                .select_digit(n, &scan_ssid_names(scanned), numbered),
            Screen::ServerList => {
                let bufs = server_label_bufs(servers);
                self.list
                    .select_digit(n, &server_label_refs(&bufs), numbered)
            }
            Screen::Language => self.list.select_digit(n, &language_labels(), numbered),
            _ => None,
        }
    }

    fn choice_list_ud(
        &mut self,
        dir: NavDir,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) {
        let numbered = self.choice_numbered();
        match self.screen {
            Screen::Menu => self.choice_ud(dir, &menu_labels(), numbered),
            Screen::SsidList => self.choice_ud(dir, &compiled_ssids(), numbered),
            Screen::SsidScan => self.choice_ud(dir, &scan_ssid_names(scanned), numbered),
            Screen::ServerList => {
                let bufs = server_label_bufs(servers);
                self.choice_ud(dir, &server_label_refs(&bufs), numbered);
            }
            Screen::Language => self.choice_ud(dir, &language_labels(), numbered),
            _ => {}
        }
    }

    fn choice_ud(&mut self, dir: NavDir, items: &[&str], numbered: bool) {
        match dir {
            NavDir::Up => self.list.list_prev(items, numbered),
            NavDir::Down => self.list.list_next(items, numbered),
            _ => {}
        }
    }

    fn choice_page(
        &mut self,
        next: bool,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) {
        let numbered = self.choice_numbered();
        match self.screen {
            Screen::Menu => self.choice_page_dir(next, &menu_labels(), numbered),
            Screen::SsidList => self.choice_page_dir(next, &compiled_ssids(), numbered),
            Screen::SsidScan => self.choice_page_dir(next, &scan_ssid_names(scanned), numbered),
            Screen::ServerList => {
                let bufs = server_label_bufs(servers);
                self.choice_page_dir(next, &server_label_refs(&bufs), numbered);
            }
            Screen::Language => self.choice_page_dir(next, &language_labels(), numbered),
            _ => {}
        }
    }

    fn choice_page_dir(&mut self, next: bool, items: &[&str], numbered: bool) {
        if next {
            self.list.page_next(items, numbered);
        } else {
            self.list.page_prev(items);
        }
    }

    fn is_list_screen(&self) -> bool {
        matches!(
            self.screen,
            Screen::Menu
                | Screen::Extras
                | Screen::SsidList
                | Screen::SsidScan
                | Screen::ServerList
                | Screen::ServerProto
                | Screen::RosterList
                | Screen::FunctionList
                | Screen::Device
                | Screen::Language
                | Screen::DirectCommands
        )
    }

    fn list_count(
        &self,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) -> usize {
        match self.screen {
            Screen::SsidList => self.list.visible_count(&compiled_ssids(), true),
            Screen::SsidScan => self.list.visible_count(&scan_ssid_names(scanned), true),
            Screen::ServerList => {
                let bufs = server_label_bufs(servers);
                self.list.visible_count(&server_label_refs(&bufs), true)
            }
            Screen::ServerProto => 2,
            Screen::Menu => self.list.visible_count(&menu_labels(), true),
            Screen::Extras => page_item_count(&extras_labels(), self.list.page, false),
            Screen::RosterList => {
                let names = roster_names(domain);
                page_item_count(&names, self.list.page, true)
            }
            Screen::FunctionList => {
                let names = function_names(domain);
                page_item_count(&names, self.list.page, true)
            }
            Screen::Device => 3,
            Screen::Language => self.list.visible_count(&language_labels(), false),
            Screen::DirectCommands => page_item_count(&direct_labels(), self.list.page, false),
            _ => 0,
        }
    }

    fn list_nav(
        &mut self,
        dir: NavDir,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) -> Intent {
        if self.is_choice_list() && matches!(dir, NavDir::Up | NavDir::Down) {
            self.choice_list_ud(dir, scanned, servers);
            return Intent::None;
        }
        let count = self.list_count(domain, scanned, servers);
        if count == 0 {
            return Intent::None;
        }
        match dir {
            NavDir::Up => {
                if self.list.cursor == 0 {
                    if self.screen == Screen::FunctionList {
                        if self.fn_page > 0 {
                            self.fn_page -= 1;
                            self.list.page = self.fn_page;
                            self.list.cursor =
                                self.list_count(domain, scanned, servers).saturating_sub(1);
                        } else {
                            self.list.cursor = count - 1;
                        }
                    } else if self.list.page > 0 {
                        self.list.page -= 1;
                        self.list.cursor =
                            self.list_count(domain, scanned, servers).saturating_sub(1);
                    } else {
                        self.list.cursor = count - 1;
                    }
                } else {
                    self.list.cursor -= 1;
                }
            }
            NavDir::Down => {
                if self.list.cursor + 1 >= count {
                    if self.has_list_next_page(domain, scanned, servers) {
                        if self.screen == Screen::FunctionList {
                            self.fn_page += 1;
                            self.list.page = self.fn_page;
                        } else {
                            self.list.page += 1;
                        }
                        self.list.cursor = 0;
                    } else {
                        self.list.cursor = 0;
                    }
                } else {
                    self.list.cursor += 1;
                }
            }
            NavDir::Right => return self.list_page_next(domain, scanned, servers),
            NavDir::Left => return self.list_page_prev(domain, scanned, servers),
        }
        Intent::None
    }

    fn has_list_next_page(
        &self,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) -> bool {
        match self.screen {
            Screen::SsidList => has_next_page(&compiled_ssids(), self.list.page, true),
            Screen::SsidScan => has_next_page(&scan_ssid_names(scanned), self.list.page, true),
            Screen::ServerList => {
                let bufs = server_label_bufs(servers);
                has_next_page(&server_label_refs(&bufs), self.list.page, true)
            }
            Screen::Menu => has_next_page(&menu_labels(), self.list.page, true),
            Screen::Extras => has_next_page(&extras_labels(), self.list.page, false),
            Screen::RosterList => has_next_page(&roster_names(domain), self.list.page, true),
            Screen::FunctionList => has_next_page(&function_names(domain), self.fn_page, true),
            Screen::Language => has_next_page(&language_labels(), self.list.page, false),
            Screen::DirectCommands => has_next_page(&direct_labels(), self.list.page, false),
            _ => false,
        }
    }

    fn on_page_next(
        &mut self,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) -> Intent {
        match self.screen {
            Screen::Diagnostics => {
                self.list.page = (self.list.page + 1) % DIAG_PAGES;
                Intent::None
            }
            Screen::SsidList => {
                self.screen = Screen::SsidScanning;
                self.list.page = 0;
                self.list.cursor = 0;
                Intent::WifiScan
            }
            Screen::ServerList => {
                self.screen = Screen::ServerProto;
                self.list.cursor = 0;
                Intent::None
            }
            _ => self.list_page_next(domain, scanned, servers),
        }
    }

    fn on_page_prev(
        &mut self,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) -> Intent {
        match self.screen {
            Screen::Diagnostics if self.list.page == 0 => self.on_back(domain),
            Screen::Diagnostics => {
                self.list.page -= 1;
                Intent::None
            }
            Screen::ServerList => {
                self.begin_manual_server_from_list();
                Intent::None
            }
            _ => self.list_page_prev(domain, scanned, servers),
        }
    }

    fn list_page_next(
        &mut self,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) -> Intent {
        if self.is_choice_list() {
            self.choice_page(true, scanned, servers);
            return Intent::None;
        }
        if self.screen == Screen::RosterList {
            let names = roster_names(domain);
            if has_next_page(&names, self.list.page, true) {
                self.list.page += 1;
            } else if names.len() > page_item_count(&names, 0, true) {
                self.list.page = 0;
            }
            self.list.cursor = 0;
            return Intent::None;
        }
        if self.screen == Screen::FunctionList {
            if has_next_page(&function_names(domain), self.fn_page, true) {
                self.fn_page += 1;
                self.list.page = self.fn_page;
                self.list.cursor = 0;
            }
            return Intent::None;
        }
        if self.has_list_next_page(domain, scanned, servers) {
            self.list.page += 1;
            self.list.cursor = 0;
        }
        Intent::None
    }

    fn list_page_prev(
        &mut self,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) -> Intent {
        if self.is_choice_list() {
            self.choice_page(false, scanned, servers);
            return Intent::None;
        }
        if self.screen == Screen::FunctionList {
            if self.fn_page > 0 {
                self.fn_page -= 1;
                self.list.page = self.fn_page;
                self.list.cursor = 0;
            }
            return Intent::None;
        }
        let _ = domain;
        if self.list.page > 0 {
            self.list.page -= 1;
            self.list.cursor = 0;
        }
        Intent::None
    }

    fn ok_throttle(&mut self, domain: &DomainState) -> Intent {
        if domain.current_slot_has_loco() {
            if self.hash_functions {
                self.screen = Screen::FunctionList;
                self.fn_page = 0;
                self.list.cursor = 0;
            } else {
                self.screen = Screen::DirectCommands;
                self.list.cursor = 0;
            }
            return Intent::None;
        }
        self.addr.clear();
        let _ = self.addr.push_str(self.addr_kbd.buffer.as_str());
        self.addr_kbd.clear();
        if self.addr.is_empty() {
            Intent::None
        } else {
            Intent::AcquireAddr
        }
    }

    fn ok_menu(&mut self) -> Intent {
        let labels = menu_labels();
        let idx = self.list.global_index(&labels, true);
        self.list_num.clear();
        match idx {
            0 => {
                self.screen = Screen::FunctionList;
                self.fn_page = 0;
                self.list.reset();
                Intent::None
            }
            1 => {
                self.screen = Screen::RosterList;
                self.list.reset();
                Intent::None
            }
            2 => {
                self.screen = Screen::Throttle;
                Intent::Action(Action::SpeedMultiplier)
            }
            3 => {
                self.screen = Screen::Throttle;
                Intent::Action(Action::PowerToggle)
            }
            4 => {
                self.screen = Screen::Extras;
                self.list.reset();
                Intent::None
            }
            _ => Intent::None,
        }
    }

    fn ok_extras(&mut self) -> Intent {
        let idx = page_start(&extras_labels(), self.list.page, false) + self.list.cursor;
        match idx {
            0 => {
                self.screen = Screen::IpConfig;
                Intent::None
            }
            1 => {
                self.screen = Screen::Device;
                self.list.cursor = 0;
                Intent::None
            }
            2 => {
                self.screen = Screen::Throttle;
                Intent::HashFunctionsToggle
            }
            3 => {
                self.screen = Screen::Throttle;
                Intent::HeartbeatToggle
            }
            4 => {
                self.screen = Screen::Throttle;
                Intent::Action(Action::MaxThrottleIncrease)
            }
            5 => {
                self.screen = Screen::Throttle;
                Intent::Action(Action::MaxThrottleDecrease)
            }
            6 => {
                self.screen = Screen::Throttle;
                Intent::Sleep
            }
            7 => {
                self.screen = Screen::Throttle;
                Intent::DropBeforeAcquireToggle
            }
            8 => {
                self.screen = Screen::Language;
                self.list.cursor = 0;
                Intent::None
            }
            9 => {
                self.screen = Screen::FirmwareUpdate;
                Intent::None
            }
            10 => {
                self.screen = Screen::Diagnostics;
                self.list.page = 0;
                Intent::None
            }
            _ => Intent::None,
        }
    }

    fn ok_ssid_list(&mut self, domain: &DomainState) -> Intent {
        let names = compiled_ssids();
        let idx = self.list.global_index(&names, true);
        if let Some(n) = config::network::NETWORKS.get(idx) {
            self.selected_ssid_idx = idx;
            self.selected_from_scan = false;
            self.begin_password_edit(domain, n.ssid);
            Intent::None
        } else {
            Intent::None
        }
    }

    fn ok_ssid_scan(
        &mut self,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) -> Intent {
        let _ = servers;
        let names = scan_ssid_names(scanned);
        self.selected_ssid_idx = self.list.global_index(&names, true);
        self.selected_from_scan = true;
        if let Some(s) = scanned.get(self.selected_ssid_idx) {
            self.begin_password_edit(domain, s.ssid.as_str());
            return Intent::None;
        }
        Intent::None
    }

    fn begin_password_edit(&mut self, domain: &DomainState, ssid: &str) {
        let keep_draft = self.selected_ssid.as_str() == ssid
            && (!self.pw.is_empty() || !self.text_kbd.buffer.is_empty());
        self.selected_ssid.clear();
        let _ = self.selected_ssid.push_str(ssid);
        self.screen = Screen::Password;
        self.text_kbd.mode = KeyboardMode::Text;
        self.text_kbd.set_max_len(64);
        if keep_draft {
            return;
        }
        let stored = domain
            .persist
            .find_password(ssid)
            .or_else(|| {
                config::network::NETWORKS
                    .iter()
                    .find(|n| n.ssid == ssid)
                    .map(|n| n.password)
            })
            .unwrap_or("");
        self.text_kbd.load(stored);
        self.pw.clear();
        let _ = self.pw.push_str(stored);
    }

    fn ok_password(&mut self) -> Intent {
        let _ = self.text_kbd.ok();
        self.pw.clear();
        let _ = self.pw.push_str(self.text_kbd.buffer.as_str());
        if self.selected_from_scan && !self.pw.is_empty() {
            self.pending_password_save = true;
        }
        self.screen = Screen::Connecting;
        Intent::WifiConnect
    }

    fn ok_server_list(
        &mut self,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) -> Intent {
        let bufs = server_label_bufs(servers);
        let names = server_label_refs(&bufs);
        let idx = self.list.global_index(&names, true);
        Intent::ServerSelect(idx)
    }

    fn ok_server_proto(&mut self) -> Intent {
        match self.list.cursor {
            0 => {
                self.begin_server_entry(Protocol::WiThrottle);
                Intent::None
            }
            1 => {
                self.begin_server_entry(Protocol::Z21);
                Intent::None
            }
            _ => Intent::None,
        }
    }

    fn ok_server_entry(&mut self) -> Intent {
        let _ = self.ip_kbd.ok();
        self.ip_digits.clear();
        let _ = self.ip_digits.push_str(self.ip_kbd.buffer.as_str());
        if self.ip_digits.len() == 17 {
            Intent::ServerManual
        } else {
            Intent::None
        }
    }

    fn ok_roster(&mut self, domain: &DomainState) -> Intent {
        let names = roster_names(domain);
        let idx = page_start(&names, self.list.page, true) + self.list.cursor;
        self.screen = Screen::Throttle;
        if !domain.roster.is_empty() {
            Intent::AcquireRoster(idx.min(domain.roster.len().saturating_sub(1)))
        } else if let Some(e) = domain.persist.static_roster.get(idx) {
            self.addr.clear();
            let _ = self.addr.push_str(e.addr.as_str());
            Intent::AcquireAddr
        } else {
            Intent::None
        }
    }

    fn ok_fn_list(&mut self, domain: &DomainState) -> Intent {
        let names = function_names(domain);
        let idx = page_start(&names, self.fn_page, true) + self.list.cursor;
        Intent::Function(idx.min(sizes::MAX_FUNCTIONS - 1) as u8)
    }

    fn ok_device(&mut self, domain: &DomainState) -> Intent {
        match self.list.cursor {
            0 => {
                self.begin_device_name_edit(domain);
                Intent::None
            }
            1 => {
                self.begin_device_id_edit(domain);
                Intent::None
            }
            2 => Intent::RegenerateDeviceId,
            _ => Intent::None,
        }
    }

    fn ok_device_name(&mut self, domain: &DomainState) -> Intent {
        let _ = self.text_kbd.ok();
        self.device_name_edit.clear();
        let _ = self
            .device_name_edit
            .push_str(self.text_kbd.buffer.as_str());
        let mut device = domain.persist.device.clone();
        device.name.clear();
        let _ = device.name.push_str(self.device_name_edit.as_str());
        self.screen = Screen::Device;
        Intent::SaveDevice(device)
    }

    fn ok_device_id(&mut self, domain: &DomainState) -> Intent {
        let _ = self.id_kbd.ok();
        if self.id_kbd.buffer.len() == 4 {
            let mut id = 0u16;
            for b in self.id_kbd.buffer.as_bytes() {
                id = id.saturating_mul(10).saturating_add((b - b'0') as u16);
            }
            if id >= DEVICE_ID_MIN && id <= DEVICE_ID_MAX {
                let mut device = domain.persist.device.clone();
                device.id = id;
                self.screen = Screen::Device;
                return Intent::SaveDevice(device);
            }
        }
        Intent::None
    }

    fn ok_language(&mut self) -> Intent {
        let lang = match self.list.global_index(&language_labels(), false) {
            0 => Language::En,
            1 => Language::Pl,
            2 => Language::De,
            _ => return Intent::None,
        };
        if self.boot_language {
            self.boot_language = false;
            return Intent::SetLanguage(lang);
        }
        self.screen = Screen::Extras;
        Intent::SetLanguage(lang)
    }

    fn ok_ip_edit(&mut self) -> Intent {
        self.sync_digits_from_net_kbd();
        self.ip_edit_advance()
    }

    fn ok_direct(&mut self, idx: usize) -> Intent {
        let actions = [
            Action::Function(0),
            Action::NextThrottle,
            Action::SpeedMultiplier,
            Action::DirectionReverse,
            Action::EStop,
            Action::None,
        ];
        if idx == 5 {
            self.screen = Screen::Throttle;
            Intent::None
        } else if let Some(a) = actions.get(idx) {
            if *a == Action::None {
                Intent::None
            } else {
                Intent::Action(*a)
            }
        } else {
            Intent::None
        }
    }

    fn should_accumulate_list_num(
        &self,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) -> bool {
        self.numbered_list_len(domain, scanned, servers)
            .is_some_and(|n| n > 9)
    }

    fn numbered_list_len(
        &self,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) -> Option<usize> {
        match self.screen {
            Screen::FunctionList => Some(function_names(domain).len()),
            Screen::RosterList => Some(roster_names(domain).len()),
            Screen::SsidList => Some(compiled_ssids().len()),
            Screen::SsidScan => Some(scan_ssid_names(scanned).len()),
            Screen::ServerList => Some(servers.len()),
            _ => None,
        }
    }

    /// Parse `list_num` as a 1-based index and jump the cursor there.
    fn apply_list_num(
        &mut self,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) -> bool {
        let parsed = self.list_num.parse::<usize>().ok();
        self.list_num.clear();
        let Some(n) = parsed.filter(|n| *n > 0) else {
            return false;
        };
        let global = n - 1;
        match self.screen {
            Screen::FunctionList => {
                let names = function_names(domain);
                let ok = self.list.jump_to_global(&names, true, global);
                if ok {
                    self.fn_page = self.list.page;
                }
                ok
            }
            Screen::RosterList => {
                let names = roster_names(domain);
                self.list.jump_to_global(&names, true, global)
            }
            Screen::SsidList => {
                let names = compiled_ssids();
                self.list.jump_to_global(&names, true, global)
            }
            Screen::SsidScan => {
                let names = scan_ssid_names(scanned);
                self.list.jump_to_global(&names, true, global)
            }
            Screen::ServerList => {
                let bufs = server_label_bufs(servers);
                let names = server_label_refs(&bufs);
                self.list.jump_to_global(&names, true, global)
            }
            _ => false,
        }
    }
}

pub(crate) fn menu_labels() -> [&'static str; 5] {
    let t = crate::ui::i18n::tr();
    [
        t.menu_fn,
        t.menu_locos,
        t.menu_speed_mult,
        t.menu_power,
        t.menu_extras,
    ]
}

pub(crate) fn extras_labels() -> [&'static str; 11] {
    let t = crate::ui::i18n::tr();
    [
        t.extras_net_config,
        t.extras_device,
        t.extras_fnc_key_tgl,
        t.extras_heartbt_tgl,
        t.extras_throttles_plus,
        t.extras_throttles_minus,
        t.extras_off_sleep,
        t.extras_one_loco_tgl,
        t.extras_language,
        t.extras_firmware,
        t.extras_diag,
    ]
}

pub(crate) fn language_labels() -> [&'static str; 3] {
    let t = crate::ui::i18n::tr();
    [t.lang_en, t.lang_pl, t.lang_de]
}

pub(crate) fn direct_labels() -> [&'static str; 6] {
    let t = crate::ui::i18n::tr();
    [
        t.direct_fn,
        t.direct_next_thr,
        t.direct_spd_mult,
        t.direct_rev,
        t.direct_estop,
        t.direct_back,
    ]
}

pub(crate) fn server_label_bufs(
    servers: &heapless::Vec<WitServer, { sizes::MAX_FOUND_SERVERS }>,
) -> heapless::Vec<Line, { sizes::MAX_FOUND_SERVERS }> {
    let mut v = heapless::Vec::new();
    for s in servers {
        let mut line = Line::new();
        let _ = line.push_str(s.label.as_str());
        let _ = line.push(' ');
        let tag = match s.protocol {
            Protocol::WiThrottle => 'W',
            Protocol::Z21 => 'Z',
        };
        let _ = line.push(tag);
        if v.push(line).is_err() {
            break;
        }
    }
    v
}

fn server_label_refs<'a>(
    bufs: &'a heapless::Vec<Line, { sizes::MAX_FOUND_SERVERS }>,
) -> heapless::Vec<&'a str, { sizes::MAX_FOUND_SERVERS }> {
    let mut v = heapless::Vec::new();
    for b in bufs {
        if v.push(b.as_str()).is_err() {
            break;
        }
    }
    v
}

pub(crate) fn compiled_ssids() -> heapless::Vec<&'static str, 16> {
    let mut v = heapless::Vec::new();
    for n in config::network::NETWORKS.iter() {
        if v.push(n.ssid).is_err() {
            break;
        }
    }
    v
}

fn scan_ssid_names(
    scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
) -> heapless::Vec<&str, { sizes::MAX_FOUND_SSIDS }> {
    let mut v = heapless::Vec::new();
    for s in scanned {
        if v.push(s.ssid.as_str()).is_err() {
            break;
        }
    }
    v
}

pub(crate) fn roster_names(domain: &DomainState) -> heapless::Vec<&str, { sizes::MAX_ROSTER }> {
    let mut v = heapless::Vec::new();
    if !domain.roster.is_empty() {
        for e in &domain.roster {
            if v.push(e.name.as_str()).is_err() {
                break;
            }
        }
    } else {
        for e in &domain.persist.static_roster {
            let s = if e.name.is_empty() {
                e.addr.as_str()
            } else {
                e.name.as_str()
            };
            if v.push(s).is_err() {
                break;
            }
        }
    }
    v
}

fn function_names(domain: &DomainState) -> heapless::Vec<&str, { sizes::MAX_FUNCTIONS }> {
    let mut v = heapless::Vec::new();
    let slot = domain.current_slot();
    for i in 0..sizes::MAX_FUNCTIONS {
        if v.push(slot.labels[i].as_str()).is_err() {
            break;
        }
    }
    v
}
