//! WiFi STA: connection task (scan/connect via WIFI_CTRL), stack runner, DHCP/status.

use core::net::Ipv4Addr;
use core::pin::pin;

use embassy_futures::select::{Either, Either3, select, select3};
use embassy_net::icmp::PacketMetadata;
use embassy_net::icmp::ping::{PingManager, PingParams};
use embassy_net::{ConfigV4, DhcpConfig, Ipv4Address, Ipv4Cidr, Runner, Stack, StaticConfigV4};
use embassy_time::{Duration, Instant, Timer};
use esp_radio::wifi::{
    AuthenticationMethod, Config as WifiConfig, Interface, PowerSaveMode, Protocol, Protocols,
    WifiController, WifiError, ap::AccessPointInfo, scan::ScanConfig, sta::StationConfig,
};
use log::{error, info, warn};
use longfred_proto::network::roam::{BssCandidate, RoamAction, RoamEngine};
use longfred_proto::persist::RadioConfig;

use crate::config;
use crate::config::sizes;
use crate::net::{
    IS_DHCP, LAST_LEASE, NET_CONFIG_CTRL, NetStatus, RADIO, STA_NET, STATE, SsidInfo, StaNet,
    WIFI_CTRL, WIFI_CTRL_DEPTH, WIFI_HOSTNAME, WIFI_LINK, WIFI_SCAN, WifiCmd, WifiLink,
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
    // Shorten DHCP timeouts for faster recovery after link-down.
    // smoltcp defaults: discover 10 s, request 5 s, 5 retries.
    let radio = RADIO.try_get().unwrap_or_default();
    let mut retry = smoltcp::socket::dhcpv4::RetryConfig::default();
    retry.discover_timeout =
        smoltcp::time::Duration::from_secs(radio.dhcp_discover_timeout_s as u64);
    retry.initial_request_timeout = smoltcp::time::Duration::from_secs(1);
    retry.request_retries = 3;
    dhcp.retry_config = retry;
    ConfigV4::Dhcp(dhcp)
}

