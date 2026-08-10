//! Joystick / tact-switch navigation handlers for MenuFsm.

use longfred_proto::command::Protocol;
use longfred_proto::model::TurnoutAction;
use longfred_proto::persist::{DEVICE_ID_MAX, DEVICE_ID_MIN, Language};

use crate::config::{self, buttons, sizes};
use crate::domain::actions::Action;
use crate::domain::state::DomainState;
use crate::input::{InputEvent, NavDir};
use crate::net::SsidInfo;
use crate::ui::keyboard::KeyboardMode;
use crate::ui::menu::{Intent, ListRef, MENU_KEYS, MENU_TYPES, MenuFsm, MenuItemType, Screen};
use crate::ui::nav_profile::{self, NavAction, NavProfile};

impl MenuFsm {
    pub fn handle_input(
        &mut self,
        ev: InputEvent,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
    ) -> Intent {
        let text_entry = self.is_text_entry_screen();
        let on_throttle = self.screen == Screen::Throttle;
        let profile = nav_profile::active();
        let Some(action) = profile.map(ev, text_entry, on_throttle) else {
            return Intent::None;
        };
        match action {
            NavAction::ListPrev => self.on_list_step(NavDir::Up, domain, scanned),
            NavAction::ListNext => self.on_list_step(NavDir::Down, domain, scanned),
            NavAction::Select => self.on_ok(domain, scanned),
            NavAction::Cancel => self.on_back(domain),
            NavAction::MenuEnter => self.on_menu_enter(domain, scanned),
            NavAction::CharCycle(d) => self.on_char_cycle(d, domain),
            NavAction::CursorMove(d) => self.on_cursor_move(d, domain),
            NavAction::CaseToggle => self.on_case_toggle(),
            NavAction::Digit(c) => self.on_digit(c, domain),
            NavAction::PassThrough(ev) => self.handle_passthrough(ev, domain, scanned),
        }
    }

