//! Generic protocol session: TCP (WiThrottle) or UDP (Z21) with shared adapter loop.

use embassy_futures::select::{Either3, select3};
use embassy_net::tcp::TcpSocket;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{IpAddress, IpEndpoint, Stack};
use embassy_time::{Duration, Instant, Timer};
use log::{info, warn};
use longfred_proto::adapter::{Adapter, WireBuf};
use longfred_proto::bigfred::BigFredAdapter;
use longfred_proto::command::Protocol;
use longfred_proto::events::ServerEvent;
use longfred_proto::persist::DeviceIdentity;
use longfred_proto::wt::WtAdapter;
use longfred_proto::z21::Z21Adapter;

use crate::config;
use crate::net::{CONN, ConnState, DEVICE, PROTO_COMMANDS, PROTO_EVENTS, SERVER, ServerEndpoint};

const TCP_RX_SIZE: usize = 1024;
const TCP_TX_SIZE: usize = 1024;
const UDP_RX_SIZE: usize = 1536;
const UDP_TX_SIZE: usize = 512;
const SESSION_TICK: Duration = Duration::from_millis(250);

trait Transport {
    async fn send(&mut self, data: &[u8]) -> bool;
    async fn recv(&mut self, buf: &mut [u8]) -> Option<usize>;
}

struct TcpTransport<'a> {
    sock: TcpSocket<'a>,
}

impl Transport for TcpTransport<'_> {
    async fn send(&mut self, data: &[u8]) -> bool {
        write_all(&mut self.sock, data).await
    }

    async fn recv(&mut self, buf: &mut [u8]) -> Option<usize> {
        match self.sock.read(buf).await {
            Ok(0) => None,
            Ok(n) => Some(n),
            Err(_) => None,
        }
    }
}

struct UdpTransport<'a> {
    sock: UdpSocket<'a>,
    remote: IpEndpoint,
}

impl Transport for UdpTransport<'_> {
    async fn send(&mut self, data: &[u8]) -> bool {
        self.sock.send_to(data, self.remote).await.is_ok()
    }

    async fn recv(&mut self, buf: &mut [u8]) -> Option<usize> {
        match self.sock.recv_from(buf).await {
            Ok((n, _)) => Some(n),
            Err(_) => None,
        }
    }
}

