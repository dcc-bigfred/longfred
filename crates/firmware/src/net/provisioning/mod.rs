//! Soft-AP programming / pairing mode (HTTP provisioning).

mod http_server;

use embassy_net::{Config as NetConfig, Ipv4Address, Ipv4Cidr, Stack, StackResources, StaticConfigV4};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use esp_hal::efuse::{self, InterfaceMacAddress};
use esp_hal::system::software_reset;
use esp_radio::wifi::{
    ap::AccessPointConfig, Config as WifiConfig, ControllerConfig, Interface, WifiController,
};
use heapless::String;
use log::{info, warn};
use longfred_proto::persist::PersistRecord;
use static_cell::StaticCell;

use crate::board;
use crate::config::sizes;
use crate::input::{InputEvent, INPUT_CHANNEL};
use crate::storage::{StorageCmd, PERSIST_LOADED, STORAGE_ACK, STORAGE_CTRL};
use crate::ui::view::{GridView, UiView};
use crate::ui::UI_VIEW;

const AP_IP: Ipv4Address = Ipv4Address::new(192, 168, 0, 1);
const AP_PREFIX: u8 = 24;
const SSID_PREFIX: &str = "longfred_prog_";

static PROG_REC: StaticCell<Mutex<CriticalSectionRawMutex, PersistRecord>> = StaticCell::new();

