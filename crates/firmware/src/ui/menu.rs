//! Menu and screen state machine (navigation logic, no I/O).

#[path = "menu_nav.rs"]
mod menu_nav;

use longfred_proto::command::Protocol;
use longfred_proto::model::TurnoutAction;
use longfred_proto::persist::{DEVICE_ID_MIN, DeviceIdentity, Language, StaticIpConfig};

use crate::config::{self, buttons, network, power, sizes};
use crate::domain::actions::Action;
use crate::domain::state::DomainState;
use crate::input::InputEvent;
use crate::net::SsidInfo;
use crate::ui::i18n;
use crate::ui::keyboard::{KeyboardMode, TextKeyboard};
use crate::ui::view::{GridView, Line, ThrottleView, UiView, ViewCtx};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Splash,
    SsidList,
    SsidScan,
    Password,
    ServerList,
    ServerProto,
    ServerEntry,
    Connecting,
    Throttle,
    Menu,
    Extras,
    RosterList,
    FunctionList,
    TurnoutList,
    RouteList,
    DirectCommands,
    IpConfig,
    IpEdit,
    Device,
    DeviceNameEdit,
    DeviceIdEdit,
    Language,
    FirmwareUpdate,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ListRef {
    Addr,
    Index(usize),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Intent {
    None,
    Action(Action),
    AcquireAddr,
    AcquireRoster(usize),
    ReleaseAll,
    Function(u8, bool),
    Turnout(TurnoutAction, ListRef),
    Route(ListRef),
    WifiScan,
    WifiSelect(usize, bool),
    WifiConnect,
    ServerSelect(usize),
    ServerManual,
    HeartbeatToggle,
    DropBeforeAcquireToggle,
    HashFunctionsToggle,
    Sleep,
    SaveLocos,
    RequestMdns,
    NetConfig,
    SaveNetwork(StaticIpConfig),
    SaveDevice(DeviceIdentity),
    RegenerateDeviceId,
    SetLanguage(Language),
    EnterProgrammingMode,
    SetHttpOta(bool),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BatteryMode {
    None,
    Icon,
    IconPercent,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuItemType {
    Direct,
    SubMenu,
    OneOrMore,
}

const MENU_KEYS: [char; 10] = ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];
const MENU_TYPES: [MenuItemType; 10] = [
    MenuItemType::OneOrMore,
    MenuItemType::OneOrMore,
    MenuItemType::OneOrMore,
    MenuItemType::Direct,
    MenuItemType::Direct,
    MenuItemType::OneOrMore,
    MenuItemType::OneOrMore,
    MenuItemType::OneOrMore,
    MenuItemType::Direct,
    MenuItemType::SubMenu,
];

pub struct MenuFsm {
    pub screen: Screen,
    menu_cmd: heapless::String<8>,
    page: usize,
    fn_page: usize,
    pub addr: heapless::String<5>,
    pw: heapless::String<64>,
    ip_digits: heapless::String<17>,
    hash_functions: bool,
    selected_ssid_idx: usize,
    selected_from_scan: bool,
    turnout_throw: bool,
    selected_ssid: heapless::String<32>,
    pending_password_save: bool,
    splash_done: bool,
    battery_mode: BatteryMode,
    net_cfg: StaticIpConfig,
    ip_field: u8,
    net_digits: heapless::String<12>,
    manual_protocol: Protocol,
    device_name_edit: heapless::String<32>,
    device_id_digits: heapless::String<4>,
    cursor: usize,
    text_kbd: TextKeyboard<64>,
    addr_kbd: TextKeyboard<5>,
    ip_kbd: TextKeyboard<17>,
    net_kbd: TextKeyboard<12>,
    id_kbd: TextKeyboard<4>,
}

impl MenuFsm {
    pub fn new() -> Self {
        Self {
            screen: Screen::Splash,
            menu_cmd: heapless::String::new(),
            page: 0,
            fn_page: 0,
            addr: heapless::String::new(),
            pw: heapless::String::new(),
            ip_digits: heapless::String::new(),
            hash_functions: buttons::HASH_SHOWS_FUNCTIONS_INSTEAD_OF_KEY_DEFS,
            selected_ssid_idx: 0,
            selected_from_scan: false,
            turnout_throw: true,
            selected_ssid: heapless::String::new(),
            pending_password_save: false,
            splash_done: false,
            battery_mode: if power::USE_BATTERY_TEST {
                if power::USE_BATTERY_PERCENT_WITH_ICON {
                    BatteryMode::IconPercent
                } else {
                    BatteryMode::Icon
                }
            } else {
                BatteryMode::None
            },
            net_cfg: StaticIpConfig::default(),
            ip_field: 0,
            net_digits: heapless::String::new(),
            manual_protocol: Protocol::WiThrottle,
            device_name_edit: heapless::String::new(),
            device_id_digits: heapless::String::new(),
            cursor: 0,
            text_kbd: TextKeyboard::new(KeyboardMode::Text),
            addr_kbd: TextKeyboard::new(KeyboardMode::Digits),
            ip_kbd: TextKeyboard::new(KeyboardMode::Digits),
            net_kbd: TextKeyboard::new(KeyboardMode::Digits),
            id_kbd: TextKeyboard::new(KeyboardMode::Digits),
        }
    }

    pub fn tick_splash(&mut self) -> Intent {
        if self.screen == Screen::Splash && !self.splash_done {
            self.splash_done = true;
            if network::AUTO_CONNECT_TO_FIRST_DEFINED_SERVER
                && !config::network::NETWORKS.is_empty()
            {
                self.screen = Screen::Connecting;
                Intent::WifiConnect
            } else {
                self.screen = Screen::SsidList;
                Intent::None
            }
        } else {
            Intent::None
        }
    }

    pub fn on_wifi_ready(&mut self) {
        if matches!(
            self.screen,
            Screen::Connecting | Screen::Password | Screen::SsidList | Screen::SsidScan
        ) {
            self.screen = Screen::ServerList;
        }
    }

    pub fn on_scan_done(&mut self) {
        if self.screen == Screen::SsidList {
            self.screen = Screen::SsidScan;
            self.page = 0;
        }
    }

    pub fn on_server_connected(&mut self) {
        if !matches!(
            self.screen,
            Screen::Menu
                | Screen::Extras
                | Screen::RosterList
                | Screen::FunctionList
                | Screen::TurnoutList
                | Screen::RouteList
                | Screen::DirectCommands
                | Screen::IpConfig
                | Screen::IpEdit
                | Screen::Device
                | Screen::DeviceNameEdit
                | Screen::DeviceIdEdit
                | Screen::Language
                | Screen::FirmwareUpdate
        ) {
            self.screen = Screen::Throttle;
        }
    }

    pub fn handle(
        &mut self,
        ev: InputEvent,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
    ) -> Intent {
        self.handle_input(ev, domain, scanned)
    }

    pub(crate) fn finish_menu(&mut self) -> Intent {
        if self.menu_cmd.is_empty() {
            return Intent::None;
        }
        use longfred_proto::menu::{MenuFinish, finish_menu as finish};
        let result = finish(self.menu_cmd.as_str());
        self.menu_cmd.clear();
        match result {
            MenuFinish::None => Intent::None,
            MenuFinish::AcquireAddr(addr) => {
                self.addr.clear();
                let _ = self.addr.push_str(addr.as_str());
                self.screen = Screen::Throttle;
                Intent::AcquireAddr
            }
            MenuFinish::RosterList => {
                self.screen = Screen::RosterList;
                self.page = 0;
                Intent::None
            }
            MenuFinish::ReleaseAll => {
                self.screen = Screen::Throttle;
                Intent::ReleaseAll
            }
            MenuFinish::DirectionToggle => {
                self.screen = Screen::Throttle;
                Intent::Action(Action::DirectionToggle)
            }
            MenuFinish::SpeedMultiplier => {
                self.screen = Screen::Throttle;
                Intent::Action(Action::SpeedMultiplier)
            }
            MenuFinish::TurnoutThrowAddr(addr) => {
                self.addr.clear();
                let _ = self.addr.push_str(addr.as_str());
                self.screen = Screen::Throttle;
                Intent::Turnout(TurnoutAction::Throw, ListRef::Addr)
            }
            MenuFinish::TurnoutCloseAddr(addr) => {
                self.addr.clear();
                let _ = self.addr.push_str(addr.as_str());
                self.screen = Screen::Throttle;
                Intent::Turnout(TurnoutAction::Close, ListRef::Addr)
            }
            MenuFinish::TurnoutList { throw } => {
                self.turnout_throw = throw;
                self.screen = Screen::TurnoutList;
                self.page = 0;
                Intent::None
            }
            MenuFinish::RouteAddr(addr) => {
                self.addr.clear();
                let _ = self.addr.push_str(addr.as_str());
                self.screen = Screen::Throttle;
                Intent::Route(ListRef::Addr)
            }
            MenuFinish::RouteList => {
                self.screen = Screen::RouteList;
                self.page = 0;
                Intent::None
            }
            MenuFinish::PowerToggle => {
                self.screen = Screen::Throttle;
                Intent::Action(Action::PowerToggle)
            }
            MenuFinish::FunctionPress(f) => {
                self.screen = Screen::Throttle;
                Intent::Function(f, true)
            }
            MenuFinish::FunctionList => {
                self.screen = Screen::FunctionList;
                self.fn_page = 0;
                Intent::None
            }
        }
    }

    pub(crate) fn begin_server_entry(&mut self, protocol: Protocol) {
        self.manual_protocol = protocol;
        self.ip_digits.clear();
        let ip = match protocol {
            Protocol::WiThrottle => network::DEFAULT_WIT_IP,
            Protocol::Z21 => network::DEFAULT_Z21_IP,
        };
        let port = match protocol {
            Protocol::WiThrottle => network::DEFAULT_WIT_PORT,
            Protocol::Z21 => network::DEFAULT_Z21_PORT,
        };
        push_ip_octet(&mut self.ip_digits, ip[0]);
        push_ip_octet(&mut self.ip_digits, ip[1]);
        push_ip_octet(&mut self.ip_digits, ip[2]);
        push_ip_octet(&mut self.ip_digits, ip[3]);
        push_port_digits(&mut self.ip_digits, port);
        self.ip_kbd.clear();
        let _ = self.ip_kbd.buffer.push_str(self.ip_digits.as_str());
        self.screen = Screen::ServerEntry;
    }

    pub fn manual_protocol(&self) -> Protocol {
        self.manual_protocol
    }

    pub(crate) fn begin_ip_edit(&mut self, domain: &DomainState) {
        self.net_cfg = domain.persist.network.unwrap_or(StaticIpConfig::default());
        self.ip_field = 0;
        self.net_digits.clear();
        self.load_net_field_digits();
        self.sync_net_kbd_from_digits();
        self.screen = Screen::IpEdit;
    }

    fn sync_net_kbd_from_digits(&mut self) {
        self.net_kbd.clear();
        let _ = self.net_kbd.buffer.push_str(self.net_digits.as_str());
    }

    pub(crate) fn sync_digits_from_net_kbd(&mut self) {
        self.net_digits.clear();
        let _ = self.net_digits.push_str(self.net_kbd.buffer.as_str());
    }

    fn load_net_field_digits(&mut self) {
        self.net_digits.clear();
        match self.ip_field {
            0 => {
                let _ = self
                    .net_digits
                    .push(if self.net_cfg.dhcp { '0' } else { '1' });
            }
            1 => push_ip_digits(&mut self.net_digits, self.net_cfg.ip),
            2 => {
                let _ = self
                    .net_digits
                    .push((b'0' + self.net_cfg.prefix_len / 10) as char);
                let _ = self
                    .net_digits
                    .push((b'0' + self.net_cfg.prefix_len % 10) as char);
            }
            3 => {
                if let Some(gw) = self.net_cfg.gateway {
                    push_ip_digits(&mut self.net_digits, gw);
                }
            }
            4 => {
                if let Some(dns) = self.net_cfg.dns {
                    push_ip_digits(&mut self.net_digits, dns);
                }
            }
            _ => {}
        }
    }

    fn commit_net_field(&mut self) {
        match self.ip_field {
            0 => {
                self.net_cfg.dhcp = self.net_digits.as_bytes().first() != Some(&b'1');
            }
            1 => {
                if let Some(ip) = parse_ip_digits(self.net_digits.as_str()) {
                    self.net_cfg.ip = ip;
                    self.auto_fill_from_ip();
                }
            }
            2 => {
                if self.net_digits.is_empty() {
                    self.net_cfg.prefix_len = network::DEFAULT_PREFIX_LEN;
                } else if self.net_digits.len() <= 2 {
                    let mut prefix = 0u8;
                    for b in self.net_digits.as_bytes() {
                        prefix = prefix.saturating_mul(10).saturating_add(b - b'0');
                    }
                    if prefix <= 32 {
                        self.net_cfg.prefix_len = prefix;
                    }
                }
            }
            3 => {
                if self.net_digits.is_empty() {
                    self.net_cfg.gateway = None;
                } else if let Some(gw) = parse_ip_digits(self.net_digits.as_str()) {
                    self.net_cfg.gateway = Some(gw);
                }
            }
            4 => {
                if self.net_digits.is_empty() {
                    self.net_cfg.dns = None;
                } else if let Some(dns) = parse_ip_digits(self.net_digits.as_str()) {
                    self.net_cfg.dns = Some(dns);
                }
            }
            _ => {}
        }
    }

    fn auto_fill_from_ip(&mut self) {
        if self.net_cfg.prefix_len == 0 {
            self.net_cfg.prefix_len = network::DEFAULT_PREFIX_LEN;
        }
        if self.net_cfg.gateway.is_none() {
            let mut gw = self.net_cfg.ip;
            gw[3] = 1;
            self.net_cfg.gateway = Some(gw);
        }
    }

    pub(crate) fn ip_edit_advance(&mut self) -> Intent {
        self.sync_digits_from_net_kbd();
        self.commit_net_field();
        if self.ip_field == 0 && self.net_cfg.dhcp {
            self.screen = Screen::Throttle;
            return Intent::SaveNetwork(self.net_cfg);
        }
        if self.ip_field >= 4 {
            self.screen = Screen::Throttle;
            return Intent::SaveNetwork(self.net_cfg);
        }
        self.ip_field += 1;
        self.net_digits.clear();
        self.load_net_field_digits();
        self.sync_net_kbd_from_digits();
        Intent::None
    }

    pub fn format_net_display(&self) -> heapless::String<24> {
        let mut s = heapless::String::new();
        let label = match self.ip_field {
            0 => "Mode",
            1 => "IP",
            2 => "Mask",
            3 => "GW",
            4 => "DNS",
            _ => "?",
        };
        let _ = s.push_str(label);
        let _ = s.push(' ');
        if self.ip_field == 0 {
            if self.net_digits.is_empty() {
                let _ = s.push(if self.net_cfg.dhcp { '0' } else { '1' });
            } else {
                let _ = s.push(self.net_digits.as_bytes()[0] as char);
            }
            let _ = s.push_str(
                if self.net_cfg.dhcp || self.net_digits.as_bytes().first() == Some(&b'0') {
                    " DHCP"
                } else {
                    " Static"
                },
            );
            return s;
        }
        if self.ip_field == 2 {
            let _ = s.push_str(self.net_digits.as_str());
            return s;
        }
        let d = self.net_digits.as_str();
        if d.len() >= 3 {
            let _ = s.push_str(&d[0..3]);
            let _ = s.push('.');
        }
        if d.len() >= 6 {
            let _ = s.push_str(&d[3..6]);
            let _ = s.push('.');
        }
        if d.len() >= 9 {
            let _ = s.push_str(&d[6..9]);
            let _ = s.push('.');
        }
        if d.len() > 9 {
            let _ = s.push_str(&d[9..]);
        }
        s
    }

    pub(crate) fn begin_device_name_edit(&mut self, domain: &DomainState) {
        self.text_kbd.clear();
        self.text_kbd.mode = KeyboardMode::Text;
        let _ = self
            .text_kbd
            .buffer
            .push_str(domain.persist.device.name.as_str());
        self.device_name_edit.clear();
        let _ = self
            .device_name_edit
            .push_str(domain.persist.device.name.as_str());
        self.screen = Screen::DeviceNameEdit;
    }

    pub(crate) fn begin_device_id_edit(&mut self, domain: &DomainState) {
        self.id_kbd.clear();
        let id = domain.persist.device.id;
        if id >= DEVICE_ID_MIN {
            let digits = [
                ((id / 1000) % 10) as u8 + b'0',
                ((id / 100) % 10) as u8 + b'0',
                ((id / 10) % 10) as u8 + b'0',
                (id % 10) as u8 + b'0',
            ];
            for d in digits {
                let _ = self.id_kbd.buffer.push(d as char);
            }
        }
        self.device_id_digits.clear();
        let _ = self.device_id_digits.push_str(self.id_kbd.buffer.as_str());
        self.screen = Screen::DeviceIdEdit;
    }

    pub(crate) fn encoder(&mut self, cw: bool, domain: &DomainState) -> Intent {
        if matches!(self.screen, Screen::Password | Screen::DeviceNameEdit) {
            if cw {
                let _ = self.text_kbd.nav_up();
            } else {
                let _ = self.text_kbd.nav_down();
            }
            return Intent::None;
        }
        if matches!(
            self.screen,
            Screen::ServerEntry | Screen::IpEdit | Screen::DeviceIdEdit
        ) {
            if cw {
                let _ = match self.screen {
                    Screen::ServerEntry => self.ip_kbd.nav_up(),
                    Screen::IpEdit => self.net_kbd.nav_up(),
                    Screen::DeviceIdEdit => self.id_kbd.nav_up(),
                    _ => crate::ui::keyboard::KeyboardAction::None,
                };
            } else {
                let _ = match self.screen {
                    Screen::ServerEntry => self.ip_kbd.nav_down(),
                    Screen::IpEdit => self.net_kbd.nav_down(),
                    Screen::DeviceIdEdit => self.id_kbd.nav_down(),
                    _ => crate::ui::keyboard::KeyboardAction::None,
                };
            }
            return Intent::None;
        }
        if self.screen == Screen::Throttle && !domain.current_slot_has_loco() {
            if cw {
                let _ = self.addr_kbd.nav_up();
            } else {
                let _ = self.addr_kbd.nav_down();
            }
            return Intent::None;
        }
        if self.screen == Screen::Throttle && domain.current_slot_has_loco() {
            let mut inc = cw == buttons::ENCODER_CLOCKWISE_INCREASES_SPEED;
            if buttons::ENCODER_INVERT_WHEN_REVERSED && !domain.current_forward() {
                inc = !inc;
            }
            return if inc {
                Intent::Action(Action::SpeedUp)
            } else {
                Intent::Action(Action::SpeedDown)
            };
        }
        Intent::None
    }

    pub(crate) fn encoder_button(&mut self, domain: &DomainState) -> Intent {
        if matches!(self.screen, Screen::Password | Screen::DeviceNameEdit) {
            let _ = self.text_kbd.ok();
            return Intent::None;
        }
        if self.screen == Screen::Throttle && domain.current_slot_has_loco() {
            return Intent::Action(buttons::ENCODER_BUTTON_ACTION);
        }
        Intent::None
    }

    pub fn pw_picker_char(&self) -> u8 {
        self.text_kbd
            .pending
            .map(|c| c as u8)
            .unwrap_or(i18n::PW_BLANK_CHAR)
    }

    pub fn ssid_for_connect<'a>(
        &'a self,
        scanned: &'a heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        domain: &'a DomainState,
    ) -> (&'a str, &'a str) {
        if self.selected_from_scan {
            if let Some(s) = scanned.get(self.selected_ssid_idx) {
                if !self.pw.is_empty() {
                    (s.ssid.as_str(), self.pw.as_str())
                } else if let Some(stored) = domain.persist.find_password(s.ssid.as_str()) {
                    (s.ssid.as_str(), stored)
                } else {
                    (s.ssid.as_str(), "")
                }
            } else if !self.selected_ssid.is_empty() {
                if !self.pw.is_empty() {
                    (self.selected_ssid.as_str(), self.pw.as_str())
                } else if let Some(stored) =
                    domain.persist.find_password(self.selected_ssid.as_str())
                {
                    (self.selected_ssid.as_str(), stored)
                } else {
                    (self.selected_ssid.as_str(), "")
                }
            } else {
                ("", "")
            }
        } else if let Some(n) = config::network::NETWORKS.get(self.selected_ssid_idx) {
            (n.ssid, n.password)
        } else {
            ("", "")
        }
    }

    pub fn take_pending_password_save(
        &mut self,
    ) -> Option<(heapless::String<32>, heapless::String<64>)> {
        if !self.pending_password_save {
            return None;
        }
        self.pending_password_save = false;
        let mut ssid = heapless::String::new();
        let mut pw = heapless::String::new();
        let _ = ssid.push_str(self.selected_ssid.as_str());
        let _ = pw.push_str(self.pw.as_str());
        Some((ssid, pw))
    }

    pub fn cycle_battery_mode(&mut self) {
        if !power::USE_BATTERY_TEST {
            return;
        }
        self.battery_mode = match self.battery_mode {
            BatteryMode::Icon => BatteryMode::IconPercent,
            BatteryMode::IconPercent => BatteryMode::None,
            BatteryMode::None => BatteryMode::Icon,
        };
    }

    pub fn battery_show_percent(&self) -> bool {
        self.battery_mode == BatteryMode::IconPercent
    }

    pub fn battery_visible(&self) -> bool {
        self.battery_mode != BatteryMode::None
    }

    pub fn password_preview(&self) -> heapless::String<24> {
        let mut s = heapless::String::new();
        let _ = s.push(' ');
        let preview = self.text_kbd.preview();
        let _ = s.push_str(preview.as_str());
        s
    }

    pub fn device_name_preview(&self) -> heapless::String<36> {
        let mut s = heapless::String::new();
        let _ = s.push(' ');
        let preview = self.text_kbd.preview();
        let _ = s.push_str(preview.as_str());
        s
    }

    pub fn format_device_id_display(&self) -> heapless::String<8> {
        let mut s = heapless::String::new();
        let preview = self.id_kbd.preview();
        let _ = s.push_str(preview.as_str());
        s
    }

    pub fn toggle_hash_functions(&mut self) {
        self.hash_functions = !self.hash_functions;
    }

    pub fn hash_functions_enabled(&self) -> bool {
        self.hash_functions
    }

    pub fn ip_endpoint(&self) -> Option<([u8; 4], u16)> {
        longfred_proto::menu::parse_ip_endpoint(self.ip_digits.as_str())
    }

    pub fn format_ip_display(&self) -> heapless::String<24> {
        let mut s = heapless::String::new();
        let d = if self.screen == Screen::ServerEntry {
            self.ip_kbd.buffer.as_str()
        } else {
            self.ip_digits.as_str()
        };
        if d.len() >= 3 {
            let _ = s.push_str(&d[0..3]);
            let _ = s.push('.');
        }
        if d.len() >= 6 {
            let _ = s.push_str(&d[3..6]);
            let _ = s.push('.');
        }
        if d.len() >= 9 {
            let _ = s.push_str(&d[6..9]);
            let _ = s.push('.');
        }
        if d.len() >= 12 {
            let _ = s.push_str(&d[9..12]);
            let _ = s.push(':');
        }
        if d.len() > 12 {
            let _ = s.push_str(&d[12..]);
        }
        s
    }

    pub fn view(&self, ctx: &ViewCtx<'_>) -> UiView {
        match self.screen {
            Screen::Throttle => UiView::Throttle(self.build_throttle(ctx)),
            _ => UiView::Grid(self.build_grid(ctx)),
        }
    }

    fn build_throttle(&self, ctx: &ViewCtx<'_>) -> ThrottleView {
        let slot = ctx.domain.current_slot();
        let mut loco = Line::new();
        if slot.has_loco() {
            for (i, a) in slot.consist.iter().enumerate() {
                if i > 0 {
                    let _ = loco.push(' ');
                }
                let _ = loco.push_str(a.as_str());
            }
        } else if !self.addr_kbd.buffer.is_empty() {
            let _ = loco.push_str("addr:");
            let preview = self.addr_kbd.preview();
            let _ = loco.push_str(preview.as_str());
        } else {
            let _ = loco.push_str(i18n::tr().msg_no_loco);
        }
        let mut footer = Line::new();
        if let Some(b) = ctx.broadcast {
            let _ = footer.push_str(b);
        } else {
            let _ = footer.push_str(i18n::tr().hint_throttle);
        }
        let mut functions = 0u32;
        for (i, on) in slot.functions.iter().enumerate() {
            if *on {
                functions |= 1 << i;
            }
        }
        ThrottleView {
            current: ctx.domain.current as u8,
            speed: slot.speed,
            forward: slot.direction == longfred_proto::model::Direction::Forward,
            consist_len: slot.consist.len() as u8,
            power_on: ctx.domain.track_power_on(),
            heartbeat_on: ctx.domain.heartbeat_enabled(),
            functions,
            loco,
            footer,
            next_hint: Line::new(),
            battery: if self.battery_visible() {
                ctx.battery
            } else {
                None
            },
            battery_show_percent: self.battery_show_percent(),
        }
    }

    fn build_grid(&self, ctx: &ViewCtx<'_>) -> GridView {
        let mut g = GridView::new();
        g.foot_line = true;
        match self.screen {
            Screen::Splash => {
                g.set(0, i18n::APP_NAME, false);
                g.set(1, i18n::FW_VERSION, false);
                g.set(5, i18n::tr().msg_booting, false);
            }
            Screen::SsidList => {
                g.set(0, i18n::tr().msg_ssids_listed, false);
                for (i, n) in config::network::NETWORKS.iter().enumerate().take(10) {
                    let mut line = Line::new();
                    let _ = line.push((b'0' + i as u8) as char);
                    let _ = line.push(':');
                    let _ = line.push(' ');
                    let _ = line.push_str(n.ssid);
                    g.set(i + 1, line.as_str(), self.cursor == i);
                }
                g.set(5, i18n::tr().hint_select_ssids, false);
            }
            Screen::SsidScan => {
                g.set(0, i18n::tr().msg_ssids_found, false);
                for i in 0..5 {
                    if let Some(s) = ctx.scanned_ssids.get(self.page * 5 + i) {
                        let mut line = Line::new();
                        let _ = line.push((b'0' + i as u8) as char);
                        let _ = line.push(':');
                        let _ = line.push_str(s.ssid.as_str());
                        g.set(i + 1, line.as_str(), self.cursor == i);
                    }
                }
                g.set(5, i18n::tr().hint_select_found, false);
            }
            Screen::Password => {
                g.set(0, i18n::tr().msg_enter_password, false);
                g.set(2, ctx.password_preview, false);
                g.set(5, i18n::tr().hint_enter_password, false);
            }
            Screen::ServerList => {
                g.set(0, i18n::tr().msg_services_found, false);
                for i in 0..5 {
                    if let Some(s) = ctx.found_servers.get(i) {
                        let mut line = Line::new();
                        let _ = line.push((b'0' + i as u8) as char);
                        let _ = line.push(':');
                        let _ = line.push_str(s.label.as_str());
                        let _ = line.push(' ');
                        let tag = match s.protocol {
                            Protocol::WiThrottle => 'W',
                            Protocol::Z21 => 'Z',
                        };
                        let _ = line.push(tag);
                        g.set(i + 1, line.as_str(), self.cursor == i);
                    }
                }
                g.set(5, i18n::tr().hint_select_wit, false);
            }
            Screen::ServerProto => {
                g.set(0, i18n::tr().msg_select_proto, false);
                g.set(1, i18n::tr().proto_wit, self.cursor == 0);
                g.set(2, i18n::tr().proto_z21, self.cursor == 1);
                g.set(5, i18n::tr().hint_proto, false);
            }
            Screen::ServerEntry => {
                g.set(0, i18n::tr().msg_enter_server_ip, false);
                g.set(2, ctx.ip_formatted, false);
                g.set(5, i18n::tr().hint_wit_entry, false);
            }
            Screen::Connecting => {
                g.set(1, i18n::tr().msg_trying_connect, false);
                g.set(2, ctx.selected_ssid, false);
            }
            Screen::Menu => {
                let rows = [
                    ('0', i18n::tr().menu_fn),
                    ('1', i18n::tr().menu_add),
                    ('2', i18n::tr().menu_drop),
                    ('3', i18n::tr().menu_toggle_dir),
                    ('4', i18n::tr().menu_speed_mult),
                    ('5', i18n::tr().menu_throw),
                    ('6', i18n::tr().menu_close),
                    ('7', i18n::tr().menu_route),
                    ('8', i18n::tr().menu_power),
                    ('9', i18n::tr().menu_extras),
                ];
                for (i, (k, label)) in rows.iter().enumerate() {
                    let mut line = Line::new();
                    let _ = line.push(*k);
                    let _ = line.push(' ');
                    let _ = line.push_str(label);
                    let col = if i < 5 { i + 1 } else { i + 2 };
                    g.set(col, line.as_str(), false);
                }
                g.set(5, i18n::tr().hint_menu, false);
            }
            Screen::Extras => {
                g.set(0, i18n::tr().extras_net_config, self.cursor == 0);
                g.set(1, i18n::tr().extras_device, self.cursor == 1);
                g.set(2, i18n::tr().extras_fnc_key_tgl, self.cursor == 2);
                g.set(3, i18n::tr().extras_heartbt_tgl, self.cursor == 3);
                g.set(4, i18n::tr().extras_throttles_plus, self.cursor == 4);
                g.set(6, i18n::tr().extras_throttles_minus, self.cursor == 5);
                g.set(7, i18n::tr().extras_off_sleep, self.cursor == 6);
                g.set(8, i18n::tr().extras_one_loco_tgl, self.cursor == 7);
                g.set(9, i18n::tr().extras_save_locos, self.cursor == 8);
                g.set(10, i18n::tr().extras_language, self.cursor == 9);
                g.set(5, i18n::tr().extras_firmware, self.cursor == 10);
            }
            Screen::FirmwareUpdate => {
                g.set(0, i18n::tr().msg_fw_update, false);
                if let Some(ip) = ctx.sta_ipv4 {
                    let mut line = Line::new();
                    write_ip_line(&mut line, ip);
                    g.set(1, line.as_str(), false);
                } else {
                    g.set(1, i18n::tr().msg_fw_no_ip, false);
                }
                g.set(
                    2,
                    if ctx.http_ota {
                        i18n::tr().msg_fw_http_on
                    } else {
                        i18n::tr().msg_fw_http_off
                    },
                    false,
                );
                if ctx.http_ota_busy {
                    g.set(3, i18n::tr().msg_fw_updating, false);
                }
                g.set(5, i18n::tr().hint_fw_update, false);
            }
            Screen::IpConfig => {
                g.set(0, i18n::tr().msg_net_config, false);
                match ctx.domain.persist.network {
                    Some(n) if !n.dhcp => {
                        let mut line = Line::new();
                        let _ = line.push_str(i18n::tr().msg_net_static);
                        let _ = line.push(' ');
                        let _ = write_ip_line(&mut line, n.ip);
                        g.set(1, line.as_str(), false);
                    }
                    Some(_) => {
                        g.set(1, i18n::tr().msg_net_dhcp, false);
                    }
                    None => {
                        g.set(1, i18n::tr().msg_net_dhcp, false);
                    }
                }
                g.set(5, i18n::tr().hint_net_config, false);
            }
            Screen::IpEdit => {
                g.set(0, i18n::tr().msg_net_config, false);
                g.set(2, ctx.ip_formatted, false);
                g.set(5, i18n::tr().hint_net_edit, false);
            }
            Screen::Device => {
                g.set(0, i18n::tr().msg_device, false);
                let mut name_line = Line::new();
                let _ = name_line.push_str(i18n::tr().msg_device_name);
                let _ = name_line.push(' ');
                let _ = name_line.push_str(ctx.domain.persist.device.name.as_str());
                g.set(1, name_line.as_str(), false);
                let mut id_line = Line::new();
                let _ = id_line.push_str(i18n::tr().msg_device_id);
                let _ = id_line.push(' ');
                let id = ctx.domain.persist.device.id;
                if id >= DEVICE_ID_MIN {
                    let _ = write_u16_padded(&mut id_line, id);
                } else {
                    let _ = id_line.push_str("----");
                }
                g.set(2, id_line.as_str(), false);
                g.set(3, i18n::tr().device_name_id, self.cursor <= 1);
                g.set(4, i18n::tr().device_new_id, self.cursor == 2);
                g.set(5, i18n::tr().hint_device, false);
            }
            Screen::DeviceNameEdit => {
                g.set(0, i18n::tr().msg_device_name_edit, false);
                g.set(2, ctx.password_preview, false);
                g.set(5, i18n::tr().hint_device_name_edit, false);
            }
            Screen::DeviceIdEdit => {
                g.set(0, i18n::tr().msg_device_id_edit, false);
                g.set(2, ctx.ip_formatted, false);
                g.set(5, i18n::tr().hint_device_id_edit, false);
            }
            Screen::RosterList => {
                for i in 0..5 {
                    if let Some(e) = ctx.domain.roster.get(self.page * 5 + i) {
                        let mut line = Line::new();
                        let _ = line.push((b'0' + i as u8) as char);
                        let _ = line.push(':');
                        let _ = line.push_str(e.name.as_str());
                        g.set(i + 1, line.as_str(), self.cursor == i);
                    }
                }
                g.set(5, i18n::tr().hint_list, false);
            }
            Screen::FunctionList => {
                let slot = ctx.domain.current_slot();
                for i in 0..10 {
                    let fi = self.fn_page * 10 + i;
                    if fi < sizes::MAX_FUNCTIONS {
                        let mut line = Line::new();
                        let _ = line.push((b'0' + i as u8) as char);
                        let _ = line.push(':');
                        let _ = line.push_str(slot.labels[fi].as_str());
                        g.set(i + 1, line.as_str(), slot.functions[fi]);
                    }
                }
                g.set(5, i18n::tr().hint_list, false);
            }
            Screen::TurnoutList => {
                for i in 0..10 {
                    if let Some(e) = ctx.domain.turnouts.get(self.page * 10 + i) {
                        let mut line = Line::new();
                        let _ = line.push((b'0' + i as u8) as char);
                        let _ = line.push(':');
                        let _ = line.push_str(e.user_name.as_str());
                        g.set(i + 1, line.as_str(), self.cursor == i);
                    }
                }
                g.set(5, i18n::tr().hint_list, false);
            }
            Screen::RouteList => {
                for i in 0..10 {
                    if let Some(e) = ctx.domain.routes.get(self.page * 10 + i) {
                        let mut line = Line::new();
                        let _ = line.push((b'0' + i as u8) as char);
                        let _ = line.push(':');
                        let _ = line.push_str(e.user_name.as_str());
                        g.set(i + 1, line.as_str(), self.cursor == i);
                    }
                }
                g.set(5, i18n::tr().hint_list, false);
            }
            Screen::DirectCommands => {
                g.set(0, i18n::tr().direct_fn, false);
                g.set(1, i18n::tr().direct_next_thr, false);
                g.set(2, i18n::tr().direct_spd_mult, false);
                g.set(3, i18n::tr().direct_rev, false);
                g.set(4, i18n::tr().direct_estop, false);
                g.set(5, i18n::tr().direct_back, false);
            }
            Screen::Language => {
                g.set(0, i18n::tr().msg_language, false);
                g.set(1, i18n::tr().lang_en, self.cursor == 0);
                g.set(2, i18n::tr().lang_pl, self.cursor == 1);
                g.set(3, i18n::tr().lang_de, self.cursor == 2);
                g.set(5, i18n::tr().hint_language, false);
            }
            _ => {}
        }
        if let Some(b) = ctx.broadcast {
            g.set(5, b, false);
        }
        g
    }
}

fn push_ip_octet(buf: &mut heapless::String<17>, oct: u8) {
    let _ = buf.push((b'0' + oct / 100) as char);
    let _ = buf.push((b'0' + (oct / 10) % 10) as char);
    let _ = buf.push((b'0' + oct % 10) as char);
}

fn push_port_digits(buf: &mut heapless::String<17>, port: u16) {
    let _ = buf.push((b'0' + ((port / 10000) % 10) as u8) as char);
    let _ = buf.push((b'0' + ((port / 1000) % 10) as u8) as char);
    let _ = buf.push((b'0' + ((port / 100) % 10) as u8) as char);
    let _ = buf.push((b'0' + ((port / 10) % 10) as u8) as char);
    let _ = buf.push((b'0' + (port % 10) as u8) as char);
}

fn parse_ip_digits(digits: &str) -> Option<[u8; 4]> {
    if digits.len() != 12 {
        return None;
    }
    let oct = |s: &str| s.parse::<u8>().ok();
    Some([
        oct(&digits[0..3])?,
        oct(&digits[3..6])?,
        oct(&digits[6..9])?,
        oct(&digits[9..12])?,
    ])
}

fn push_ip_digits(buf: &mut heapless::String<12>, ip: [u8; 4]) {
    buf.clear();
    for o in ip {
        let _ = buf.push((b'0' + o / 100) as char);
        let _ = buf.push((b'0' + (o / 10) % 10) as char);
        let _ = buf.push((b'0' + o % 10) as char);
    }
}

fn write_ip_line(line: &mut Line, ip: [u8; 4]) {
    let _ = line.push((b'0' + ip[0] / 100) as char);
    let _ = line.push((b'0' + (ip[0] / 10) % 10) as char);
    let _ = line.push((b'0' + ip[0] % 10) as char);
    let _ = line.push('.');
    let _ = line.push((b'0' + ip[1] / 100) as char);
    let _ = line.push((b'0' + (ip[1] / 10) % 10) as char);
    let _ = line.push((b'0' + ip[1] % 10) as char);
    let _ = line.push('.');
    let _ = line.push((b'0' + ip[2] / 100) as char);
    let _ = line.push((b'0' + (ip[2] / 10) % 10) as char);
    let _ = line.push((b'0' + ip[2] % 10) as char);
    let _ = line.push('.');
    let _ = line.push((b'0' + ip[3] / 100) as char);
    let _ = line.push((b'0' + (ip[3] / 10) % 10) as char);
    let _ = line.push((b'0' + ip[3] % 10) as char);
}

fn write_u16_padded(line: &mut Line, n: u16) -> Result<(), ()> {
    let _ = line.push((b'0' + ((n / 1000) % 10) as u8) as char);
    let _ = line.push((b'0' + ((n / 100) % 10) as u8) as char);
    let _ = line.push((b'0' + ((n / 10) % 10) as u8) as char);
    let _ = line.push((b'0' + (n % 10) as u8) as char);
    Ok(())
}
