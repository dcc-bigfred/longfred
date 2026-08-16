//! Discovery of WiThrottle and Z21 command stations via mDNS.

use embassy_futures::select::{Either, select};
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{IpAddress, IpEndpoint, Stack};
use embassy_time::{Duration, Instant, Timer, with_timeout};
use log::{info, warn};
use longfred_proto::command::Protocol;
use longfred_proto::mdns::{
    MDNS_MULTICAST_V4, MDNS_PORT, WITHROTTLE_SERVICE, WitServer, Z21_SERVICE, build_ptr_query,
    collect_servers,
};

use crate::config::{network, sizes};
use crate::net::{
    FOUND_SERVERS, HTTP_OTA_ENABLE, MDNS_CTRL, NetStatus, SERVER, STATE, ServerEndpoint,
    WIFI_HOSTNAME,
};

const MAX_SERVERS: usize = sizes::MAX_FOUND_SERVERS;

async fn query_service(
    sock: &mut UdpSocket<'_>,
    group: IpEndpoint,
    service: &str,
    protocol: Protocol,
    found: &mut heapless::Vec<WitServer, MAX_SERVERS>,
) {
    let mut qbuf = [0u8; 64];
    let qlen = build_ptr_query(service, &mut qbuf);
    if sock.send_to(&qbuf[..qlen], group).await.is_err() {
        warn!("mdns query send failed for {}", service);
        return;
    }

    let deadline = Instant::now() + Duration::from_millis(network::MDNS_WAIT_MS / 2);
    let mut rbuf = [0u8; 1536];
    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        match with_timeout(deadline - now, sock.recv_from(&mut rbuf)).await {
            Ok(Ok((n, _))) => {
                for s in collect_servers::<MAX_SERVERS>(&rbuf[..n], protocol) {
                    if !found
                        .iter()
                        .any(|f| f.ipv4 == s.ipv4 && f.port == s.port && f.protocol == s.protocol)
                    {
                        let _ = found.push(s);
                    }
                }
                if found.len() >= MAX_SERVERS {
                    break;
                }
            }
            _ => break,
        }
    }
}

/// Send mDNS PTR queries and collect servers for `MDNS_WAIT_MS`.
pub async fn discover(stack: Stack<'static>) -> heapless::Vec<WitServer, MAX_SERVERS> {
    let mut rx_meta = [PacketMetadata::EMPTY; 8];
    let mut rx_buf = [0u8; 1536];
    let mut tx_meta = [PacketMetadata::EMPTY; 8];
    let mut tx_buf = [0u8; 256];
    let mut sock = UdpSocket::new(stack, &mut rx_meta, &mut rx_buf, &mut tx_meta, &mut tx_buf);

    let group = IpAddress::v4(
        MDNS_MULTICAST_V4[0],
        MDNS_MULTICAST_V4[1],
        MDNS_MULTICAST_V4[2],
        MDNS_MULTICAST_V4[3],
    );
    if let Err(e) = stack.join_multicast_group(group) {
        warn!("mdns join multicast failed: {:?}", e);
    }
    if sock.bind(MDNS_PORT).is_err() {
        warn!("mdns bind 5353 failed");
        let _ = stack.leave_multicast_group(group);
        return heapless::Vec::new();
    }

    let dst = IpEndpoint::new(group, MDNS_PORT);
    let mut found: heapless::Vec<WitServer, MAX_SERVERS> = heapless::Vec::new();
    query_service(
        &mut sock,
        dst,
        WITHROTTLE_SERVICE,
        Protocol::WiThrottle,
        &mut found,
    )
    .await;
    query_service(&mut sock, dst, Z21_SERVICE, Protocol::Z21, &mut found).await;

    let _ = stack.leave_multicast_group(group);
    found
}

async fn wait_for_net_ready() {
    if let Some(mut rx) = STATE.receiver() {
        loop {
            if rx.try_get() == Some(NetStatus::Ready) {
                return;
            }
            rx.changed().await;
        }
    } else {
        loop {
            Timer::after(Duration::from_millis(500)).await;
            if STATE.sender().try_get() == Some(NetStatus::Ready) {
                return;
            }
        }
    }
}