/// Build Soft-AP SSID `longfred_prog_XXXXXX` from the last 3 MAC octets (hex).
pub fn ap_ssid_from_mac(mac: &[u8; 6]) -> String<32> {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut s = String::<32>::new();
    let _ = s.push_str(SSID_PREFIX);
    for &b in &mac[3..6] {
        let _ = s.push(HEX[(b >> 4) as usize] as char);
        let _ = s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

fn static_ap_config() -> NetConfig {
    NetConfig::ipv4_static(StaticConfigV4 {
        address: Ipv4Cidr::new(AP_IP, AP_PREFIX),
        gateway: None,
        dns_servers: Default::default(),
    })
}

/// Configure Soft-AP and return (controller, embassy-net interface).
///
/// Prefer calling with a fresh `WifiController`; on failure returns `None` after logging.
pub fn start_ap(
    wifi: esp_hal::peripherals::WIFI<'static>,
) -> Option<(WifiController<'static>, Interface)> {
    let mac = efuse::interface_mac_address(InterfaceMacAddress::AccessPoint);
    let mut mac_bytes = [0u8; 6];
    mac_bytes.copy_from_slice(mac.as_bytes());
    let ssid = ap_ssid_from_mac(&mac_bytes);
    info!("programming: Soft-AP SSID={}", ssid.as_str());

    let ap_cfg = AccessPointConfig::default().with_ssid(ssid.as_str());
    let ctrl_cfg =
        ControllerConfig::default().with_initial_config(WifiConfig::AccessPoint(ap_cfg));

    match WifiController::new(wifi, ctrl_cfg) {
        Ok(controller) => {
            let iface = Interface::access_point();
            info!("programming: Soft-AP started, static IP {}/{}", AP_IP, AP_PREFIX);
            Some((controller, iface))
        }
        Err(e) => {
            warn!("programming: Soft-AP start failed: {:?} — stub mode", e);
            None
        }
    }
}

/// Hold the Wi-Fi controller so Soft-AP stays up.
#[embassy_executor::task]
pub async fn ap_hold_task(controller: WifiController<'static>) {
    let _controller = controller;
    loop {
        Timer::after(Duration::from_secs(60)).await;
    }
}

/// OLED / LED pairing indication.
#[embassy_executor::task]
pub async fn pairing_ui_task(ssid: String<32>) {
    let desc = board::active_variant();
    if desc.display.is_some() {
        let mut grid = GridView::new();
        grid.set(0, "Pairing mode", false);
        grid.set(1, ssid.as_str(), false);
        grid.set(2, "192.168.0.1", false);
        UI_VIEW.sender().send(UiView::Grid(grid));
        info!("programming: display shows Pairing mode");
    }
    #[cfg(feature = "variant-heiko-wifred")]
    {
        crate::ui::led_presenter::LED_MODE
            .sender()
            .send(crate::ui::led_presenter::LedMode::Pairing);
        info!("programming: LED pairing pattern");
    }
    #[cfg(not(feature = "variant-heiko-wifred"))]
    if desc.display.is_none() {
        info!("programming: pairing active (no display/LEDs)");
    }
    loop {
        Timer::after(Duration::from_secs(30)).await;
    }
}

/// Stop / E-Stop clears programming mode and reboots.
#[embassy_executor::task]
pub async fn cancel_task() {
    let rx = INPUT_CHANNEL.receiver();
    let tx = STORAGE_CTRL.sender();
    loop {
        match rx.receive().await {
            InputEvent::Stop | InputEvent::EStop => {
                info!("programming: cancel via Stop/EStop");
                let _ = tx.try_send(StorageCmd::SetProgrammingMode(false));
                STORAGE_ACK.wait().await;
                Timer::after(Duration::from_millis(50)).await;
                software_reset();
            }
            _ => {}
        }
    }
}

/// Spawn Soft-AP stack + HTTP server. Returns `false` if AP could not be created
/// and no interface is available (caller should fall back or hang).
pub fn spawn_programming_net(
    spawner: &embassy_executor::Spawner,
    wifi: esp_hal::peripherals::WIFI<'static>,
    seed: u64,
    initial: PersistRecord,
) -> bool {
    let mac = efuse::interface_mac_address(InterfaceMacAddress::AccessPoint);
    let mut mac_bytes = [0u8; 6];
    mac_bytes.copy_from_slice(mac.as_bytes());
    let ssid = ap_ssid_from_mac(&mac_bytes);

    let Some((controller, iface)) = start_ap(wifi) else {
        warn!("programming: no Soft-AP interface; HTTP not started");
        if let Ok(token) = pairing_ui_task(ssid) {
            spawner.spawn(token);
        }
        if let Ok(token) = cancel_task() {
            spawner.spawn(token);
        }
        return false;
    };

    static RESOURCES: StaticCell<StackResources<{ sizes::NET_SOCKETS }>> = StaticCell::new();
    let resources = RESOURCES.init(StackResources::new());
    let (stack, runner) = embassy_net::new(iface, static_ap_config(), resources, seed);

    let rec = PROG_REC.init(Mutex::new(initial));

    if let Ok(token) = ap_hold_task(controller) {
        spawner.spawn(token);
    }
    if let Ok(token) = crate::net::wifi::net_task(runner) {
        spawner.spawn(token);
    }
    if let Ok(token) = http_server::task(stack, rec) {
        spawner.spawn(token);
    }
    if let Ok(token) = pairing_ui_task(ssid) {
        spawner.spawn(token);
    }
    if let Ok(token) = cancel_task() {
        spawner.spawn(token);
    }

    // Refresh local record if storage republishes.
    if let Ok(token) = sync_persist_task(rec) {
        spawner.spawn(token);
    }

    true
}

#[embassy_executor::task]
async fn sync_persist_task(rec: &'static Mutex<CriticalSectionRawMutex, PersistRecord>) {
    loop {
        let updated = PERSIST_LOADED.wait().await;
        let mut guard = rec.lock().await;
        *guard = updated;
    }
}

/// Clear programming flag, ack storage, then reboot after `delay_ms`.
pub async fn exit_programming_mode(delay_ms: u64) -> ! {
    let tx = STORAGE_CTRL.sender();
    let _ = tx.try_send(StorageCmd::SetProgrammingMode(false));
    STORAGE_ACK.wait().await;
    Timer::after(Duration::from_millis(delay_ms)).await;
    software_reset();
}

/// Used by HTTP server / tests: re-export stack type.
pub type ProgStack = Stack<'static>;
