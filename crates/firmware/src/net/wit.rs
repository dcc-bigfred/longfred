//! Klient TCP WiThrottle: połączenie, handshake, heartbeat, pętla I/O.

use embassy_net::tcp::TcpSocket;
use embassy_net::{IpAddress, IpEndpoint, Stack};
use embassy_time::{with_timeout, Duration, Instant, Timer};
use log::{info, warn};
use longfred_proto::events::ServerEvent;
use longfred_proto::{parser, protocol};

use crate::config;
use crate::net::{self, WitConnState, WitEndpoint, WIT_COMMANDS, WIT_CONN, WIT_EVENTS};

const TCP_RX_SIZE: usize = 1024;
const TCP_TX_SIZE: usize = 1024;
const LINE_BUF_SIZE: usize = 256;
const READ_POLL_MS: u64 = 50;
const RECONNECT_DELAY: Duration = Duration::from_secs(2);

async fn wait_for_server() -> WitEndpoint {
    loop {
        if let Some(mut rx) = net::WIT_SERVER.receiver() {
            loop {
                if let Some(ep) = rx.try_get().flatten() {
                    return ep;
                }
                rx.changed().await;
            }
        }
        Timer::after(Duration::from_millis(200)).await;
    }
}

fn log_event(ev: &ServerEvent) {
    match ev {
        ServerEvent::Version(v) => info!("wit event: version {}", v.as_str()),
        ServerEvent::RosterEntriesCount(n) => info!("wit event: roster entries {}", n),
        ServerEvent::HeartbeatConfig { seconds } => info!("wit heartbeat period {}s", seconds),
        _ => {}
    }
}

fn handle_line(line: &str, heartbeat_period: &mut u32) {
    parser::parse(line, |ev| {
        log_event(&ev);
        if let ServerEvent::HeartbeatConfig { seconds } = &ev {
            *heartbeat_period = (*seconds).max(1);
        }
        if WIT_EVENTS.try_send(ev).is_err() {
            warn!("wit events channel full");
        }
    });
}

fn feed_line_buf(line: &mut heapless::String<LINE_BUF_SIZE>, bytes: &[u8], heartbeat_period: &mut u32) {
    for &b in bytes {
        if b == b'\n' {
            let snapshot = line.as_str().trim_end_matches(['\r', '\n']);
            handle_line(snapshot, heartbeat_period);
            line.clear();
        } else if b != b'\r' {
            if line.push(b as char).is_err() {
                line.clear();
            }
        }
    }
}

async fn write_all(sock: &mut TcpSocket<'static>, mut data: &[u8]) -> bool {
    while !data.is_empty() {
        match sock.write(data).await {
            Ok(0) => return false,
            Ok(n) => data = &data[n..],
            Err(_) => return false,
        }
    }
    true
}

async fn send_cmd(sock: &mut TcpSocket<'static>, leading_crlf: &mut bool, cmd: &protocol::Cmd) -> bool {
    if config::network::SEND_LEADING_CR_LF && !*leading_crlf {
        if !write_all(sock, b"\r\n").await {
            return false;
        }
        *leading_crlf = true;
    }
    if !write_all(sock, cmd.as_bytes()).await {
        return false;
    }
    write_all(sock, b"\r\n").await
}

async fn run_session(stack: Stack<'static>, ep: WitEndpoint, heartbeat_period: &mut u32) -> bool {
    static RX: static_cell::StaticCell<[u8; TCP_RX_SIZE]> = static_cell::StaticCell::new();
    static TX: static_cell::StaticCell<[u8; TCP_TX_SIZE]> = static_cell::StaticCell::new();
    let rx = RX.init([0; TCP_RX_SIZE]);
    let tx = TX.init([0; TCP_TX_SIZE]);

    let mut sock = TcpSocket::new(stack, rx, tx);
    let remote = IpEndpoint::new(
        IpAddress::v4(ep.ip[0], ep.ip[1], ep.ip[2], ep.ip[3]),
        ep.port,
    );
    if sock.connect(remote).await.is_err() {
        warn!("wit tcp connect failed");
        return false;
    }
    info!("wit connected to {}.{}.{}.{}:{}", ep.ip[0], ep.ip[1], ep.ip[2], ep.ip[3], ep.port);
    WIT_CONN.sender().send(WitConnState::Connected);

    let mut leading_crlf = false;
    send_cmd(
        &mut sock,
        &mut leading_crlf,
        &protocol::handshake_name(config::DEVICE_NAME),
    )
    .await;
    send_cmd(
        &mut sock,
        &mut leading_crlf,
        &protocol::handshake_id(config::DEVICE_ID),
    )
    .await;
    send_cmd(
        &mut sock,
        &mut leading_crlf,
        &protocol::heartbeat_enable(config::buttons::HEARTBEAT_ENABLED),
    )
    .await;

    let mut line = heapless::String::<LINE_BUF_SIZE>::new();
    let mut read_buf = [0u8; 128];
    let mut last_rx = Instant::now();
    let mut hb_last = Instant::now();

    loop {
        let hb_period = Duration::from_secs(*heartbeat_period as u64);
        if hb_last.elapsed() >= hb_period {
            if !send_cmd(&mut sock, &mut leading_crlf, &protocol::heartbeat()).await {
                warn!("wit heartbeat write failed");
                sock.close();
                return false;
            }
            hb_last = Instant::now();
        }

        while let Ok(cmd) = WIT_COMMANDS.try_receive() {
            if !send_cmd(&mut sock, &mut leading_crlf, &cmd).await {
                warn!("wit command write failed");
                sock.close();
                return false;
            }
        }

        if last_rx.elapsed() > hb_period * 2 {
            warn!("wit watchdog: no data, reconnect");
            sock.close();
            return false;
        }

        match with_timeout(
            Duration::from_millis(READ_POLL_MS),
            sock.read(&mut read_buf),
        )
        .await
        {
            Ok(Ok(0)) => {
                warn!("wit read eof");
                sock.close();
                return false;
            }
            Ok(Ok(n)) => {
                last_rx = Instant::now();
                feed_line_buf(&mut line, &read_buf[..n], heartbeat_period);
            }
            Ok(Err(_)) => {
                warn!("wit read error");
                sock.close();
                return false;
            }
            Err(_) => {}
        }
    }
}

#[embassy_executor::task]
pub async fn task(stack: Stack<'static>) {
    let mut heartbeat_period = config::buttons::DEFAULT_HEARTBEAT_PERIOD_S;
    loop {
        WIT_CONN.sender().send(WitConnState::Connecting);
        let ep = wait_for_server().await;
        let ok = run_session(stack, ep, &mut heartbeat_period).await;
        WIT_CONN.sender().send(WitConnState::Disconnected);
        if !ok {
            Timer::after(RECONNECT_DELAY).await;
        }
    }
}
