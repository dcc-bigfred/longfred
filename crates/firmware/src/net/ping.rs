//! ICMP echo to the selected command-station IP.

use core::net::Ipv4Addr;

use embassy_net::Stack;
use embassy_net::icmp::PacketMetadata;
use embassy_net::icmp::ping::{PingError, PingManager, PingParams};
use embassy_time::{Duration, Timer};
use log::info;

use crate::net::{CONN, ConnState, PING, PingStatus, SERVER};

#[embassy_executor::task]
pub async fn task(stack: Stack<'static>) {
    let mut rx_buffer = [0u8; 256];
    let mut tx_buffer = [0u8; 256];
    let mut rx_meta = [PacketMetadata::EMPTY];
    let mut tx_meta = [PacketMetadata::EMPTY];
    let mut ping = PingManager::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );
    let ping_tx = PING.sender();

    loop {
        stack.wait_config_up().await;
        let connected = CONN.try_get() == Some(ConnState::Connected);
        let target = SERVER.try_get().flatten();
        match (connected, target) {
            (true, Some(ep)) => {
                let addr = Ipv4Addr::new(ep.ip[0], ep.ip[1], ep.ip[2], ep.ip[3]);
                let mut params = PingParams::new(addr);
                params.set_count(1);
                params.set_timeout(Duration::from_secs(1));
                params.set_rate_limit(Duration::from_millis(0));
                params.set_payload(b"lf");
                match ping.ping(&params).await {
                    Ok(rtt) => {
                        let ms = rtt.as_millis().min(u16::MAX as u64) as u16;
                        info!("ping {addr}: {ms} ms");
                        ping_tx.send(PingStatus::Ms(ms));
                    }
                    Err(PingError::DestinationHostUnreachable) => {
                        ping_tx.send(PingStatus::Timeout);
                    }
                    Err(_) => {
                        ping_tx.send(PingStatus::Idle);
                    }
                }
                Timer::after(Duration::from_secs(5)).await;
            }
            _ => {
                ping_tx.send(PingStatus::Idle);
                Timer::after(Duration::from_secs(2)).await;
            }
        }
        if !stack.is_config_up() {
            ping_tx.send(PingStatus::Idle);
        }
    }
}
