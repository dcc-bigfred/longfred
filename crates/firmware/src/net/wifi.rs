//! WiFi STA: connection task (scan/connect via WIFI_CTRL), stack runner, DHCP/status.

use core::pin::pin;

use embassy_futures::select::{Either, Either3, select, select3};
use embassy_net::{ConfigV4, DhcpConfig, Ipv4Address, Ipv4Cidr, Runner, Stack, StaticConfigV4};
use embassy_time::{Duration, Timer};
use esp_radio::wifi::{
    AuthenticationMethod, Config as WifiConfig, Interface, PowerSaveMode, Protocol, Protocols,
    WifiController, WifiError, ap::AccessPointInfo, scan::ScanConfig, sta::StationConfig,
};
use log::{error, info, warn};

use crate::config;
use crate::config::sizes;
use crate::net::{
    NET_CONFIG_CTRL, NetStatus, STA_NET, STATE, SsidInfo, StaNet, WIFI_CTRL, WIFI_CTRL_DEPTH,
    WIFI_HOSTNAME, WIFI_LINK, WIFI_SCAN, WifiCmd, WifiLink,
};

/// embassy-net driver type provided by esp-radio (STA).
pub type NetDriver = Interface;

fn dhcp_config_with_hostname() -> ConfigV4 {
    let mut dhcp = DhcpConfig::default();
    if let Some(host) = WIFI_HOSTNAME.sender().try_get() {
        if !host.is_empty() {
            let mut hostname = heapless::String::<32>::new();
            let _ = hostname.push_str(host.as_str());
            dhcp.hostname = Some(hostname);
        }
    }
    ConfigV4::Dhcp(dhcp)
}

fn ap_to_ssid_info(ap: &AccessPointInfo) -> SsidInfo {
    let mut ssid = heapless::String::new();
    let _ = ssid.push_str(ap.ssid.as_str());
    SsidInfo {
        ssid,
        rssi: ap.signal_strength,
        open: ap.auth_method == Some(AuthenticationMethod::None),
    }
}

async fn publish_scan(controller: &mut WifiController<'static>) {
    info!("wifi scan requested");
    let result = scan(controller).await;
    let mut out: heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }> = heapless::Vec::new();
    match result {
        Ok(aps) => {
            for ap in &aps {
                let info = ap_to_ssid_info(ap);
                info!(
                    "wifi scan ssid='{}' bytes={} rssi={}",
                    info.ssid.as_str(),
                    info.ssid.len(),
                    info.rssi
                );
                if info.ssid.is_empty() {
                    continue;
                }
                if out.push(info).is_err() {
                    break;
                }
            }
        }
        Err(e) => warn!("wifi scan error: {:?}", e),
    }
    WIFI_SCAN.signal(out);
}

/// Wait for `connect_async` to finish so a later scan cannot hit IDF
/// `ESP_ERR_WIFI_STATE` (12294). `esp-radio` panics on that unmapped code.
///
/// Do not drop this future: `disconnect_async` is a no-op while
/// `is_connected()` is false, so a cancelled connect leaves STA connecting.
/// The settle timeout bounds how long we wait for a wedged radio so the
/// connection task can still service WIFI_CTRL (scan / connect commands).
async fn connect_sta(controller: &mut WifiController<'static>, timeout: Duration) -> bool {
    let settle = Duration::from_millis(config::network::WIFI_SETTLE_TIMEOUT_MS);
    let mut connect = pin!(controller.connect_async());
    match select(&mut connect, Timer::after(timeout)).await {
        Either::First(Ok(_)) => true,
        Either::First(Err(e)) => {
            warn!("wifi connect error: {:?}", e);
            false
        }
        Either::Second(()) => {
            warn!("wifi connect timeout; waiting for radio to settle");
            match select(&mut connect, Timer::after(settle)).await {
                Either::First(Ok(_)) => true,
                Either::First(Err(e)) => {
                    warn!("wifi connect error after timeout: {:?}", e);
                    false
                }
                Either::Second(()) => {
                    error!("wifi connect wedged after settle timeout; radio unresponsive");
                    false
                }
            }
        }
    }
}

