//! Menu and screen state machine (navigation logic, no I/O).

#[path = "menu_nav.rs"]
mod menu_nav;

use longfred_proto::command::Protocol;
use longfred_proto::persist::{DEVICE_ID_MIN, DeviceIdentity, Language, StaticIpConfig};

use crate::config::{self, buttons, network, power, sizes};
use crate::domain::actions::Action;
use crate::domain::state::DomainState;
use crate::input::InputEvent;
use crate::net::SsidInfo;
use crate::ui::i18n;
use crate::ui::keyboard::{KeyboardMode, TextKeyboard};
use crate::ui::paged_list::PagedList;
use crate::ui::view::{
    GridView, Line, ThrottleView, UiView, ViewCtx, fill_list_page, fill_list_page_invert,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Splash,
    SsidList,
    SsidScan,
    SsidScanning,
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
    DirectCommands,
    IpConfig,
    IpEdit,
    Device,
    DeviceNameEdit,
    DeviceIdEdit,
    Language,
    FirmwareUpdate,
    WifiFailed,
    Diagnostics,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Intent {
    None,
    Action(Action),
    AcquireAddr,
    AcquireRoster(usize),
    ReleaseAll,
    /// Toggle DCC function `0..=31` (press on, press again off).
    Function(u8),
    WifiScan,
    WifiSelect(usize, bool),
    WifiConnect,
    ServerSelect(usize),
    ServerManual,
    HeartbeatToggle,
    DropBeforeAcquireToggle,
    HashFunctionsToggle,
    Sleep,
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

pub(crate) const DIAG_PAGES: usize = 6;

pub struct MenuFsm {
    pub screen: Screen,
    /// Typed 1-based index while picking from a list longer than 9 items.
    list_num: heapless::String<3>,
    list: PagedList,
    fn_page: usize,
    pub addr: heapless::String<8>,
    pw: heapless::String<64>,
    ip_digits: heapless::String<17>,
    hash_functions: bool,
    selected_ssid_idx: usize,
    selected_from_scan: bool,
    selected_ssid: heapless::String<32>,
    pending_password_save: bool,
    splash_done: bool,
    boot_language: bool,
    server_entry_from_list: bool,
    battery_mode: BatteryMode,
    net_cfg: StaticIpConfig,
    ip_field: u8,
    net_digits: heapless::String<12>,
    manual_protocol: Protocol,
    device_name_edit: heapless::String<32>,
    device_id_digits: heapless::String<4>,
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
            list_num: heapless::String::new(),
            list: PagedList::new(true),
            fn_page: 0,
            addr: heapless::String::new(),
            pw: heapless::String::new(),
            ip_digits: heapless::String::new(),
            hash_functions: buttons::HASH_SHOWS_FUNCTIONS_INSTEAD_OF_KEY_DEFS,
            selected_ssid_idx: 0,
            selected_from_scan: false,
            selected_ssid: heapless::String::new(),
            pending_password_save: false,
            splash_done: false,
            boot_language: false,
            server_entry_from_list: false,
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
            text_kbd: TextKeyboard::new(KeyboardMode::Text),
            addr_kbd: TextKeyboard::new(KeyboardMode::Digits),
            ip_kbd: TextKeyboard::new(KeyboardMode::Digits),
            net_kbd: TextKeyboard::new(KeyboardMode::Digits),
            id_kbd: TextKeyboard::new(KeyboardMode::Digits),
        }
    }

    pub fn begin_language_wizard(&mut self) {
        self.splash_done = true;
        self.boot_language = true;
        self.screen = Screen::Language;
        self.list.cursor = 0;
    }

    /// After splash / language: try last NVS credential or open a live scan.
    pub fn begin_wifi_setup(&mut self, last_ssid: Option<&str>) -> Intent {
        self.splash_done = true;
        self.boot_language = false;
        self.list.cursor = 0;
        self.list.page = 0;
        if let Some(ssid) = last_ssid {
            self.selected_ssid.clear();
            let _ = self.selected_ssid.push_str(ssid);
            self.selected_from_scan = false;
            self.screen = Screen::Connecting;
            Intent::WifiConnect
        } else {
            self.selected_ssid.clear();
            self.screen = Screen::SsidScanning;
            Intent::WifiScan
        }
    }

    pub fn show_wifi_failed(&mut self) {
        self.screen = Screen::WifiFailed;
    }

    pub fn show_ssid_scan(&mut self) -> Intent {
        self.screen = Screen::SsidScanning;
        self.list.page = 0;
        self.list.cursor = 0;
        Intent::WifiScan
    }

    pub fn skip_wifi_to_servers(&mut self) -> Intent {
        self.screen = Screen::ServerList;
        self.list.page = 0;
        self.list.cursor = 0;
        Intent::RequestMdns
    }

    pub fn show_server_list(&mut self) -> Intent {
        self.screen = Screen::ServerList;
        self.list.page = 0;
        self.list.cursor = 0;
        Intent::RequestMdns
    }

    pub fn on_wifi_ready(&mut self) {
        if matches!(
            self.screen,
            Screen::Connecting
                | Screen::Password
                | Screen::SsidList
                | Screen::SsidScan
                | Screen::SsidScanning
                | Screen::WifiFailed
        ) {
            self.screen = Screen::ServerList;
        }
    }

    pub fn on_scan_done(&mut self) {
        if matches!(
            self.screen,
            Screen::SsidList | Screen::SsidScan | Screen::SsidScanning
        ) {
            self.screen = Screen::SsidScan;
        }
    }

    pub fn on_server_connected(&mut self) {
        if !matches!(
            self.screen,
            Screen::Menu
                | Screen::Extras
                | Screen::RosterList
                | Screen::FunctionList
                | Screen::DirectCommands
                | Screen::IpConfig
                | Screen::IpEdit
                | Screen::Device
                | Screen::DeviceNameEdit
                | Screen::DeviceIdEdit
                | Screen::Language
                | Screen::FirmwareUpdate
                | Screen::Diagnostics
                | Screen::WifiFailed
                | Screen::Splash
        ) {
            self.screen = Screen::Throttle;
        }
    }

    pub fn handle(
        &mut self,
        ev: InputEvent,
        domain: &DomainState,
        scanned: &heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }>,
        servers: &heapless::Vec<longfred_proto::mdns::WitServer, { sizes::MAX_FOUND_SERVERS }>,
    ) -> Intent {
        self.handle_input(ev, domain, scanned, servers)
    }

    pub(crate) fn begin_server_entry(&mut self, protocol: Protocol) {
        let keep_draft = self.manual_protocol == protocol
            && (!self.ip_kbd.buffer.is_empty() || !self.ip_digits.is_empty());
        self.manual_protocol = protocol;
        self.server_entry_from_list = false;
        self.screen = Screen::ServerEntry;
        if keep_draft {
            if self.ip_kbd.buffer.is_empty() {
                self.ip_kbd.load(self.ip_digits.as_str());
            }
            return;
        }
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
        self.ip_kbd.load(self.ip_digits.as_str());
    }

    pub(crate) fn begin_manual_server_from_list(&mut self) {
        self.begin_server_entry(Protocol::WiThrottle);
        self.server_entry_from_list = true;
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
        self.net_kbd.load(self.net_digits.as_str());
    }

    pub(crate) fn sync_digits_from_net_kbd(&mut self) {
        let _ = self.net_kbd.ok();
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

    pub fn format_net_display(&self) -> Line {
        let mut s = Line::new();
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
            crate::ui::view::push_oled(&mut s, self.net_kbd.preview().as_str());
            let d = self
                .net_kbd
                .pending()
                .or_else(|| self.net_kbd.buffer.chars().next())
                .unwrap_or(if self.net_cfg.dhcp { '0' } else { '1' });
            let _ = s.push_str(if d == '0' { " DHCP" } else { " Static" });
            return s;
        }
        if self.ip_field == 2 {
            crate::ui::view::push_oled(&mut s, self.net_kbd.preview().as_str());
            return s;
        }
        let ip = crate::ui::keyboard::format_grouped_ip(
            self.net_kbd.buffer.as_str(),
            self.net_kbd.cursor(),
            self.net_kbd.slot_char(),
            false,
        );
        crate::ui::view::push_oled(&mut s, ip.as_str());
        s
    }

    pub(crate) fn begin_device_name_edit(&mut self, domain: &DomainState) {
        self.text_kbd.mode = KeyboardMode::Text;
        self.text_kbd
            .set_max_len(longfred_proto::persist::MAX_DEVICE_NAME_LEN);
        self.text_kbd.load(domain.persist.device.name.as_str());
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
            let mut s = heapless::String::<4>::new();
            for d in digits {
                let _ = s.push(d as char);
            }
            self.id_kbd.load(s.as_str());
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
        if self.screen == Screen::Throttle && domain.current_slot_has_loco() {
            return Intent::Action(buttons::ENCODER_BUTTON_ACTION);
        }
        Intent::None
    }

    pub fn tick_text_field(&mut self, now_ms: u64) {
        match self.screen {
            Screen::Password | Screen::DeviceNameEdit => self.text_kbd.tick(now_ms),
            Screen::ServerEntry => self.ip_kbd.tick(now_ms),
            Screen::IpEdit => self.net_kbd.tick(now_ms),
            Screen::DeviceIdEdit => self.id_kbd.tick(now_ms),
            _ => {}
        }
    }

    pub fn pw_picker_char(&self) -> u8 {
        self.text_kbd
            .pending()
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
        } else if !self.selected_ssid.is_empty() {
            if !self.pw.is_empty() {
                (self.selected_ssid.as_str(), self.pw.as_str())
            } else if let Some(stored) = domain.persist.find_password(self.selected_ssid.as_str()) {
                (self.selected_ssid.as_str(), stored)
            } else {
                (self.selected_ssid.as_str(), "")
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

    pub fn password_preview(&self) -> Line {
        self.text_kbd.preview()
    }

    pub fn device_name_preview(&self) -> Line {
        self.text_kbd.preview()
    }

    pub fn format_device_id_display(&self) -> Line {
        self.id_kbd.preview()
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

    pub fn format_ip_display(&self) -> Line {
        crate::ui::keyboard::format_grouped_ip(
            self.ip_kbd.buffer.as_str(),
            self.ip_kbd.cursor(),
            self.ip_kbd.slot_char(),
            true,
        )
    }

    pub fn view(&self, ctx: &ViewCtx<'_>) -> UiView {
        match self.screen {
            Screen::Splash => UiView::Splash,
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
            let preview = self.addr_kbd.value_preview();
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
                ctx.battery.map(|s| s.percent)
            } else {
                None
            },
            battery_show_percent: self.battery_show_percent(),
        }
    }

    fn build_grid(&self, ctx: &ViewCtx<'_>) -> GridView {
        let mut g = GridView::new();
        g.foot_line = true;
        if matches!(self.screen, Screen::Password | Screen::DeviceNameEdit)
            && crate::board::active().has_keypad
        {
            g.caps = Some(self.text_kbd.uppercase());
        }
        match self.screen {
            Screen::Splash => {
                g.set(0, i18n::APP_NAME, false);
                g.set(1, i18n::FW_VERSION, false);
                g.set(5, i18n::tr().msg_booting, false);
            }
            Screen::SsidList => {
                let names = menu_nav::compiled_ssids();
                self.list
                    .draw(&mut g, Some(i18n::tr().msg_ssids_listed), &names, true);
            }
            Screen::SsidScan => {
                let mut names: heapless::Vec<&str, { sizes::MAX_FOUND_SSIDS }> =
                    heapless::Vec::new();
                for s in ctx.scanned_ssids {
                    let _ = names.push(s.ssid.as_str());
                }
                self.list
                    .draw(&mut g, Some(i18n::tr().msg_ssids_found), &names, true);
            }
            Screen::SsidScanning => {
                g.set(0, i18n::tr().msg_scanning_wifi, false);
                g.set(5, i18n::tr().hint_scanning_wifi, false);
            }
            Screen::Password => {
                g.set(0, i18n::tr().msg_enter_password, false);
                g.set(2, ctx.password_preview, false);
                g.set(5, i18n::tr().hint_enter_password, false);
            }
            Screen::ServerList => {
                let bufs = menu_nav::server_label_bufs(ctx.found_servers);
                let mut names: heapless::Vec<&str, { sizes::MAX_FOUND_SERVERS }> =
                    heapless::Vec::new();
                for b in &bufs {
                    let _ = names.push(b.as_str());
                }
                self.list
                    .draw(&mut g, Some(i18n::tr().msg_services_found), &names, true);
            }
            Screen::ServerProto => {
                g.set(0, i18n::tr().msg_select_proto, false);
                g.set(1, i18n::tr().proto_wit, self.list.cursor == 0);
                g.set(2, i18n::tr().proto_z21, self.list.cursor == 1);
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
            Screen::WifiFailed => {
                g.set(0, i18n::tr().msg_wifi_fail_1, false);
                g.set(1, i18n::tr().msg_wifi_fail_2, false);
                g.set(5, i18n::tr().hint_wifi_fail, false);
            }
            Screen::Menu => {
                let labels = menu_nav::menu_labels();
                self.list.draw(&mut g, None, &labels, true);
            }
            Screen::Extras => {
                g.set(0, i18n::tr().menu_extras, false);
                let labels = menu_nav::extras_labels();
                fill_list_page(&mut g, &labels, self.list.page, self.list.cursor, false);
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
                g.set(3, i18n::tr().device_name_id, self.list.cursor <= 1);
                g.set(4, i18n::tr().device_new_id, self.list.cursor == 2);
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
                g.set(0, i18n::tr().menu_locos, false);
                let names = menu_nav::roster_names(ctx.domain);
                fill_list_page(&mut g, &names, self.list.page, self.list.cursor, true);
            }
            Screen::FunctionList => {
                g.set(0, i18n::tr().menu_fn, false);
                let slot = ctx.domain.current_slot();
                let mut names: heapless::Vec<&str, { sizes::MAX_FUNCTIONS }> = heapless::Vec::new();
                for i in 0..sizes::MAX_FUNCTIONS {
                    let _ = names.push(slot.labels[i].as_str());
                }
                fill_list_page_invert(&mut g, &names, self.fn_page, true, |_local, global| {
                    slot.functions.get(global).copied().unwrap_or(false)
                });
            }
            Screen::DirectCommands => {
                let labels = menu_nav::direct_labels();
                fill_list_page(&mut g, &labels, self.list.page, self.list.cursor, false);
            }
            Screen::Language => {
                let labels = menu_nav::language_labels();
                self.list
                    .draw(&mut g, Some(i18n::tr().msg_language), &labels, false);
            }
            Screen::Diagnostics => draw_diagnostics(&mut g, self.list.page, ctx),
            _ => {}
        }
        if !self.list_num.is_empty() {
            g.set(0, self.list_num.as_str(), false);
        }
        if let Some(b) = ctx.broadcast {
            g.set(5, b, false);
        }
        g
    }
}

fn draw_diagnostics(g: &mut GridView, page: usize, ctx: &ViewCtx<'_>) {
    use crate::net::PingStatus;
    use core::fmt::Write;

    g.foot_line = false;
    let t = i18n::tr();
    let na = t.diag_na;
    let mut lines: heapless::Vec<Line, 8> = heapless::Vec::new();
    let title = match page {
        0 => t.diag_battery,
        1 => t.diag_version,
        2 => t.diag_software,
        3 => t.diag_range,
        4 => t.diag_wifi,
        _ => t.diag_ping,
    };
    g.set(0, title, false);

    match page {
        0 => {
            if let Some(b) = ctx.battery {
                let mut l = Line::new();
                let _ = write!(l, "{}%", b.percent);
                let _ = lines.push(l);
                let mut l = Line::new();
                let _ = write!(l, "{} mV", b.millivolts);
                let _ = lines.push(l);
                let mut l = Line::new();
                let _ = write!(l, "ADC {}", b.raw);
                let _ = lines.push(l);
            } else {
                let mut l = Line::new();
                let _ = l.push_str(na);
                let _ = lines.push(l);
            }
            let mut l = Line::new();
            let _ = write!(
                l,
                "factor {:.1}",
                crate::config::power::BATTERY_CONVERSION_FACTOR
            );
            let _ = lines.push(l);
            let mut l = Line::new();
            let _ = l.push_str("3.2-4.2 V");
            let _ = lines.push(l);
        }
        1 => {
            let mut l = Line::new();
            let _ = l.push_str(i18n::APP_NAME);
            let _ = lines.push(l);
            let mut l = Line::new();
            let _ = l.push_str(i18n::FW_VERSION);
            let _ = lines.push(l);
        }
        2 => {
            let board = crate::board::active();
            let mut l = Line::new();
            let _ = l.push_str(board.id);
            let _ = lines.push(l);
            let mut l = Line::new();
            let _ = l.push_str(board.mcu);
            let _ = lines.push(l);
            let proto = ctx
                .server
                .map(|s| s.protocol)
                .or_else(|| ctx.domain.persist.last_server.map(|s| s.protocol));
            let mut l = Line::new();
            match proto {
                Some(Protocol::WiThrottle) => {
                    let _ = l.push_str("WiThrottle");
                }
                Some(Protocol::Z21) => {
                    let _ = l.push_str("Z21");
                }
                None => {
                    let _ = l.push_str(na);
                }
            }
            let _ = lines.push(l);
        }
        3 => {
            if let Some(link) = ctx.wifi_link.as_ref() {
                let mut l = Line::new();
                let _ = write!(l, "RSSI {} dB", link.rssi);
                let _ = lines.push(l);
                let mut l = Line::new();
                let _ = write!(l, "ch {}", link.channel);
                let _ = lines.push(l);
            } else {
                let mut l = Line::new();
                let _ = l.push_str(na);
                let _ = lines.push(l);
            }
        }
        4 => {
            if let Some(link) = ctx.wifi_link.as_ref() {
                let mut l = Line::new();
                let _ = l.push_str(link.ssid.as_str());
                let _ = lines.push(l);
            } else {
                let mut l = Line::new();
                let _ = l.push_str(na);
                let _ = lines.push(l);
            }
            if let Some(net) = ctx.sta_net {
                let mut l = Line::new();
                write_ip_line(&mut l, net.ip);
                let _ = write!(l, "/{}", net.prefix);
                let _ = lines.push(l);
                let mut l = Line::new();
                if let Some(gw) = net.gateway {
                    write_ip_line(&mut l, gw);
                } else {
                    let _ = l.push_str(na);
                }
                let _ = lines.push(l);
                let mut l = Line::new();
                let _ = l.push_str("STA ");
                write_mac(&mut l, net.mac);
                let _ = lines.push(l);
            } else {
                let mut l = Line::new();
                let _ = l.push_str(na);
                let _ = lines.push(l);
            }
            let mut l = Line::new();
            let _ = l.push_str("AP ");
            if let Some(link) = ctx.wifi_link.as_ref() {
                write_mac(&mut l, link.bssid);
            } else {
                let _ = l.push_str(na);
            }
            let _ = lines.push(l);
        }
        _ => {
            let ep = ctx.server.or_else(|| {
                ctx.domain
                    .persist
                    .last_server
                    .map(|s| crate::net::ServerEndpoint {
                        ip: s.ip,
                        port: s.port,
                        protocol: s.protocol,
                    })
            });
            if let Some(ep) = ep {
                let mut l = Line::new();
                write_ip_line(&mut l, ep.ip);
                let _ = l.push(':');
                let _ = write!(l, "{}", ep.port);
                let _ = lines.push(l);
            } else {
                let mut l = Line::new();
                let _ = l.push_str(na);
                let _ = lines.push(l);
            }
            let mut l = Line::new();
            match ctx.ping {
                PingStatus::Ms(ms) => {
                    let _ = write!(l, "{} ms", ms);
                }
                PingStatus::Timeout => {
                    let _ = l.push_str(t.diag_timeout);
                }
                PingStatus::Idle => {
                    let _ = l.push_str(na);
                }
            }
            let _ = lines.push(l);
        }
    }

    let mut refs: heapless::Vec<&str, 8> = heapless::Vec::new();
    for line in &lines {
        let _ = refs.push(line.as_str());
    }
    fill_list_page(g, &refs, 0, usize::MAX, false);
}

fn write_mac(line: &mut Line, mac: [u8; 6]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (i, b) in mac.iter().enumerate() {
        if i > 0 {
            let _ = line.push(':');
        }
        let _ = line.push(HEX[(b >> 4) as usize] as char);
        let _ = line.push(HEX[(b & 0x0f) as usize] as char);
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
