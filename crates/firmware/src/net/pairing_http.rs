//! Concurrent BigFred handset pairing / session-status HTTP client.

use embassy_net::tcp::TcpSocket;
use embassy_net::{IpAddress, IpEndpoint, Stack};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, with_timeout};
use log::info;
use longfred_proto::bigfred::pairing_http::{
    HANDSET_PAIRING_PATH, HANDSET_SESSION_PATH, encode_request, parse_error_code, parse_response,
    parse_session_response, split_response,
};

use super::ServerEndpoint;

const HTTP_RX_SIZE: usize = 1024;
const HTTP_TX_SIZE: usize = 1024;
const HTTP_PORT: u16 = 8080;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandsetHttpOp {
    Pair,
    Session,
}

#[derive(Clone, Debug)]
pub struct PairingHttpRequest {
    pub op: HandsetHttpOp,
    pub endpoint: ServerEndpoint,
    pub login: heapless::String<32>,
    pub pin: heapless::String<16>,
    pub device_id: heapless::String<8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PairingHttpResult {
    Code {
        endpoint: ServerEndpoint,
        code: heapless::String<6>,
    },
    Session {
        endpoint: ServerEndpoint,
        paired: bool,
    },
    Failed {
        endpoint: ServerEndpoint,
        error: heapless::String<32>,
    },
}

pub static PAIRING_HTTP_CTRL: Channel<CriticalSectionRawMutex, PairingHttpRequest, 1> =
    Channel::new();
pub static PAIRING_HTTP_RESULT: Channel<CriticalSectionRawMutex, PairingHttpResult, 1> =
    Channel::new();

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

fn failed(endpoint: ServerEndpoint, error: &str) -> PairingHttpResult {
    let mut s = heapless::String::new();
    let _ = s.push_str(error);
    PairingHttpResult::Failed { endpoint, error: s }
}

async fn post_json(
    stack: Stack<'static>,
    req: &PairingHttpRequest,
    path: &str,
    rx: &mut [u8],
    tx: &mut [u8],
) -> Option<(u16, heapless::Vec<u8, 1024>)> {
    let mut json = [0u8; 256];
    let json_len = encode_request(
        &mut json,
        req.login.as_str(),
        req.pin.as_str(),
        req.device_id.as_str(),
    )?;

    let mut sock = TcpSocket::new(stack, rx, tx);
    sock.set_timeout(Some(Duration::from_secs(3)));
    let ip = req.endpoint.ip;
    let remote = IpEndpoint::new(IpAddress::v4(ip[0], ip[1], ip[2], ip[3]), HTTP_PORT);
    if !matches!(
        with_timeout(Duration::from_secs(3), sock.connect(remote)).await,
        Ok(Ok(()))
    ) {
        return None;
    }

    let mut header = heapless::String::<256>::new();
    core::fmt::write(
        &mut header,
        format_args!(
            "POST {path} HTTP/1.1\r\nHost: bigfred\r\n\
             Content-Type: application/json\r\nContent-Length: {json_len}\r\n\
             Connection: close\r\n\r\n"
        ),
    )
    .ok()?;
    if !write_all(&mut sock, header.as_bytes()).await
        || !write_all(&mut sock, &json[..json_len]).await
    {
        return None;
    }

    let mut response = [0u8; 1024];
    let mut used = 0usize;
    while used < response.len() {
        match with_timeout(Duration::from_secs(3), sock.read(&mut response[used..])).await {
            Ok(Ok(0)) => break,
            Ok(Ok(n)) => used += n,
            Ok(Err(_)) | Err(_) => break,
        }
    }
    let (status, body) = split_response(&response[..used])?;
    let mut copied = heapless::Vec::new();
    copied.extend_from_slice(body).ok()?;
    Some((status, copied))
}

fn interpret(req: &PairingHttpRequest, status: u16, body: &[u8]) -> PairingHttpResult {
    match req.op {
        HandsetHttpOp::Pair => {
            if status == 201
                && let Some(parsed) = parse_response(body)
            {
                info!(
                    "handset pairing code received for layout {} station {}, expires {}",
                    parsed.layout_id, parsed.command_station_id, parsed.expires_at
                );
                return PairingHttpResult::Code {
                    endpoint: req.endpoint,
                    code: parsed.pairing_code,
                };
            }
            let err = parse_error_code(body).unwrap_or_else(|| {
                let mut s = heapless::String::new();
                let _ = s.push_str("pairing_failed");
                s
            });
            PairingHttpResult::Failed {
                endpoint: req.endpoint,
                error: err,
            }
        }
        HandsetHttpOp::Session => {
            if status == 200
                && let Some(parsed) = parse_session_response(body)
            {
                return PairingHttpResult::Session {
                    endpoint: req.endpoint,
                    paired: parsed.paired,
                };
            }
            let err = parse_error_code(body).unwrap_or_else(|| {
                let mut s = heapless::String::new();
                let _ = s.push_str("session_failed");
                s
            });
            PairingHttpResult::Failed {
                endpoint: req.endpoint,
                error: err,
            }
        }
    }
}

#[embassy_executor::task]
pub async fn task(stack: Stack<'static>) {
    static HTTP_RX: static_cell::ConstStaticCell<[u8; HTTP_RX_SIZE]> =
        static_cell::ConstStaticCell::new([0; HTTP_RX_SIZE]);
    static HTTP_TX: static_cell::ConstStaticCell<[u8; HTTP_TX_SIZE]> =
        static_cell::ConstStaticCell::new([0; HTTP_TX_SIZE]);
    let rx = HTTP_RX.take();
    let tx = HTTP_TX.take();
    let requests = PAIRING_HTTP_CTRL.receiver();

    loop {
        let req = requests.receive().await;
        let path = match req.op {
            HandsetHttpOp::Pair => HANDSET_PAIRING_PATH,
            HandsetHttpOp::Session => HANDSET_SESSION_PATH,
        };
        let result = match post_json(stack, &req, path, rx, tx).await {
            Some((status, body)) => interpret(&req, status, &body),
            None => failed(req.endpoint, "http_timeout"),
        };
        PAIRING_HTTP_RESULT.sender().send(result).await;
    }
}
