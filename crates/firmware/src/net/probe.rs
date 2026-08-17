//! HTTP probe that distinguishes BigFred from a generic WiThrottle host.

use embassy_net::tcp::TcpSocket;
use embassy_net::{IpAddress, IpEndpoint, Stack};
use embassy_time::{Duration, with_timeout};
use longfred_proto::caps::{Probe, http_probe_matches};
use longfred_proto::command::Protocol;

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

/// True when `ip` answers the BigFred HTTP probe on the protocol's probe port.
pub async fn is_bigfred(stack: Stack<'static>, ip: [u8; 4], rx: &mut [u8], tx: &mut [u8]) -> bool {
    let Probe::HttpGet { port, path, expect } = Protocol::BigFred.probe() else {
        return false;
    };
    let mut sock = TcpSocket::new(stack, rx, tx);
    sock.set_timeout(Some(Duration::from_secs(2)));
    let remote = IpEndpoint::new(IpAddress::v4(ip[0], ip[1], ip[2], ip[3]), port);
    if !matches!(
        with_timeout(Duration::from_secs(2), sock.connect(remote)).await,
        Ok(Ok(()))
    ) {
        return false;
    }
    let mut request = heapless::String::<128>::new();
    if core::fmt::write(
        &mut request,
        format_args!("GET {path} HTTP/1.1\r\nHost: bigfred\r\nConnection: close\r\n\r\n"),
    )
    .is_err()
        || !write_all(&mut sock, request.as_bytes()).await
    {
        return false;
    }

    let mut response = [0u8; 1024];
    let mut used = 0usize;
    while used < response.len() {
        match with_timeout(Duration::from_secs(2), sock.read(&mut response[used..])).await {
            Ok(Ok(0)) => break,
            Ok(Ok(n)) => used += n,
            Ok(Err(_)) | Err(_) => break,
        }
    }
    http_probe_matches(&response[..used], expect.as_bytes())
}
