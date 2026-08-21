//! ICMP echo to the selected command-station IP (Diagnostics screen only).

use core::net::Ipv4Addr;

use embassy_futures::select::select;
use embassy_net::Stack;
use embassy_net::icmp::PacketMetadata;
use embassy_net::icmp::ping::{PingError, PingManager, PingParams};
use embassy_time::{Duration, Timer};
use log::info;

use crate::net::{CONN, ConnState, PING, PING_ENABLE, PingStatus, SERVER};

fn ping_enabled() -> bool {
    PING_ENABLE.try_get() == Some(true)
}

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
        wait_enabled().await;
        stack.wait_config_up().await;
        if !ping_enabled() {
            ping_tx.send(PingStatus::Idle);
            continue;
        }
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
                wait_interval_or_disabled(Duration::from_secs(5)).await;
            }
            _ => {
                ping_tx.send(PingStatus::Idle);
                wait_interval_or_disabled(Duration::from_secs(2)).await;
            }
        }
        if !ping_enabled() || !stack.is_config_up() {
            ping_tx.send(PingStatus::Idle);
        }
    }
}

async fn wait_enabled() {
    if ping_enabled() {
        return;
    }
    if let Some(mut rx) = PING_ENABLE.receiver() {
        loop {
            if rx.try_get() == Some(true) {
                return;
            }
            rx.changed().await;
        }
    }
    loop {
        Timer::after(Duration::from_millis(200)).await;
        if ping_enabled() {
            return;
        }
    }
}

async fn wait_interval_or_disabled(dur: Duration) {
    if !ping_enabled() {
        return;
    }
    if let Some(mut rx) = PING_ENABLE.receiver() {
        let _ = rx.try_get();
        let _ = select(Timer::after(dur), rx.changed()).await;
        return;
    }
    Timer::after(dur).await;
}
