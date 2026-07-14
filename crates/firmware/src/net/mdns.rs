//! Discovery serwerów WiThrottle przez mDNS (I/O; logika pakietów w longfred-proto).

use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{IpAddress, IpEndpoint, Stack};
use embassy_time::{with_timeout, Duration, Instant, Timer};
use log::{info, warn};
use longfred_proto::mdns::{
    build_ptr_query, collect_servers, WitServer, MDNS_MULTICAST_V4, MDNS_PORT,
};

use crate::config::{network, sizes};
use crate::net::{NetStatus, WitEndpoint, STATE, WIT_SERVER};

const MAX_SERVERS: usize = sizes::MAX_FOUND_WIT_SERVERS;

/// Wysyła zapytanie mDNS i zbiera serwery przez `MDNS_WAIT_MS`.
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

    let mut qbuf = [0u8; 64];
    let qlen = build_ptr_query(&mut qbuf);
    let dst = IpEndpoint::new(group, MDNS_PORT);
    if sock.send_to(&qbuf[..qlen], dst).await.is_err() {
        warn!("mdns query send failed");
    }

    let mut found: heapless::Vec<WitServer, MAX_SERVERS> = heapless::Vec::new();
    let deadline = Instant::now() + Duration::from_millis(network::MDNS_WAIT_MS);
    let mut rbuf = [0u8; 1536];

    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        match with_timeout(deadline - now, sock.recv_from(&mut rbuf)).await {
            Ok(Ok((n, _))) => {
                for s in collect_servers::<MAX_SERVERS>(&rbuf[..n]) {
                    if !found
                        .iter()
                        .any(|f| f.ipv4 == s.ipv4 && f.port == s.port)
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

/// Task: po uzyskaniu IP uruchamia discovery, wybiera serwer i publikuje.
#[embassy_executor::task]
pub async fn task(stack: Stack<'static>, ssid: &'static str) {
    wait_for_net_ready().await;

    let is_dccex = ssid.contains("DCCEX") || ssid.contains("DCC-EX");
    let selected = if is_dccex {
        info!("mdns bypass: DCC-EX AP guess");
        Some(WitEndpoint {
            ip: network::DEFAULT_WIT_IP,
            port: network::DEFAULT_WIT_PORT,
        })
    } else {
        let servers = discover(stack).await;
        for s in &servers {
            info!(
                "wit server: {} {:?}:{}",
                s.label.as_str(),
                s.ipv4,
                s.port
            );
        }
        servers
            .iter()
            .find_map(|s| s.ipv4.map(|ip| WitEndpoint { ip, port: s.port }))
            .or(Some(WitEndpoint {
                ip: network::DEFAULT_WIT_IP,
                port: network::DEFAULT_WIT_PORT,
            }))
    };

    if let Some(ep) = selected {
        info!("selected WiThrottle server {:?}:{}", ep.ip, ep.port);
    }
    WIT_SERVER.sender().send(selected);
}