fn ap_to_ssid_info(ap: &AccessPointInfo) -> SsidInfo {
    let mut ssid = heapless::String::new();
    let _ = ssid.push_str(ap.ssid.as_str());
    SsidInfo {
        ssid,
        rssi: ap.signal_strength,
        open: ap.auth_method == Some(AuthenticationMethod::None),
        bssid: ap.bssid,
        channel: ap.channel,
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

/// Result of a single STA association attempt.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ConnectOutcome {
    /// Station associated with the AP.
    Associated,
    /// Connect finished with a disconnect or other error.
    Failed,
    /// `connect_async` did not complete; IDF STA is still connecting.
    /// Caller must [`abort_connecting`] before `set_config` or scan.
    Wedged,
}

#[allow(unsafe_code)]
unsafe extern "C" {
    /// Same symbol `esp-radio` calls; the public `esp_wifi_disconnect` is not
    /// in the prebuilt C6 blobs and does not link.
    fn esp_wifi_disconnect_internal() -> i32;
}

/// Abort an in-flight STA connect.
///
/// `disconnect_async` is a no-op while `is_connected()` is false, so dropping
/// a timed-out `connect_async` leaves the IDF driver in Connecting. The next
/// `set_config` then returns `ESP_ERR_WIFI_STATE` (12294) and `esp-radio`
/// 1.0.0-beta.0 panics on that unmapped code.
#[allow(unsafe_code)]
async fn abort_connecting() {
    // SAFETY: `esp_wifi_disconnect_internal` is the IDF entry used by
    // `WifiController::disconnect_impl` to cancel a STA connect-in-progress.
    // Called only after `connect_async` was dropped while the radio was still
    // connecting (`ConnectOutcome::Wedged`). The public `esp_wifi_disconnect`
    // is not exported by the C6 blobs.
    let rc = unsafe { esp_wifi_disconnect_internal() };
    if rc != 0 {
        warn!("wifi abort disconnect rc={rc}");
    }
    Timer::after(Duration::from_millis(config::network::WIFI_ABORT_SETTLE_MS)).await;
}

/// Wait for `connect_async` to finish so a later scan cannot hit IDF
/// `ESP_ERR_WIFI_STATE` (12294). `esp-radio` panics on that unmapped code.
///
/// Do not drop this future: `disconnect_async` is a no-op while
/// `is_connected()` is false, so a cancelled connect leaves STA connecting.
/// The settle timeout bounds how long we wait for a wedged radio so the
/// connection task can still service WIFI_CTRL (scan / connect commands).
/// A [`ConnectOutcome::Wedged`] return **must** be followed by
/// [`abort_connecting`] before the next `set_config` or scan.
async fn connect_sta(
    controller: &mut WifiController<'static>,
    timeout: Duration,
) -> ConnectOutcome {
    let settle = Duration::from_millis(config::network::WIFI_SETTLE_TIMEOUT_MS);
    let mut connect = pin!(controller.connect_async());
    match select(&mut connect, Timer::after(timeout)).await {
        Either::First(Ok(_)) => ConnectOutcome::Associated,
        Either::First(Err(e)) => {
            warn!("wifi connect error: {:?}", e);
            ConnectOutcome::Failed
        }
        Either::Second(()) => {
            warn!("wifi connect timeout; waiting for radio to settle");
            match select(&mut connect, Timer::after(settle)).await {
                Either::First(Ok(_)) => ConnectOutcome::Associated,
                Either::First(Err(e)) => {
                    warn!("wifi connect error after timeout: {:?}", e);
                    ConnectOutcome::Failed
                }
                Either::Second(()) => {
                    error!("wifi connect wedged after settle timeout; radio unresponsive");
                    ConnectOutcome::Wedged
                }
            }
        }
    }
}

/// Run [`connect_sta`] and abort the IDF connect if the radio wedged.
async fn connect_sta_recover(controller: &mut WifiController<'static>, timeout: Duration) -> bool {
    match connect_sta(controller, timeout).await {
        ConnectOutcome::Associated => true,
        ConnectOutcome::Failed => false,
        ConnectOutcome::Wedged => {
            abort_connecting().await;
            false
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
                mut bssid,
                mut channel,
            } => loop {
                let mut sta = StationConfig::default()
                    .with_ssid(ssid.as_str())
                    .with_password(password.as_str().into());
                if let Some(b) = bssid {
                    sta = sta.with_bssid(b);
                }
                if let Some(ch) = channel {
                    sta = sta.with_channel(ch);
                }
                let cfg = WifiConfig::Station(sta);
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
                apply_radio_phy(&mut controller);
                info!("wifi connecting to SSID={}", ssid.as_str());
                let timeout = Duration::from_millis(config::network::SSID_CONNECTION_TIMEOUT_MS);
                let associated = connect_sta_recover(&mut controller, timeout).await;
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
                            bssid: next_bssid,
                            channel: next_channel,
                        } => {
                            ssid = next_ssid;
                            password = next_password;
                            bssid = next_bssid;
                            channel = next_channel;
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
                let mut engine =
                    RoamEngine::new(controller.ap_info().map(|a| a.bssid).unwrap_or([0; 6]));
                let mut dropped = false;
                loop {
                    let radio = RADIO.try_get().unwrap_or_default();
                    let sample = Duration::from_millis(radio.roam_sample_ms as u64);
                    match select3(
                        controller.wait_for_disconnect_async(),
                        Timer::after(sample),
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
                        Either3::Second(_) => {
                            publish_wifi_link(&controller);
                            // RSSI-driven roaming: sample and feed the engine.
                            if let Ok(rssi_raw) = controller.rssi() {
                                let rssi = rssi_raw.clamp(i8::MIN as i32, i8::MAX as i32) as i8;
                                let now_ms = Instant::now().as_millis();
                                if matches!(
                                    engine.on_sample(rssi, now_ms, &radio),
                                    RoamAction::Scan { .. }
                                ) {
                                    if let Some(best) = roam_scan_pick(
                                        &mut controller,
                                        ssid.as_str(),
                                        rssi,
                                        &engine,
                                        &radio,
                                    )
                                    .await
                                    {
                                        info!(
                                            "roam: target bssid={:02x?} ch={} rssi={}",
                                            best.bssid, best.channel, best.rssi
                                        );
                                        if roam_to(
                                            &mut controller,
                                            ssid.as_str(),
                                            password.as_str(),
                                            best.bssid,
                                            best.channel,
                                            timeout,
                                        )
                                        .await
                                        {
                                            engine.on_roam_done(
                                                best.bssid,
                                                Instant::now().as_millis(),
                                                &radio,
                                            );
                                            publish_wifi_link(&controller);
                                            info!("roam: ok, now on bssid={:02x?}", best.bssid);
                                        } else {
                                            warn!("roam: target failed, falling back");
                                            if !reconnect_open(
                                                &mut controller,
                                                ssid.as_str(),
                                                password.as_str(),
                                                timeout,
                                            )
                                            .await
                                            {
                                                dropped = true;
                                                break;
                                            }
                                            engine.set_current_bssid(
                                                controller
                                                    .ap_info()
                                                    .map(|a| a.bssid)
                                                    .unwrap_or([0; 6]),
                                            );
                                            publish_wifi_link(&controller);
                                        }
                                    }
                                }
                            }
                        }
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

/// Apply power-save mode and 802.11ax protocols from the live `RADIO` config.
/// Called after every `set_config` (initial connect and BSSID-locked roam),
/// since the radio may reset PHY settings on a config change.
fn apply_radio_phy(controller: &mut WifiController<'static>) {
    let radio = RADIO.try_get().unwrap_or_default();
    if radio.power_save_off {
        if let Err(e) = controller.set_power_saving(PowerSaveMode::None) {
            warn!("wifi set_power_saving error: {:?}", e);
        }
    }
    if radio.enable_11ax {
        let protocols =
            Protocols::default().with_2_4(Protocol::B | Protocol::G | Protocol::N | Protocol::AX);
        if let Err(e) = controller.set_protocols(protocols) {
            warn!("wifi set_protocols error: {:?}", e);
        }
    }
}

/// Scan for a better AP on the same SSID and pick the best candidate.
/// Returns `Some` only if a roam should fire (RSSI >= current + hysteresis,
/// different BSSID).
async fn roam_scan_pick(
    controller: &mut WifiController<'static>,
    ssid: &str,
    current_rssi: i8,
    engine: &RoamEngine,
    radio: &RadioConfig,
) -> Option<BssCandidate> {
    let aps = match scan(controller).await {
        Ok(a) => a,
        Err(e) => {
            warn!("roam scan error: {:?}", e);
            return None;
        }
    };
    let mut candidates: heapless::Vec<BssCandidate, 8> = heapless::Vec::new();
    for ap in &aps {
        if ap.ssid.as_str() != ssid {
            continue;
        }
        let _ = candidates.push(BssCandidate {
            bssid: ap.bssid,
            channel: ap.channel,
            rssi: ap.signal_strength,
        });
    }
    engine.on_scan_results(current_rssi, &candidates, radio)
}

/// Disconnect and reconnect to a specific BSSID/channel (BSSID-locked roam).
async fn roam_to(
    controller: &mut WifiController<'static>,
    ssid: &str,
    password: &str,
    bssid: [u8; 6],
    channel: u8,
    timeout: Duration,
) -> bool {
    let _ = controller.disconnect_async().await;
    let sta = StationConfig::default()
        .with_ssid(ssid)
        .with_password(password.into())
        .with_bssid(bssid)
        .with_channel(channel);
    let cfg = WifiConfig::Station(sta);
    if let Err(e) = controller.set_config(&cfg) {
        warn!("roam set_config error: {:?}", e);
        return false;
    }
    apply_radio_phy(controller);
    connect_sta_recover(controller, timeout).await
}

/// Reconnect to the SSID without a BSSID lock (let the radio pick the best AP).
/// Used as a fallback when a BSSID-locked roam fails.
async fn reconnect_open(
    controller: &mut WifiController<'static>,
    ssid: &str,
    password: &str,
    timeout: Duration,
) -> bool {
    let sta = StationConfig::default()
        .with_ssid(ssid)
        .with_password(password.into());
    let cfg = WifiConfig::Station(sta);
    if let Err(e) = controller.set_config(&cfg) {
        warn!("reconnect set_config error: {:?}", e);
        return false;
    }
    apply_radio_phy(controller);
    connect_sta_recover(controller, timeout).await
}

/// embassy-net stack task (packet handling / DHCP).
#[embassy_executor::task]
pub async fn net_task(mut runner: Runner<'static, NetDriver>) -> ! {
    runner.run().await
}

/// Task waiting for link-up + IP; publishes NetStatus::Ready.
///
/// Owns the IPv4 address lifecycle for IP pinning: on link-down the
/// last DHCP lease is pinned as static so link-up/down transitions do not clear
/// the address. On link return, SSID + gateway reachability is validated; on
/// failure the pin is rolled back to DHCP. A watchdog (`ip_pin_max_gap_s`)
/// unpins after a long gap to avoid returning to a different network.
#[embassy_executor::task]
pub async fn status_task(stack: Stack<'static>) {
    let sender = STATE.sender();
    let lease_tx = LAST_LEASE.sender();
    loop {
        let radio = RADIO.try_get().unwrap_or_default();
        stack.wait_link_up().await;
        stack.wait_config_up().await;
        if let Some(cfg) = stack.config_v4() {
            info!("net ready: ip={}", cfg.address);
            // Save lease for pinning (DHCP mode only, pinning enabled).
            let is_dhcp = IS_DHCP.try_get().unwrap_or(true);
            let pinning = radio.ip_pinning && is_dhcp;
            let ssid = match WIFI_LINK.try_get() {
                Some(Some(l)) => l.ssid.clone(),
                _ => heapless::String::new(),
            };
            if pinning {
                let lease = StaticConfigV4 {
                    address: cfg.address,
                    gateway: cfg.gateway,
                    dns_servers: cfg.dns_servers.clone(),
                };
                lease_tx.send(Some((lease, ssid.clone())));
                info!("ip pin: saved lease for ssid={}", ssid.as_str());
            }
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

        // Pin the last lease as static so link-up/down does not clear the address.
        let pinned = pin_lease(&stack, &radio);
        if pinned {
            info!("ip pin: holding lease as static during link-down");
        }

        // Watchdog: race link-up against the gap timeout. If the gap
        // exceeds ip_pin_max_gap_s, unpin (return to DHCP) before the link comes back.
        let gap = Duration::from_secs(radio.ip_pin_max_gap_s as u64);
        let mut link_up = pin!(stack.wait_link_up());
        match select(&mut link_up, Timer::after(gap)).await {
            Either::First(_) => {
                // Link came back before the watchdog — validate the pin.
                validate_pin(stack, pinned).await;
            }
            Either::Second(_) => {
                // Watchdog fired — unpin and wait for DHCP.
                warn!("ip pin: gap exceeded {gap:?}, unpinning");
                unpin(&stack);
                stack.wait_link_up().await;
                stack.wait_config_up().await;
            }
        }
    }
}

/// Pin the last lease as static IPv4. Returns `true` if pinning is active.
fn pin_lease(stack: &Stack<'static>, radio: &RadioConfig) -> bool {
    if !radio.ip_pinning {
        return false;
    }
    let Some((lease, _ssid)) = LAST_LEASE.try_get().flatten() else {
        return false;
    };
    stack.set_config_v4(ConfigV4::Static(lease));
    true
}

/// Unpin: return to DHCP configuration.
fn unpin(stack: &Stack<'static>) {
    LAST_LEASE.sender().send(None);
    stack.set_config_v4(dhcp_config_with_hostname());
}

/// Validate the pin after link-up: check SSID match and gateway reachability.
/// On failure, unpin (return to DHCP). Roaming keeps the SSID while changing
/// BSSID, so validation is by SSID (not BSSID) plus a single ICMP echo to the
/// gateway.
async fn validate_pin(stack: Stack<'static>, pinned: bool) {
    if !pinned {
        return;
    }
    let current_ssid = match WIFI_LINK.try_get() {
        Some(Some(l)) => l.ssid.clone(),
        _ => return,
    };
    let Some((lease, saved_ssid)) = LAST_LEASE.try_get().flatten() else {
        return;
    };
    if current_ssid != saved_ssid {
        warn!(
            "ip pin: SSID changed ({} -> {}), unpinning",
            saved_ssid, current_ssid
        );
        unpin(&stack);
        return;
    }
    // Gateway reachability: a single ICMP echo with a short timeout.
    // On failure, unpin (different subnet/VLAN despite same SSID).
    let Some(gw) = lease.gateway else {
        info!("ip pin: validated (SSID match, no gateway)");
        return;
    };
    let mut rx_buffer = [0u8; 64];
    let mut tx_buffer = [0u8; 64];
    let mut rx_meta = [PacketMetadata::EMPTY];
    let mut tx_meta = [PacketMetadata::EMPTY];
    let mut ping = PingManager::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );
    let oct = gw.octets();
    let addr = Ipv4Addr::new(oct[0], oct[1], oct[2], oct[3]);
    let mut params = PingParams::new(addr);
    params.set_count(1);
    params.set_timeout(Duration::from_secs(1));
    params.set_rate_limit(Duration::from_millis(0));
    params.set_payload(b"lf");
    match ping.ping(&params).await {
        Ok(_) => info!("ip pin: validated (gateway {} reachable)", addr),
        Err(e) => {
            warn!("ip pin: gateway {} unreachable ({:?}), unpinning", addr, e);
            unpin(&stack);
        }
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
        IS_DHCP.sender().send(cfg.dhcp);
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
