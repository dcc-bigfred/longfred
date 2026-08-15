//! WiFi STA: connection task (scan/connect via WIFI_CTRL), stack runner, DHCP/status.

use embassy_net::{ConfigV4, DhcpConfig, Ipv4Address, Ipv4Cidr, Runner, Stack, StaticConfigV4};
use embassy_time::{Duration, Timer};
use esp_radio::wifi::{
    AuthenticationMethod, Config as WifiConfig, Interface, PowerSaveMode, Protocol, Protocols,
    WifiController, WifiError, ap::AccessPointInfo, scan::ScanConfig, sta::StationConfig,
};
use log::{info, warn};

use crate::config;
use crate::config::sizes;
use crate::net::{
    NET_CONFIG_CTRL, NetStatus, STATE, SsidInfo, WIFI_CTRL, WIFI_HOSTNAME, WIFI_SCAN, WifiCmd,
};

/// embassy-net driver type provided by esp-radio (STA).
pub type NetDriver = Interface;

const RETRY_DELAY: Duration = Duration::from_secs(2);

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

/// Task maintaining STA connection: handles WIFI_CTRL (scan/connect).
#[embassy_executor::task]
pub async fn connection(mut controller: WifiController<'static>) {
    let ctrl_rx = WIFI_CTRL.receiver();
    let state_tx = STATE.sender();

    loop {
        let cmd = ctrl_rx.receive().await;
        match cmd {
            WifiCmd::Scan => {
                info!("wifi scan requested");
                let result = scan(&mut controller).await;
                let mut out: heapless::Vec<SsidInfo, { sizes::MAX_FOUND_SSIDS }> =
                    heapless::Vec::new();
                match result {
                    Ok(aps) => {
                        for ap in &aps {
                            if out.push(ap_to_ssid_info(ap)).is_err() {
                                break;
                            }
                        }
                    }
                    Err(e) => warn!("wifi scan error: {:?}", e),
                }
                WIFI_SCAN.signal(out);
            }
            WifiCmd::Connect { ssid, password } => {
                let cfg = WifiConfig::Station(
                    StationConfig::default()
                        .with_ssid(ssid.as_str())
                        .with_password(password.as_str().into()),
                );
                loop {
                    state_tx.send(NetStatus::Connecting);
                    if let Err(e) = controller.set_config(&cfg) {
                        warn!("wifi set_config error: {:?}", e);
                        Timer::after(RETRY_DELAY).await;
                        continue;
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
                    match controller.connect_async().await {
                        Ok(_) => {
                            info!("wifi connected");
                            state_tx.send(NetStatus::WifiConnected);
                            controller.wait_for_disconnect_async().await.ok();
                            warn!("wifi disconnected");
                            state_tx.send(NetStatus::Disconnected);
                            break;
                        }
                        Err(e) => {
                            warn!("wifi connect error: {:?}", e);
                            state_tx.send(NetStatus::Disconnected);
                            Timer::after(RETRY_DELAY).await;
                        }
                    }
                }
            }
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
        }
        stack.wait_link_down().await;
        warn!("net link down");
        crate::net::STA_IPV4.sender().send(None);
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