async fn backoff_or_cmd(
    ctrl_rx: &embassy_sync::channel::Receiver<
        'static,
        embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
        WifiCmd,
        WIFI_CTRL_DEPTH,
    >,
    fails: u32,
) -> Option<WifiCmd> {
    let backoff =
        (config::network::RECONNECT_MIN_MS << fails.min(4)).min(config::network::RECONNECT_MAX_MS);
    match select(
        Timer::after(Duration::from_millis(backoff)),
        ctrl_rx.receive(),
    )
    .await
    {
        Either::First(()) => None,
        Either::Second(cmd) => Some(cmd),
    }
}

/// Task maintaining STA connection: handles WIFI_CTRL (scan/connect) and auto-rejoin.
#[embassy_executor::task]
pub async fn connection(mut controller: WifiController<'static>) {
    let ctrl_rx = WIFI_CTRL.receiver();
    let state_tx = STATE.sender();
    let mut pending = None;
    let mut rejoin_fails: u32 = 0;

    loop {
        let cmd = match pending.take() {
            Some(c) => c,
            None => ctrl_rx.receive().await,
        };
        match cmd {
            WifiCmd::Scan => publish_scan(&mut controller).await,
            WifiCmd::Connect {
                mut ssid,
                mut password,
            } => loop {
                let cfg = WifiConfig::Station(
                    StationConfig::default()
                        .with_ssid(ssid.as_str())
                        .with_password(password.as_str().into()),
                );
                state_tx.send(NetStatus::Connecting);
                if let Err(e) = controller.set_config(&cfg) {
                    warn!("wifi set_config error: {:?}", e);
                    state_tx.send(NetStatus::Disconnected);
                    match backoff_or_cmd(&ctrl_rx, rejoin_fails).await {
                        Some(next) => {
                            pending = Some(next);
                            break;
                        }
                        None => {
                            rejoin_fails = rejoin_fails.saturating_add(1);
                            continue;
                        }
                    }
                }
                if config::network::WIFI_FORCE_POWER_SAVE_NONE {
                    if let Err(e) = controller.set_power_saving(PowerSaveMode::None) {
                        warn!("wifi set_power_saving error: {:?}", e);
                    }
                }
                if config::network::WIFI_ENABLE_11AX {
                    let protocols = Protocols::default()
                        .with_2_4(Protocol::B | Protocol::G | Protocol::N | Protocol::AX);
                    if let Err(e) = controller.set_protocols(protocols) {
                        warn!("wifi set_protocols error: {:?}", e);
                    }
                }
                info!("wifi connecting to SSID={}", ssid.as_str());
                let timeout = Duration::from_millis(config::network::SSID_CONNECTION_TIMEOUT_MS);
                let associated = connect_sta(&mut controller, timeout).await;
                if !associated {
                    state_tx.send(NetStatus::Disconnected);
                }
                if let Ok(next) = ctrl_rx.try_receive() {
                    match next {
                        WifiCmd::Scan => {
                            pending = Some(WifiCmd::Scan);
                            break;
                        }
                        WifiCmd::Connect {
                            ssid: next_ssid,
                            password: next_password,
                        } => {
                            ssid = next_ssid;
                            password = next_password;
                            rejoin_fails = 0;
                            continue;
                        }
                    }
                }
                if !associated {
                    match backoff_or_cmd(&ctrl_rx, rejoin_fails).await {
                        Some(next) => {
                            pending = Some(next);
                            break;
                        }
                        None => {
                            rejoin_fails = rejoin_fails.saturating_add(1);
                            continue;
                        }
                    }
                }
                info!("wifi connected");
                rejoin_fails = 0;
                state_tx.send(NetStatus::WifiConnected);
                publish_wifi_link(&controller);
                let mut dropped = false;
                loop {
                    match select3(
                        controller.wait_for_disconnect_async(),
                        Timer::after(Duration::from_secs(1)),
                        ctrl_rx.receive(),
                    )
                    .await
                    {
                        Either3::First(_) => {
                            WIFI_LINK.sender().send(None);
                            warn!("wifi disconnected");
                            state_tx.send(NetStatus::Disconnected);
                            dropped = true;
                            break;
                        }
                        Either3::Second(_) => publish_wifi_link(&controller),
                        Either3::Third(WifiCmd::Scan) => {
                            publish_scan(&mut controller).await;
                        }
                        Either3::Third(next @ WifiCmd::Connect { .. }) => {
                            info!("wifi reconnect requested");
                            let _ = controller.disconnect_async().await;
                            WIFI_LINK.sender().send(None);
                            pending = Some(next);
                            break;
                        }
                    }
                }
                if pending.is_some() {
                    break;
                }
                if dropped {
                    match backoff_or_cmd(&ctrl_rx, rejoin_fails).await {
                        Some(next) => {
                            pending = Some(next);
                            break;
                        }
                        None => {
                            rejoin_fails = rejoin_fails.saturating_add(1);
                            continue;
                        }
                    }
                }
                break;
            },
        }
    }
}

