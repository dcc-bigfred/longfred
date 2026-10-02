//! Per-screen objects. Each [`ScreenId`] has a concrete type in this module.

mod addr_edit;
mod battery;
mod choice;
mod device;
mod device_id;
mod device_name;
mod diagnostics;
mod direct;
mod extras;
mod firmware;
mod functions;
pub(crate) mod helpers;
mod ip_edit;
mod language;
mod menu;
mod pairing;
mod pairing_wait;
mod radio_edit;
mod radio_settings;
mod roster;
mod server_confirm;
mod server_entry;
mod server_list;
mod server_menu;
mod server_proto;
mod slot_count;
mod splash;
mod throttle;
mod wifi_connecting;
mod wifi_failed;
mod wifi_list;
mod wifi_password;
mod wifi_scan;
mod wifi_scanning;
mod wifi_settings;

use crate::context::ScreenCtx;
use crate::intent::AppEvent;
use crate::nav::{Nav, PageDir, ScreenId, Step};
use crate::screen::{KeyBindings, Screen};
use crate::view::UiView;

pub use addr_edit::AddrEditScreen;
pub use battery::BatteryScreen;
pub use choice::ChoiceScreen;
pub use device::DeviceScreen;
pub use device_id::DeviceIdEditScreen;
pub use device_name::DeviceNameEditScreen;
pub use diagnostics::DiagnosticsScreen;
pub use direct::DirectCommandsScreen;
pub use extras::ExtrasScreen;
pub use firmware::FirmwareUpdateScreen;
pub use functions::FunctionListScreen;
pub use ip_edit::IpEditScreen;
pub use language::LanguageScreen;
pub use menu::MenuScreen;
pub use pairing::PairingScreen;
pub use pairing_wait::PairingWaitScreen;
pub use radio_edit::RadioEditScreen;
pub use radio_settings::RadioSettingsScreen;
pub use roster::RosterListScreen;
pub use server_confirm::ServerConfirmScreen;
pub use server_entry::ServerEntryScreen;
pub use server_list::ServerListScreen;
pub use server_menu::ServerMenuScreen;
pub use server_proto::ServerProtoScreen;
pub use slot_count::SlotCountEditScreen;
pub use splash::SplashScreen;
pub use throttle::ThrottleScreen;
pub use wifi_connecting::ConnectingScreen;
pub use wifi_failed::WifiFailedScreen;
pub use wifi_list::SsidListScreen;
pub use wifi_password::PasswordScreen;
pub use wifi_scan::SsidScanScreen;
pub use wifi_scanning::SsidScanningScreen;
pub use wifi_settings::WifiSettingsScreen;

/// Active screen object (size = max variant, not the sum of all screens).
#[expect(missing_docs, reason = "variants match ScreenId one-to-one")]
pub enum ScreenState {
    Splash(SplashScreen),
    SsidList(SsidListScreen),
    SsidScan(SsidScanScreen),
    SsidScanning(SsidScanningScreen),
    Password(PasswordScreen),
    ServerList(ServerListScreen),
    ServerConfirm(ServerConfirmScreen),
    ServerProto(ServerProtoScreen),
    ServerEntry(ServerEntryScreen),
    Connecting(ConnectingScreen),
    Throttle(ThrottleScreen),
    Menu(MenuScreen),
    Extras(ExtrasScreen),
    ServerMenu(ServerMenuScreen),
    WifiSettings(WifiSettingsScreen),
    Choice(ChoiceScreen),
    RosterList(RosterListScreen),
    AddrEdit(AddrEditScreen),
    Pairing(PairingScreen),
    PairingWait(PairingWaitScreen),
    FunctionList(FunctionListScreen),
    DirectCommands(DirectCommandsScreen),
    IpEdit(IpEditScreen),
    Device(DeviceScreen),
    DeviceNameEdit(DeviceNameEditScreen),
    DeviceIdEdit(DeviceIdEditScreen),
    SlotCountEdit(SlotCountEditScreen),
    Language(LanguageScreen),
    FirmwareUpdate(FirmwareUpdateScreen),
    WifiFailed(WifiFailedScreen),
    Diagnostics(DiagnosticsScreen),
    Battery(BatteryScreen),
    RadioSettings(RadioSettingsScreen),
    RadioEdit(RadioEditScreen),
}

