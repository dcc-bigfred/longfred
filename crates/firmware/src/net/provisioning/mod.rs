//! Soft-AP programming / pairing mode (HTTP provisioning).

mod http_server;
pub mod ota;

use embassy_futures::select::{Either, select};
use embassy_net::{
    Config as NetConfig, Ipv4Address, Ipv4Cidr, Stack, StackResources, StaticConfigV4,
};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use esp_hal::efuse::{self, InterfaceMacAddress};
use esp_hal::system::software_reset;
use esp_radio::wifi::{
    Config as WifiConfig, ControllerConfig, Interface, WifiController, ap::AccessPointConfig,
};
use heapless::String;
use log::{error, info, warn};
use longfred_proto::persist::PersistRecord;
use static_cell::StaticCell;

use crate::board;
use crate::config::{self, sizes};
use crate::input::{INPUT_CHANNEL, InputEvent, NavDir};
use crate::storage::{PERSIST_LOADED, STORAGE_ACK, STORAGE_CTRL, SharedFlash, StorageCmd};
use crate::ui::UI_VIEW;
use crate::ui::i18n;
use crate::ui::view::{GridView, UiView};

const AP_IP: Ipv4Address = Ipv4Address::new(
    config::network::AP_IP[0],
    config::network::AP_IP[1],
    config::network::AP_IP[2],
    config::network::AP_IP[3],
);
const AP_PREFIX: u8 = config::network::AP_PREFIX;
const SSID_PREFIX: &str = "longfred_prog_";