/// embassy-net stack task (packet handling / DHCP).
#[embassy_executor::task]
pub async fn net_task(mut runner: Runner<'static, NetDriver>) -> ! {
    runner.run().await
}

/// Task waiting for link-up + IP; publishes NetStatus::Ready.
#[embassy_executor::task]
pub async fn status_task(stack: Stack<'static>) {
    let sender = STATE.sender();
    loop {
        stack.wait_link_up().await;
        stack.wait_config_up().await;
        if let Some(cfg) = stack.config_v4() {
            info!("net ready: ip={}", cfg.address);
            sender.send(NetStatus::Ready);
            let oct = cfg.address.address().octets();
            crate::net::STA_IPV4.sender().send(Some(oct));
            let mac = match stack.hardware_address() {
                embassy_net::HardwareAddress::Ethernet(addr) => addr.0,
            };
            STA_NET.sender().send(Some(StaNet {
                ip: oct,
                prefix: cfg.address.prefix_len(),
                gateway: cfg.gateway.map(|g| g.octets()),
                dns: cfg.dns_servers.first().map(|d| d.octets()),
                mac,
            }));
        }
        stack.wait_link_down().await;
        warn!("net link down");
        sender.send(NetStatus::Disconnected);
        crate::net::STA_IPV4.sender().send(None);
        STA_NET.sender().send(None);
        crate::net::HTTP_OTA_ENABLE.sender().send(false);
    }
}

/// Apply live IPv4 configuration from NET_CONFIG_CTRL.
#[embassy_executor::task]
pub async fn config_task(stack: Stack<'static>) {
    loop {
        let cfg = NET_CONFIG_CTRL.wait().await;
        let v4 = if cfg.dhcp {
            dhcp_config_with_hostname()
        } else {
            let mut static_cfg = StaticConfigV4 {
                address: Ipv4Cidr::new(
                    Ipv4Address::new(cfg.ip[0], cfg.ip[1], cfg.ip[2], cfg.ip[3]),
                    cfg.prefix_len,
                ),
                gateway: cfg
                    .gateway
                    .map(|g| Ipv4Address::new(g[0], g[1], g[2], g[3])),
                dns_servers: Default::default(),
            };
            if let Some(d) = cfg.dns {
                let _ = static_cfg
                    .dns_servers
                    .push(Ipv4Address::new(d[0], d[1], d[2], d[3]));
            }
            ConfigV4::Static(static_cfg)
        };
        stack.set_config_v4(v4);
        info!("net config applied: dhcp={}", cfg.dhcp);
    }
}

fn publish_wifi_link(controller: &WifiController<'_>) {
    let mut link = WifiLink {
        ssid: heapless::String::new(),
        rssi: 0,
        bssid: [0; 6],
        channel: 0,
    };
    if let Ok(ap) = controller.ap_info() {
        let _ = link.ssid.push_str(ap.ssid.as_str());
        link.bssid = ap.bssid;
        link.channel = ap.channel;
        link.rssi = ap.signal_strength;
    }
    if let Ok(rssi) = controller.rssi() {
        link.rssi = rssi.clamp(i8::MIN as i32, i8::MAX as i32) as i8;
    }
    if link.ssid.is_empty() && link.rssi == 0 && link.channel == 0 {
        WIFI_LINK.sender().send(None);
    } else {
        WIFI_LINK.sender().send(Some(link));
    }
}

/// Network scan (called from the connection task).
pub async fn scan(
    controller: &mut WifiController<'static>,
) -> Result<heapless::Vec<AccessPointInfo, { sizes::MAX_FOUND_SSIDS }>, WifiError> {
    let found = controller.scan_async(&ScanConfig::default()).await?;
    let mut out = heapless::Vec::new();
    for ap in found {
        if out.push(ap).is_err() {
            break;
        }
    }
    Ok(out)
}