/// Construct the concrete screen object for `id` (fresh local state).
#[must_use]
pub fn new_screen(id: ScreenId) -> ScreenState {
    match id {
        ScreenId::Splash => ScreenState::Splash(SplashScreen::new()),
        ScreenId::SsidList => ScreenState::SsidList(SsidListScreen::new()),
        ScreenId::SsidScan => ScreenState::SsidScan(SsidScanScreen::new()),
        ScreenId::SsidScanning => ScreenState::SsidScanning(SsidScanningScreen),
        ScreenId::Password => ScreenState::Password(PasswordScreen::new()),
        ScreenId::ServerList => ScreenState::ServerList(ServerListScreen::new()),
        ScreenId::ServerConfirm => ScreenState::ServerConfirm(ServerConfirmScreen),
        ScreenId::ServerProto => ScreenState::ServerProto(ServerProtoScreen::new()),
        ScreenId::ServerEntry => ScreenState::ServerEntry(ServerEntryScreen::new()),
        ScreenId::Connecting => ScreenState::Connecting(ConnectingScreen),
        ScreenId::Throttle => ScreenState::Throttle(ThrottleScreen::new()),
        ScreenId::Menu => ScreenState::Menu(MenuScreen::new()),
        ScreenId::Extras => ScreenState::Extras(ExtrasScreen::new()),
        ScreenId::ServerMenu => ScreenState::ServerMenu(ServerMenuScreen::new()),
        ScreenId::WifiSettings => ScreenState::WifiSettings(WifiSettingsScreen::new()),
        ScreenId::Choice => ScreenState::Choice(ChoiceScreen::new()),
        ScreenId::RosterList => ScreenState::RosterList(RosterListScreen::new()),
        ScreenId::AddrEdit => ScreenState::AddrEdit(AddrEditScreen::new()),
        ScreenId::Pairing => ScreenState::Pairing(PairingScreen::new()),
        ScreenId::PairingWait => ScreenState::PairingWait(PairingWaitScreen),
        ScreenId::FunctionList => ScreenState::FunctionList(FunctionListScreen::new()),
        ScreenId::DirectCommands => ScreenState::DirectCommands(DirectCommandsScreen::new()),
        ScreenId::IpEdit => ScreenState::IpEdit(IpEditScreen::new()),
        ScreenId::Device => ScreenState::Device(DeviceScreen::new()),
        ScreenId::DeviceNameEdit => ScreenState::DeviceNameEdit(DeviceNameEditScreen::new()),
        ScreenId::DeviceIdEdit => ScreenState::DeviceIdEdit(DeviceIdEditScreen::new()),
        ScreenId::SlotCountEdit => ScreenState::SlotCountEdit(SlotCountEditScreen::new()),
        ScreenId::Language => ScreenState::Language(LanguageScreen::new()),
        ScreenId::FirmwareUpdate => ScreenState::FirmwareUpdate(FirmwareUpdateScreen),
        ScreenId::WifiFailed => ScreenState::WifiFailed(WifiFailedScreen),
        ScreenId::Diagnostics => ScreenState::Diagnostics(DiagnosticsScreen::new()),
        ScreenId::Battery => ScreenState::Battery(BatteryScreen::new()),
        ScreenId::RadioSettings => ScreenState::RadioSettings(RadioSettingsScreen::new()),
        ScreenId::RadioEdit => ScreenState::RadioEdit(RadioEditScreen::new()),
    }
}