static PROG_REC: StaticCell<Mutex<CriticalSectionRawMutex, PersistRecord>> = StaticCell::new();
static STA_REC: StaticCell<Mutex<CriticalSectionRawMutex, PersistRecord>> = StaticCell::new();

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
        gateway: Some(AP_IP),
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
    let ctrl_cfg = ControllerConfig::default().with_initial_config(WifiConfig::AccessPoint(ap_cfg));

    match WifiController::new(wifi, ctrl_cfg) {
        Ok(controller) => {
            let iface = Interface::access_point();
            info!(
                "programming: Soft-AP started, static IP {}/{}",
                AP_IP, AP_PREFIX
            );
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum PairingPage {
    Ask,
    Url,
}

fn ask_grid(ssid: &str) -> GridView {
    let mut grid = GridView::new();
    grid.set(0, ssid, false);
    grid.set(2, i18n::tr().msg_prog_connected, false);
    grid.set(5, i18n::tr().hint_prog_ask, false);
    grid
}

fn url_grid(ssid: &str) -> GridView {
    let mut grid = GridView::new();
    grid.set(0, ssid, false);
    grid.set(2, config::network::PAIRING_HTTP_URL, false);
    grid.set(5, i18n::tr().hint_prog_back, false);
    grid
}

fn publish_pairing_page(page: PairingPage, ssid: &str, qr_ok: bool) {
    let view = match page {
        PairingPage::Ask => UiView::Grid(ask_grid(ssid)),
        PairingPage::Url if qr_ok => UiView::PairingQr,
        PairingPage::Url => UiView::Grid(url_grid(ssid)),
    };
    UI_VIEW.sender().send(view);
}

/// OLED wizard + LED pairing; Stop / left-on-ask exits Soft-AP.
#[embassy_executor::task]
pub async fn pairing_ui_task(ssid: String<32>) {
    let desc = board::active_variant();
    let has_display = desc.display.is_some();
    let qr_ok = desc.display.is_some_and(|d| d.height > 32);
    let mut page = PairingPage::Ask;

    if has_display {
        publish_pairing_page(page, ssid.as_str(), qr_ok);
        info!("programming: display shows pairing wizard");
    }
    #[cfg(feature = "variant-heiko-wifred")]
    {
        crate::ui::led_presenter::LED_MODE
            .sender()
            .send(crate::ui::led_presenter::LedMode::Pairing);
        info!("programming: LED pairing pattern");
    }
    #[cfg(not(feature = "variant-heiko-wifred"))]
    if !has_display {
        info!("programming: pairing active (no display/LEDs)");
    }

    let rx = INPUT_CHANNEL.receiver();
    let mut was_busy = false;
    loop {
        match select(rx.receive(), Timer::after(Duration::from_millis(250))).await {
            Either::First(ev) => match ev {
                InputEvent::Stop | InputEvent::EStop => {
                    info!("programming: cancel via Stop/EStop");
                    exit_programming_mode(50).await;
                }
                InputEvent::Nav(NavDir::Left) => match page {
                    PairingPage::Ask => {
                        info!("programming: cancel via left");
                        exit_programming_mode(50).await;
                    }
                    PairingPage::Url => {
                        page = PairingPage::Ask;
                        if has_display {
                            publish_pairing_page(page, ssid.as_str(), qr_ok);
                        }
                    }
                },
                InputEvent::Nav(NavDir::Right) if page == PairingPage::Ask => {
                    page = PairingPage::Url;
                    if has_display {
                        publish_pairing_page(page, ssid.as_str(), qr_ok);
                    }
                }
                _ => {}
            },
            Either::Second(()) => {
                let busy = crate::net::http_ota_busy();
                if was_busy && !busy && has_display {
                    publish_pairing_page(page, ssid.as_str(), qr_ok);
                }
                was_busy = busy;
            }
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
    flash: &'static SharedFlash,
) -> bool {
    let mac = efuse::interface_mac_address(InterfaceMacAddress::AccessPoint);
    let mut mac_bytes = [0u8; 6];
    mac_bytes.copy_from_slice(mac.as_bytes());
    let ssid = ap_ssid_from_mac(&mac_bytes);
    i18n::set_language(initial.language);

    let Some((controller, iface)) = start_ap(wifi) else {
        error!("programming: no Soft-AP interface");
        return false;
    };

    static RESOURCES: StaticCell<StackResources<{ sizes::PROG_NET_SOCKETS }>> = StaticCell::new();
    let resources = RESOURCES.init(StackResources::new());
    let (stack, runner) = embassy_net::new(iface, static_ap_config(), resources, seed);

    let rec = PROG_REC.init(Mutex::new(initial));

    crate::spawn_or_reset!(spawner, ap_hold_task(controller), "ap-hold");
    crate::spawn_or_reset!(spawner, crate::net::wifi::net_task(runner), "prog-net");
    crate::spawn_or_reset!(
        spawner,
        http_server::task_ap(stack, rec, flash),
        "prog-http"
    );
    crate::spawn_or_reset!(spawner, dhcp_task(stack), "prog-dhcp");
    crate::spawn_or_reset!(spawner, pairing_ui_task(ssid), "prog-ui");
    crate::spawn_or_warn!(spawner, sync_persist_task(rec), "prog-persist");

    true
}

#[embassy_executor::task]
async fn sync_persist_task(rec: &'static Mutex<CriticalSectionRawMutex, PersistRecord>) {
    let Some(mut rx) = PERSIST_LOADED.receiver() else {
        warn!("programming: persist watch has no free receiver");
        return;
    };
    loop {
        let updated = rx.changed().await;
        let mut guard = rec.lock().await;
        *guard = updated;
    }
}

/// Clear programming flag, ack storage, then reboot after `delay_ms`.
pub async fn exit_programming_mode(delay_ms: u64) -> ! {
    let tx = STORAGE_CTRL.sender();
    let _ = tx.try_send(StorageCmd::SetProgrammingMode(false));
    if !STORAGE_ACK.wait().await {
        warn!("programming: persist failed — resetting anyway");
    }
    Timer::after(Duration::from_millis(delay_ms)).await;
    software_reset();
}

/// Used by HTTP server / tests: re-export stack type.
pub type ProgStack = Stack<'static>;

/// STA HTTP OTA + mDNS announce (gated by [`crate::net::HTTP_OTA_ENABLE`]).
pub fn spawn_sta_http(
    spawner: &embassy_executor::Spawner,
    stack: Stack<'static>,
    initial: PersistRecord,
    flash: &'static SharedFlash,
) {
    let rec = STA_REC.init(Mutex::new(initial));
    crate::spawn_or_reset!(
        spawner,
        http_server::task_sta(stack, rec, flash),
        "sta-http"
    );
    crate::spawn_or_warn!(spawner, sync_persist_task(rec), "sta-persist");
    crate::spawn_or_warn!(
        spawner,
        crate::net::mdns::ota_announce_task(stack),
        "ota-mdns"
    );
}

#[embassy_executor::task]
async fn dhcp_task(stack: Stack<'static>) {
    use esp_hal_dhcp_server::simple_leaser::SimpleDhcpLeaser;
    use esp_hal_dhcp_server::structs::DhcpServerConfig;
    use esp_hal_dhcp_server::{Ipv4Addr, run_dhcp_server};

    let ip = Ipv4Addr::new(
        config::network::AP_IP[0],
        config::network::AP_IP[1],
        config::network::AP_IP[2],
        config::network::AP_IP[3],
    );
    let gw = [ip];
    let dns = [ip];
    let dhcp_config = DhcpServerConfig {
        ip,
        lease_time: Duration::from_secs(3600),
        gateways: &gw,
        subnet: Some(Ipv4Addr::new(255, 255, 255, 0)),
        dns: &dns,
        use_captive_portal: false,
    };
    let mut leaser = SimpleDhcpLeaser {
        start: Ipv4Addr::new(
            config::network::AP_DHCP_START[0],
            config::network::AP_DHCP_START[1],
            config::network::AP_DHCP_START[2],
            config::network::AP_DHCP_START[3],
        ),
        end: Ipv4Addr::new(
            config::network::AP_DHCP_END[0],
            config::network::AP_DHCP_END[1],
            config::network::AP_DHCP_END[2],
            config::network::AP_DHCP_END[3],
        ),
        leases: Default::default(),
    };
    info!(
        "programming: DHCP pool {}.{}.{}.{}-{}.{}.{}.{}",
        config::network::AP_DHCP_START[0],
        config::network::AP_DHCP_START[1],
        config::network::AP_DHCP_START[2],
        config::network::AP_DHCP_START[3],
        config::network::AP_DHCP_END[0],
        config::network::AP_DHCP_END[1],
        config::network::AP_DHCP_END[2],
        config::network::AP_DHCP_END[3],
    );
    if let Err(e) = run_dhcp_server(stack, dhcp_config, &mut leaser).await {
        warn!("programming: DHCP server failed: {:?}", e);
        loop {
            Timer::after(Duration::from_secs(60)).await;
        }
    }
}