async fn wait_for_server() -> ServerEndpoint {
    loop {
        if let Some(mut rx) = SERVER.receiver() {
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
        ServerEvent::Version(v) => info!("proto event: version {}", v.as_str()),
        ServerEvent::RosterEntriesCount(n) => info!("proto event: roster entries {}", n),
        ServerEvent::HeartbeatConfig { seconds } => info!("proto heartbeat period {}s", seconds),
        _ => {}
    }
}

fn emit_event(ev: ServerEvent) {
    log_event(&ev);
    if PROTO_EVENTS.try_send(ev).is_err() {
        warn!("proto events channel full");
    }
}

async fn write_all(sock: &mut TcpSocket<'_>, mut data: &[u8]) -> bool {
    while !data.is_empty() {
        match sock.write(data).await {
            Ok(0) => return false,
            Ok(n) => data = &data[n..],
            Err(_) => return false,
        }
    }
    true
}

fn make_adapter(ep: ServerEndpoint) -> Adapter {
    let dev = DEVICE
        .sender()
        .try_get()
        .unwrap_or_else(DeviceIdentity::empty);
    let id = dev.id_wire();
    match ep.protocol {
        Protocol::WiThrottle => Adapter::Wt(WtAdapter::new(
            dev.name.as_str(),
            id.as_str(),
            config::buttons::DEFAULT_HEARTBEAT_PERIOD_S,
            config::network::SEND_LEADING_CR_LF,
            config::buttons::HEARTBEAT_ENABLED,
        )),
        Protocol::BigFred => Adapter::BigFred(BigFredAdapter::new(
            dev.name.as_str(),
            id.as_str(),
            config::buttons::DEFAULT_HEARTBEAT_PERIOD_S,
            config::network::SEND_LEADING_CR_LF,
            config::buttons::HEARTBEAT_ENABLED,
        )),
        Protocol::Z21 => Adapter::Z21(Z21Adapter::new()),
    }
}

async fn run_session<T: Transport>(mut tr: T, mut adapter: Adapter) -> bool {
    let mut out = WireBuf::new();
    adapter.on_connect(&mut out, &mut |ev| emit_event(ev));
    if !out.is_empty() && !tr.send(&out).await {
        return true;
    }

    let cmd_rx = PROTO_COMMANDS.receiver();
    let mut rx = [0u8; 512];
    let mut last_rx = Instant::now();
    let mut hb_last = Instant::now();

    loop {
        let hb_period = Duration::from_secs(adapter.tick_period_s() as u64);
        match select3(
            tr.recv(&mut rx),
            cmd_rx.receive(),
            Timer::after(SESSION_TICK),
        )
        .await
        {
            Either3::First(None) => return true,
            Either3::First(Some(n)) => {
                last_rx = Instant::now();
                let mut hb_cfg = None;
                adapter.decode(&rx[..n], &mut |ev| {
                    if let ServerEvent::HeartbeatConfig { seconds } = &ev {
                        hb_cfg = Some(*seconds);
                    }
                    emit_event(ev);
                });
                if let Some(seconds) = hb_cfg {
                    adapter.set_heartbeat_period(seconds);
                }
            }
            Either3::Second(cmd) => {
                let mut out = WireBuf::new();
                adapter.encode(&cmd, &mut out, &mut |ev| emit_event(ev));
                if !out.is_empty() && !tr.send(&out).await {
                    return true;
                }
            }
            Either3::Third(_) => {
                let mut out = WireBuf::new();
                let polled = adapter.poll(&mut out, &mut |ev| emit_event(ev));
                if polled && !out.is_empty() && !tr.send(&out).await {
                    return true;
                }
                if hb_last.elapsed() >= hb_period {
                    let mut out = WireBuf::new();
                    if adapter.on_tick(&mut out) && !out.is_empty() && !tr.send(&out).await {
                        return true;
                    }
                    hb_last = Instant::now();
                }
                if last_rx.elapsed() > hb_period * 2 {
                    warn!("proto watchdog: no data, reconnect");
                    return true;
                }
            }
        }
    }
}

async fn run_tcp_session(
    stack: Stack<'static>,
    ep: ServerEndpoint,
    rx: &mut [u8],
    tx: &mut [u8],
) -> bool {
    let mut sock = TcpSocket::new(stack, rx, tx);
    sock.set_nagle_enabled(!config::network::TCP_NODELAY);
    sock.set_timeout(Some(Duration::from_secs(config::network::TCP_TIMEOUT_S)));
    sock.set_keep_alive(Some(Duration::from_secs(config::network::TCP_KEEPALIVE_S)));

    let remote = IpEndpoint::new(
        IpAddress::v4(ep.ip[0], ep.ip[1], ep.ip[2], ep.ip[3]),
        ep.port,
    );
    if sock.connect(remote).await.is_err() {
        warn!("tcp connect failed");
        return false;
    }
    info!(
        "wit connected to {}.{}.{}.{}:{}",
        ep.ip[0], ep.ip[1], ep.ip[2], ep.ip[3], ep.port
    );
    CONN.sender().send(ConnState::Connected);

    let adapter = make_adapter(ep);
    let tr = TcpTransport { sock };
    run_session(tr, adapter).await;
    true
}

async fn run_udp_session(
    stack: Stack<'static>,
    ep: ServerEndpoint,
    rx_meta: &mut [PacketMetadata],
    rx: &mut [u8],
    tx_meta: &mut [PacketMetadata],
    tx: &mut [u8],
) -> bool {
    let mut sock = UdpSocket::new(stack, rx_meta, rx, tx_meta, tx);
    if sock.bind(0).is_err() {
        warn!("z21 udp bind failed");
        return false;
    }

    let remote = IpEndpoint::new(
        IpAddress::v4(ep.ip[0], ep.ip[1], ep.ip[2], ep.ip[3]),
        ep.port,
    );
    info!(
        "z21 session to {}.{}.{}.{}:{}",
        ep.ip[0], ep.ip[1], ep.ip[2], ep.ip[3], ep.port
    );
    CONN.sender().send(ConnState::Connected);

    let adapter = make_adapter(ep);
    let tr = UdpTransport { sock, remote };
    run_session(tr, adapter).await
}

#[embassy_executor::task]
pub async fn task(stack: Stack<'static>) {
    // Taken once: reconnect must reuse these buffers (`StaticCell::init` panics on the 2nd call).
    static TCP_RX: static_cell::ConstStaticCell<[u8; TCP_RX_SIZE]> =
        static_cell::ConstStaticCell::new([0; TCP_RX_SIZE]);
    static TCP_TX: static_cell::ConstStaticCell<[u8; TCP_TX_SIZE]> =
        static_cell::ConstStaticCell::new([0; TCP_TX_SIZE]);
    static UDP_RX_META: static_cell::ConstStaticCell<[PacketMetadata; 4]> =
        static_cell::ConstStaticCell::new([PacketMetadata::EMPTY; 4]);
    static UDP_TX_META: static_cell::ConstStaticCell<[PacketMetadata; 4]> =
        static_cell::ConstStaticCell::new([PacketMetadata::EMPTY; 4]);
    static UDP_RX: static_cell::ConstStaticCell<[u8; UDP_RX_SIZE]> =
        static_cell::ConstStaticCell::new([0; UDP_RX_SIZE]);
    static UDP_TX: static_cell::ConstStaticCell<[u8; UDP_TX_SIZE]> =
        static_cell::ConstStaticCell::new([0; UDP_TX_SIZE]);
    let tcp_rx = TCP_RX.take();
    let tcp_tx = TCP_TX.take();
    let udp_rx_meta = UDP_RX_META.take();
    let udp_tx_meta = UDP_TX_META.take();
    let udp_rx = UDP_RX.take();
    let udp_tx = UDP_TX.take();

    let mut connect_fails: u32 = 0;
    loop {
        CONN.sender().send(ConnState::Connecting);
        let ep = wait_for_server().await;
        let established = match ep.protocol {
            Protocol::WiThrottle | Protocol::BigFred => {
                run_tcp_session(stack, ep, tcp_rx, tcp_tx).await
            }
            Protocol::Z21 => {
                run_udp_session(stack, ep, udp_rx_meta, udp_rx, udp_tx_meta, udp_tx).await
            }
        };
        CONN.sender().send(ConnState::Disconnected);
        if established {
            connect_fails = 0;
            Timer::after(Duration::from_millis(config::network::RECONNECT_MIN_MS)).await;
        } else {
            let backoff = (config::network::RECONNECT_MIN_MS << connect_fails.min(4))
                .min(config::network::RECONNECT_MAX_MS);
            connect_fails = connect_fails.saturating_add(1);
            Timer::after(Duration::from_millis(backoff)).await;
        }
    }
}