/// Forward a [`Screen`] method to the live enum variant.
macro_rules! dispatch_screen {
    ($self:expr, $method:ident $(, $arg:expr)* $(,)?) => {
        match $self {
            ScreenState::Splash(s) => s.$method($($arg),*),
            ScreenState::SsidList(s) => s.$method($($arg),*),
            ScreenState::SsidScan(s) => s.$method($($arg),*),
            ScreenState::SsidScanning(s) => s.$method($($arg),*),
            ScreenState::Password(s) => s.$method($($arg),*),
            ScreenState::ServerList(s) => s.$method($($arg),*),
            ScreenState::ServerConfirm(s) => s.$method($($arg),*),
            ScreenState::ServerProto(s) => s.$method($($arg),*),
            ScreenState::ServerEntry(s) => s.$method($($arg),*),
            ScreenState::Connecting(s) => s.$method($($arg),*),
            ScreenState::Throttle(s) => s.$method($($arg),*),
            ScreenState::Menu(s) => s.$method($($arg),*),
            ScreenState::Extras(s) => s.$method($($arg),*),
            ScreenState::ServerMenu(s) => s.$method($($arg),*),
            ScreenState::WifiSettings(s) => s.$method($($arg),*),
            ScreenState::Choice(s) => s.$method($($arg),*),
            ScreenState::RosterList(s) => s.$method($($arg),*),
            ScreenState::AddrEdit(s) => s.$method($($arg),*),
            ScreenState::Pairing(s) => s.$method($($arg),*),
            ScreenState::PairingWait(s) => s.$method($($arg),*),
            ScreenState::FunctionList(s) => s.$method($($arg),*),
            ScreenState::DirectCommands(s) => s.$method($($arg),*),
            ScreenState::IpEdit(s) => s.$method($($arg),*),
            ScreenState::Device(s) => s.$method($($arg),*),
            ScreenState::DeviceNameEdit(s) => s.$method($($arg),*),
            ScreenState::DeviceIdEdit(s) => s.$method($($arg),*),
            ScreenState::SlotCountEdit(s) => s.$method($($arg),*),
            ScreenState::Language(s) => s.$method($($arg),*),
            ScreenState::FirmwareUpdate(s) => s.$method($($arg),*),
            ScreenState::WifiFailed(s) => s.$method($($arg),*),
            ScreenState::Diagnostics(s) => s.$method($($arg),*),
            ScreenState::Battery(s) => s.$method($($arg),*),
            ScreenState::RadioSettings(s) => s.$method($($arg),*),
            ScreenState::RadioEdit(s) => s.$method($($arg),*),
        }
    };
}

impl Screen for ScreenState {
    fn id(&self) -> ScreenId {
        dispatch_screen!(self, id)
    }

    /// Key map of the live variant.
    fn key_bindings(&self, cx: &ScreenCtx<'_>) -> KeyBindings {
        dispatch_screen!(self, key_bindings, cx)
    }

    /// Render the live variant.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        dispatch_screen!(self, view, cx)
    }

    fn on_enter(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        dispatch_screen!(self, on_enter, cx, nav);
    }

    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        dispatch_screen!(self, on_select, cx, nav);
    }

    fn on_cancel(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        dispatch_screen!(self, on_cancel, cx, nav);
    }

    fn on_stop(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        dispatch_screen!(self, on_stop, cx, nav);
    }

    fn on_star(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        dispatch_screen!(self, on_star, cx, nav);
    }

    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        dispatch_screen!(self, on_digit, c, cx, nav);
    }

    fn on_list_step(&mut self, d: Step, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        dispatch_screen!(self, on_list_step, d, cx, nav);
    }

    fn on_page(&mut self, d: PageDir, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        dispatch_screen!(self, on_page, d, cx, nav);
    }

    fn on_char_cycle(&mut self, d: i8, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        dispatch_screen!(self, on_char_cycle, d, cx, nav);
    }

    fn on_cursor_move(&mut self, d: i8, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        dispatch_screen!(self, on_cursor_move, d, cx, nav);
    }

    fn on_case_toggle(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        dispatch_screen!(self, on_case_toggle, cx, nav);
    }

    fn on_menu_key(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        dispatch_screen!(self, on_menu_key, cx, nav);
    }

    fn on_fn_key(&mut self, k: u8, down: bool, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        dispatch_screen!(self, on_fn_key, k, down, cx, nav);
    }

    fn on_tick(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        dispatch_screen!(self, on_tick, cx, nav);
    }

    fn on_app_event(&mut self, e: AppEvent, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        dispatch_screen!(self, on_app_event, e, cx, nav);
    }
}