    fn handle_passthrough(
        &mut self,
        ev: InputEvent,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
    ) -> Intent {
        if let Some(intent) = self.handle_global(ev, domain) {
            return intent;
        }
        match ev {
            InputEvent::Nav(dir) => self.on_nav(dir, domain, scanned),
            InputEvent::Ok => self.on_ok(domain, scanned),
            InputEvent::Back => self.on_back(domain),
            InputEvent::Menu => self.on_menu_key(domain),
            InputEvent::FnPress(k) => self.on_fn_press(k, domain, scanned),
            InputEvent::FnRelease(k) => self.on_fn_release(k, domain),
            InputEvent::EncoderClockwise => self.encoder(true, domain),
            InputEvent::EncoderCounterClockwise => self.encoder(false, domain),
            InputEvent::EncoderButton => self.encoder_button(domain),
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
    ) -> Intent {
        if self.screen == Screen::Throttle {
            return self.on_menu_key(domain);
        }
        if self.is_text_entry_screen()
            || self.is_list_screen()
            || matches!(
                self.screen,
                Screen::IpConfig | Screen::IpEdit | Screen::ServerEntry
            )
        {
            return self.on_ok(domain, scanned);
        }
        self.on_menu_key(domain)
    }

    fn on_list_step(
        &mut self,
        dir: NavDir,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
    ) -> Intent {
        if self.is_list_screen() {
            return self.list_nav(dir, domain, scanned);
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
        // Buffer cursor lives in MenuFsm later; for now Left=backspace, Right=commit.
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

    fn on_digit(&mut self, c: char, domain: &DomainState) -> Intent {
        if self.screen == Screen::Menu {
            if c.is_ascii_digit() {
                let _ = self.menu_cmd.push(c);
            }
            return Intent::None;
        }
        match self.screen {
            Screen::Throttle if !domain.current_slot_has_loco() && c.is_ascii_digit() => {
                if self.addr_kbd.buffer.len() < 5 {
                    let _ = self.addr_kbd.buffer.push(c);
                }
            }
            Screen::Password | Screen::DeviceNameEdit => {
                if self.text_kbd.buffer.len() < 64 {
                    let _ = self.text_kbd.buffer.push(c);
                }
            }
            Screen::ServerEntry if c.is_ascii_digit() => {
                if self.ip_kbd.buffer.len() < 17 {
                    let _ = self.ip_kbd.buffer.push(c);
                }
            }
            Screen::IpEdit if c.is_ascii_digit() => {
                if self.net_kbd.buffer.len() < 12 {
                    let _ = self.net_kbd.buffer.push(c);
                }
            }
            Screen::DeviceIdEdit if c.is_ascii_digit() => {
                if self.id_kbd.buffer.len() < 4 {
                    let _ = self.id_kbd.buffer.push(c);
                }
            }
            _ => {}
        }
        Intent::None
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
                self.menu_cmd.clear();
                self.screen = Screen::Menu;
                self.cursor = 0;
                Some(Intent::None)
            }
            InputEvent::FnPress(k)
                if self.screen == Screen::Throttle
                    && domain.current_slot_has_loco()
                    && !self.is_text_entry_screen() =>
            {
                Some(Intent::Function(
                    buttons::FN_TO_DCC[k.min(10) as usize],
                    true,
                ))
            }
            InputEvent::FnRelease(k)
                if self.screen == Screen::Throttle && domain.current_slot_has_loco() =>
            {
                Some(Intent::Function(
                    buttons::FN_TO_DCC[k.min(10) as usize],
                    false,
                ))
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
        ) || (self.screen == Screen::Throttle && !self.addr_kbd.buffer.is_empty())
    }

    fn on_menu_key(&mut self, domain: &DomainState) -> Intent {
        if self.screen == Screen::Throttle && !domain.current_slot_has_loco() {
            return Intent::None;
        }
        self.menu_cmd.clear();
        self.screen = Screen::Menu;
        self.cursor = 0;
        Intent::None
    }

    fn on_nav(
        &mut self,
        dir: NavDir,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
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
            _ if self.is_list_screen() => self.list_nav(dir, domain, scanned),
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
    ) -> Intent {
        match self.screen {
            Screen::Throttle => self.ok_throttle(domain),
            Screen::Menu => self.ok_menu(),
            Screen::Extras => self.ok_extras(),
            Screen::SsidList => self.ok_ssid_list(),
            Screen::SsidScan => self.ok_ssid_scan(domain, scanned),
            Screen::Password => self.ok_password(),
            Screen::ServerList => self.ok_server_list(),
            Screen::ServerProto => self.ok_server_proto(),
            Screen::ServerEntry => self.ok_server_entry(),
            Screen::RosterList => self.ok_roster(domain),
            Screen::FunctionList => self.ok_fn_list(),
            Screen::TurnoutList => self.ok_turnout(),
            Screen::RouteList => self.ok_route(),
            Screen::IpConfig => {
                self.begin_ip_edit(domain);
                Intent::None
            }
            Screen::IpEdit => self.ok_ip_edit(),
            Screen::Device => self.ok_device(domain),
            Screen::DeviceNameEdit => self.ok_device_name(domain),
            Screen::DeviceIdEdit => self.ok_device_id(domain),
            Screen::Language => self.ok_language(),
            Screen::DirectCommands => self.ok_direct(self.cursor),
            _ => Intent::None,
        }
    }

    fn on_back(&mut self, _domain: &DomainState) -> Intent {
        match self.screen {
            Screen::Throttle => Intent::None,
            Screen::Menu => {
                self.screen = Screen::Throttle;
                self.menu_cmd.clear();
                Intent::None
            }
            Screen::Extras => {
                self.screen = Screen::Menu;
                Intent::None
            }
            Screen::Password => {
                self.screen = Screen::SsidScan;
                Intent::None
            }
            Screen::SsidScan => {
                self.screen = Screen::SsidList;
                Intent::None
            }
            Screen::ServerProto => {
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
            Screen::Device | Screen::DeviceNameEdit | Screen::DeviceIdEdit | Screen::Language => {
                self.screen = Screen::Extras;
                Intent::None
            }
            Screen::RosterList
            | Screen::FunctionList
            | Screen::TurnoutList
            | Screen::RouteList
            | Screen::DirectCommands => {
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
        _scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
    ) -> Intent {
        if self.screen == Screen::Menu && !self.menu_cmd.is_empty() {
            if k <= 9 {
                let c = (b'0' + k) as char;
                let _ = self.menu_cmd.push(c);
            }
            return Intent::None;
        }
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
            Screen::Menu if self.menu_cmd.is_empty() => {
                self.cursor = k.min(9) as usize;
                Intent::None
            }
            _ => Intent::None,
        }
    }

    fn on_fn_release(&mut self, k: u8, domain: &DomainState) -> Intent {
        if self.screen == Screen::FunctionList {
            let fi = k as usize + self.fn_page * 10;
            if fi < sizes::MAX_FUNCTIONS {
                return Intent::Function(fi as u8, false);
            }
        }
        let _ = domain;
        Intent::None
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
                | Screen::TurnoutList
                | Screen::RouteList
                | Screen::Device
                | Screen::Language
                | Screen::DirectCommands
        )
    }

    fn list_count(
        &self,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
    ) -> usize {
        match self.screen {
            Screen::SsidList => config::network::NETWORKS.len(),
            Screen::SsidScan => scanned.len().saturating_sub(self.page * 5).min(5),
            Screen::ServerList => 5,
            Screen::ServerProto => 2,
            Screen::Menu => 10,
            Screen::Extras => 10,
            Screen::RosterList => domain.roster.len().saturating_sub(self.page * 5).min(5),
            Screen::FunctionList => 10,
            Screen::TurnoutList => domain.turnouts.len().saturating_sub(self.page * 10).min(10),
            Screen::RouteList => domain.routes.len().saturating_sub(self.page * 10).min(10),
            Screen::Device => 3,
            Screen::Language => 3,
            Screen::DirectCommands => 6,
            _ => 0,
        }
    }

    fn list_nav(
        &mut self,
        dir: NavDir,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
    ) -> Intent {
        let count = self.list_count(domain, scanned);
        if count == 0 {
            return Intent::None;
        }
        match dir {
            NavDir::Up => {
                if self.cursor == 0 {
                    self.cursor = count - 1;
                } else {
                    self.cursor -= 1;
                }
            }
            NavDir::Down => {
                self.cursor = (self.cursor + 1) % count;
            }
            NavDir::Right => return self.list_page_next(domain, scanned),
            NavDir::Left => return self.list_page_prev(),
        }
        Intent::None
    }

    fn list_page_next(
        &mut self,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
    ) -> Intent {
        match self.screen {
            Screen::SsidList => {
                self.screen = Screen::SsidScan;
                self.page = 0;
                self.cursor = 0;
                Intent::WifiScan
            }
            Screen::SsidScan if scanned.len() > (self.page + 1) * 5 => {
                self.page += 1;
                self.cursor = 0;
                Intent::None
            }
            Screen::RosterList if roster_pages(domain) > 1 => {
                self.page = (self.page + 1) % roster_pages(domain);
                self.cursor = 0;
                Intent::None
            }
            Screen::FunctionList => {
                self.fn_page += 1;
                if self.fn_page > 3 {
                    self.fn_page = 0;
                    self.screen = Screen::Throttle;
                }
                self.cursor = 0;
                Intent::None
            }
            Screen::TurnoutList | Screen::RouteList => {
                self.page += 1;
                self.cursor = 0;
                Intent::None
            }
            Screen::ServerList => {
                self.screen = Screen::ServerProto;
                Intent::None
            }
            _ => Intent::None,
        }
    }

    fn list_page_prev(&mut self) -> Intent {
        if self.page > 0 {
            self.page -= 1;
            self.cursor = 0;
        }
        Intent::None
    }

    fn ok_throttle(&mut self, domain: &DomainState) -> Intent {
        if domain.current_slot_has_loco() {
            if self.hash_functions {
                self.screen = Screen::FunctionList;
                self.fn_page = 0;
                self.cursor = 0;
            } else {
                self.screen = Screen::DirectCommands;
                self.cursor = 0;
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
        if !self.menu_cmd.is_empty() {
            return self.finish_menu();
        }
        let c = MENU_KEYS[self.cursor];
        if self.menu_cmd.is_empty() {
            if let Some(idx) = MENU_KEYS.iter().position(|&k| k == c) {
                match MENU_TYPES[idx] {
                    MenuItemType::Direct => {
                        let _ = self.menu_cmd.push(c);
                        return self.finish_menu();
                    }
                    MenuItemType::SubMenu if c == '9' => {
                        self.screen = Screen::Extras;
                        self.cursor = 0;
                        return Intent::None;
                    }
                    MenuItemType::OneOrMore | MenuItemType::SubMenu => {
                        let _ = self.menu_cmd.push(c);
                        return Intent::None;
                    }
                }
            }
        }
        Intent::None
    }

    fn ok_extras(&mut self) -> Intent {
        match self.cursor {
            0 => {
                self.screen = Screen::IpConfig;
                Intent::None
            }
            1 => {
                self.screen = Screen::Device;
                self.cursor = 0;
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
                self.screen = Screen::Throttle;
                Intent::SaveLocos
            }
            9 => {
                self.screen = Screen::Language;
                self.cursor = 0;
                Intent::None
            }
            _ => Intent::None,
        }
    }

    fn ok_ssid_list(&mut self) -> Intent {
        if self.cursor < config::network::NETWORKS.len() {
            self.selected_ssid_idx = self.cursor;
            self.selected_from_scan = false;
            self.screen = Screen::Connecting;
            Intent::WifiConnect
        } else {
            Intent::None
        }
    }

    fn ok_ssid_scan(
        &mut self,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
    ) -> Intent {
        self.selected_ssid_idx = self.page * 5 + self.cursor;
        self.selected_from_scan = true;
        self.selected_ssid.clear();
        if let Some(s) = scanned.get(self.selected_ssid_idx) {
            let _ = self.selected_ssid.push_str(s.ssid.as_str());
            if known_password(domain, s.ssid.as_str()) {
                self.screen = Screen::Connecting;
                return Intent::WifiConnect;
            }
        }
        self.screen = Screen::Password;
        self.text_kbd.clear();
        self.text_kbd.mode = KeyboardMode::Text;
        self.pw.clear();
        Intent::None
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

    fn ok_server_list(&mut self) -> Intent {
        Intent::ServerSelect(self.cursor)
    }

    fn ok_server_proto(&mut self) -> Intent {
        match self.cursor {
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
        self.ip_digits.clear();
        let _ = self.ip_digits.push_str(self.ip_kbd.buffer.as_str());
        if self.ip_digits.len() == 17 {
            Intent::ServerManual
        } else {
            Intent::None
        }
    }

    fn ok_roster(&mut self, domain: &DomainState) -> Intent {
        let idx = self.page * 5 + self.cursor;
        self.screen = Screen::Throttle;
        Intent::AcquireRoster(idx.min(domain.roster.len()))
    }

    fn ok_fn_list(&mut self) -> Intent {
        Intent::Function((self.cursor as u8) + self.fn_page as u8 * 10, true)
    }

    fn ok_turnout(&mut self) -> Intent {
        let idx = self.page * 10 + self.cursor;
        self.screen = Screen::Throttle;
        let action = if self.turnout_throw {
            TurnoutAction::Throw
        } else {
            TurnoutAction::Close
        };
        Intent::Turnout(action, ListRef::Index(idx))
    }

    fn ok_route(&mut self) -> Intent {
        let idx = self.page * 10 + self.cursor;
        self.screen = Screen::Throttle;
        Intent::Route(ListRef::Index(idx))
    }

    fn ok_device(&mut self, domain: &DomainState) -> Intent {
        match self.cursor {
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
        let lang = match self.cursor {
            0 => Language::En,
            1 => Language::Pl,
            2 => Language::De,
            _ => return Intent::None,
        };
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
}

fn roster_pages(domain: &DomainState) -> usize {
    if domain.roster.is_empty() {
        1
    } else {
        (domain.roster.len() + 4) / 5
    }
}

fn known_password(domain: &DomainState, ssid: &str) -> bool {
    domain.persist.find_password(ssid).is_some()
        || config::network::NETWORKS
            .iter()
            .any(|n| n.ssid == ssid && !n.password.is_empty())
}