async fn run_discovery(
    stack: Stack<'static>,
    ssid: &'static str,
) -> heapless::Vec<WitServer, MAX_SERVERS> {
    let is_dccex = ssid.contains("DCCEX") || ssid.contains("DCC-EX");
    if is_dccex {
        info!("mdns bypass: DCC-EX AP guess");
        let mut v: heapless::Vec<WitServer, MAX_SERVERS> = heapless::Vec::new();
        let mut label = longfred_proto::model::ShortText::new();
        let _ = label.push_str("DCC-EX");
        let _ = v.push(WitServer {
            label,
            ipv4: Some(network::DEFAULT_WIT_IP),
            port: network::DEFAULT_WIT_PORT,
            protocol: Protocol::WiThrottle,
        });
        return v;
    }

    let servers = discover(stack).await;
    for s in &servers {
        info!(
            "server: {} {:?}:{} {:?}",
            s.label.as_str(),
            s.ipv4,
            s.port,
            s.protocol
        );
    }
    servers
}

fn maybe_auto_connect(servers: &heapless::Vec<WitServer, MAX_SERVERS>) {
    if !network::AUTO_CONNECT_TO_FIRST_WITHROTTLE_SERVER {
        return;
    }
    if let Some(s) = servers.iter().find_map(|s| {
        s.ipv4.map(|ip| ServerEndpoint {
            ip,
            port: s.port,
            protocol: s.protocol,
        })
    }) {
        info!(
            "auto-selected server {:?}:{} {:?}",
            s.ip, s.port, s.protocol
        );
        SERVER.sender().send(Some(s));
    }
}

/// Task: discovery on demand (`MDNS_CTRL`) after IP is ready.
#[embassy_executor::task]
pub async fn task(stack: Stack<'static>, ssid: &'static str) {
    wait_for_net_ready().await;
    let mdns_rx = MDNS_CTRL.receiver();

    loop {
        if HTTP_OTA_ENABLE.try_get() == Some(true) {
            Timer::after(Duration::from_millis(500)).await;
            continue;
        }
        let servers = run_discovery(stack, ssid).await;
        maybe_auto_connect(&servers);
        FOUND_SERVERS.signal(servers);

        match select(mdns_rx.receive(), Timer::after(Duration::from_secs(3600))).await {
            Either::First(()) => {}
            Either::Second(_) => {}
        }
    }
}

/// Advertise `_longfred-ota._tcp.local` while STA HTTP OTA is enabled.
#[embassy_executor::task]
pub async fn ota_announce_task(stack: Stack<'static>) {
    loop {
        wait_ota_enabled().await;
        let Some(ip) = crate::net::sta_ipv4() else {
            Timer::after(Duration::from_millis(200)).await;
            continue;
        };
        let hostname = WIFI_HOSTNAME
            .try_get()
            .filter(|h| !h.is_empty())
            .unwrap_or_else(|| {
                let mut s = heapless::String::new();
                let _ = s.push_str("longfred");
                s
            });

        let mut rx_meta = [PacketMetadata::EMPTY; 4];
        let mut rx_buf = [0u8; 512];
        let mut tx_meta = [PacketMetadata::EMPTY; 4];
        let mut tx_buf = [0u8; 512];
        let mut sock = UdpSocket::new(stack, &mut rx_meta, &mut rx_buf, &mut tx_meta, &mut tx_buf);
        let group = IpAddress::v4(
            MDNS_MULTICAST_V4[0],
            MDNS_MULTICAST_V4[1],
            MDNS_MULTICAST_V4[2],
            MDNS_MULTICAST_V4[3],
        );
        let _ = stack.join_multicast_group(group);
        if sock.bind(MDNS_PORT).is_err() {
            warn!("ota-mdns: bind 5353 failed");
            Timer::after(Duration::from_secs(1)).await;
            continue;
        }
        let dst = IpEndpoint::new(group, MDNS_PORT);
        info!("ota-mdns: announcing {} at {:?}:80", hostname.as_str(), ip);
        while crate::net::http_ota_enabled() {
            let mut pkt = [0u8; 512];
            let n = longfred_proto::mdns::build_ota_announce(hostname.as_str(), ip, 80, &mut pkt);
            let _ = sock.send_to(&pkt[..n], dst).await;
            Timer::after(Duration::from_secs(2)).await;
        }
        let _ = stack.leave_multicast_group(group);
        info!("ota-mdns: stopped");
    }
}

async fn wait_ota_enabled() {
    if HTTP_OTA_ENABLE.try_get() == Some(true) {
        return;
    }
    if let Some(mut rx) = HTTP_OTA_ENABLE.receiver() {
        loop {
            if rx.try_get() == Some(true) {
                return;
            }
            rx.changed().await;
        }
    }
    loop {
        Timer::after(Duration::from_millis(200)).await;
        if HTTP_OTA_ENABLE.try_get() == Some(true) {
            return;
        }
    }
}
