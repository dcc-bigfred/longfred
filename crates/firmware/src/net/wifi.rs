//! WiFi STA: task połączenia (autoconnect + reconnect), task stosu, skan.

use embassy_net::{Runner, Stack};
use embassy_time::{Duration, Timer};
use esp_radio::wifi::{
    ap::AccessPointInfo, scan::ScanConfig, sta::StationConfig, Config as WifiConfig, Interface,
    WifiController, WifiError,
};
use log::{info, warn};

use crate::config::{network, sizes};
use crate::net::{NetStatus, STATE};

/// Typ drivera embassy-net dostarczanego przez esp-radio (STA).
pub type NetDriver = Interface;

const RETRY_DELAY: Duration = Duration::from_secs(2);

/// Task utrzymujący połączenie STA: konfiguruje, łączy, reconnectuje.
#[embassy_executor::task]
pub async fn connection(mut controller: WifiController<'static>) {
    // Etap 4: pierwsza zdefiniowana sieć. Wybór z listy/UI -> Etap 9.
    let net = &network::NETWORKS[0];
    let cfg = WifiConfig::Station(
        StationConfig::default()
            .with_ssid(net.ssid)
            .with_password(net.password.into()),
    );

    let sender = STATE.sender();
    loop {
        sender.send(NetStatus::Connecting);
        if let Err(e) = controller.set_config(&cfg) {
            warn!("wifi set_config error: {:?}", e);
            Timer::after(RETRY_DELAY).await;
            continue;
        }
        info!("wifi connecting to SSID={}", net.ssid);
        match controller.connect_async().await {
            Ok(_) => {
                info!("wifi connected");
                sender.send(NetStatus::WifiConnected);
                controller.wait_for_disconnect_async().await.ok();
                warn!("wifi disconnected");
                sender.send(NetStatus::Disconnected);
            }
            Err(e) => {
                warn!("wifi connect error: {:?}", e);
                sender.send(NetStatus::Disconnected);
                Timer::after(RETRY_DELAY).await;
            }
        }
    }
}

/// Task stosu embassy-net (obsługa pakietów / DHCP).
#[embassy_executor::task]
pub async fn net_task(mut runner: Runner<'static, NetDriver>) -> ! {
    runner.run().await
}

/// Task oczekujący na IP z DHCP; aktualizuje status i loguje adres.
#[embassy_executor::task]
pub async fn status_task(stack: Stack<'static>) {
    let sender = STATE.sender();
    loop {
        stack.wait_config_up().await;
        if let Some(cfg) = stack.config_v4() {
            info!("net ready: ip={}", cfg.address);
            sender.send(NetStatus::Ready);
        }
        stack.wait_config_down().await;
        warn!("net config down");
    }
}

/// Skan sieci (pod przyszły picker SSID w Etapie 9).
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
