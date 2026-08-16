//! Minimal HTTP/1.1 server for Soft-AP provisioning and STA firmware OTA.

use embassy_net::Stack;
use embassy_net::tcp::TcpSocket;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use log::{info, warn};
use longfred_proto::persist::PersistRecord;
use longfred_proto::provisioning::{
    apply_settings_put, deserialize_settings_put, serialize_settings_from_record,
};

use crate::net::provisioning::exit_programming_mode;
use crate::net::provisioning::ota;
use crate::net::{self, HTTP_OTA_BUSY};
use crate::storage::{STORAGE_ACK, STORAGE_CTRL, SharedFlash, StorageCmd};
use crate::ui::UI_VIEW;
use crate::ui::i18n;
use crate::ui::view::{GridView, UiView};

const INDEX_HTML: &str = include_str!("index.html");

const RX_BUF: usize = 2048;
const TX_BUF: usize = 4096;
const BODY_MAX: usize = 1536;
const JSON_MAX: usize = 1536;
const FW_TIMEOUT_SECS: u64 = 120;

/// Which routes the HTTP server exposes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HttpMode {
    /// Soft-AP: settings + firmware + exit programming mode.
    Ap,
    /// STA: firmware upload only (plus GET settings / page).
    Sta,
}

#[embassy_executor::task]
pub async fn task_ap(
    stack: Stack<'static>,
    rec: &'static Mutex<CriticalSectionRawMutex, PersistRecord>,
    flash: &'static SharedFlash,
) {
    info!("programming: HTTP listening on :80");
    serve_loop(stack, Some(rec), flash, HttpMode::Ap, false).await;
}

#[embassy_executor::task]
pub async fn task_sta(
    stack: Stack<'static>,
    rec: &'static Mutex<CriticalSectionRawMutex, PersistRecord>,
    flash: &'static SharedFlash,
) {
    info!("http-ota: STA server idle until enabled");
    serve_loop(stack, Some(rec), flash, HttpMode::Sta, true).await;
}

async fn serve_loop(
    stack: Stack<'static>,
    rec: Option<&'static Mutex<CriticalSectionRawMutex, PersistRecord>>,
    flash: &'static SharedFlash,
    mode: HttpMode,
    gated: bool,
) {
    loop {
        if gated {
            wait_enabled().await;
        }
        let mut rx = [0u8; RX_BUF];
        let mut tx = [0u8; TX_BUF];
        let mut sock = TcpSocket::new(stack, &mut rx, &mut tx);
        let timeout = if gated {
            Duration::from_secs(2)
        } else {
            Duration::from_secs(20)
        };
        sock.set_timeout(Some(timeout));

        if gated && !is_enabled() {
            sock.abort();
            continue;
        }

        match sock.accept(80).await {
            Ok(()) => {}
            Err(_) => {
                Timer::after(Duration::from_millis(50)).await;
                continue;
            }
        }

        if let Err(e) = handle_client(&mut sock, rec, flash, mode).await {
            warn!("http: request error: {}", e);
        }
        sock.abort();
        let _ = sock.flush().await;
    }
}

fn is_enabled() -> bool {
    net::http_ota_enabled()
}

fn show_updating() {
    let mut grid = GridView::new();
    grid.set(0, i18n::tr().msg_fw_updating, false);
    UI_VIEW.sender().send(UiView::Grid(grid));
}

async fn wait_enabled() {
    if is_enabled() {
        return;
    }
    if let Some(mut rx) = crate::net::HTTP_OTA_ENABLE.receiver() {
        loop {
            if rx.try_get() == Some(true) {
                return;
            }
            rx.changed().await;
        }
    }
    loop {
        Timer::after(Duration::from_millis(200)).await;
        if is_enabled() {
            return;
        }
    }
}

async fn handle_client(
    sock: &mut TcpSocket<'_>,
    rec: Option<&'static Mutex<CriticalSectionRawMutex, PersistRecord>>,
    flash: &'static SharedFlash,
    mode: HttpMode,
) -> Result<(), &'static str> {
    let mut hdr = [0u8; RX_BUF];
    let n = read_headers(sock, &mut hdr).await?;
    let (method, path, content_len) = parse_request(&hdr[..n])?;
    let header_end = find_header_end(&hdr[..n]).ok_or("bad headers")?;
    let already = n.saturating_sub(header_end);
    let already_bytes = if already > 0 {
        &hdr[header_end..header_end + already]
    } else {
        &[][..]
    };

    if method == "POST" && path == "/api/v1/firmware" {
        return handle_firmware(sock, flash, already_bytes, content_len).await;
    }

    let mut body_buf = [0u8; BODY_MAX];
    let body = if content_len > 0 {
        if content_len > BODY_MAX {
            return respond(sock, 400, "text/plain", b"body too large").await;
        }
        if already > content_len {
            return Err("bad body framing");
        }
        body_buf[..already].copy_from_slice(&hdr[header_end..header_end + already]);
        let mut got = already;
        while got < content_len {
            match sock.read(&mut body_buf[got..content_len]).await {
                Ok(0) => return Err("eof body"),
                Ok(k) => got += k,
                Err(_) => return Err("read body"),
            }
        }
        &body_buf[..content_len]
    } else {
        &[][..]
    };

    match (method, path, mode) {
        ("GET", "/", _) => {
            respond(sock, 200, "text/html; charset=utf-8", INDEX_HTML.as_bytes()).await
        }
        ("GET", "/api/v1/settings", _) => {
            let Some(rec) = rec else {
                return respond(sock, 503, "text/plain", b"no settings").await;
            };
            let guard = rec.lock().await;
            let mut json = [0u8; JSON_MAX];
            match serialize_settings_from_record(&mut json, &*guard, i18n::FW_VERSION) {
                Ok(len) => respond(sock, 200, "application/json", &json[..len]).await,
                Err(_) => respond(sock, 500, "text/plain", b"serialize error").await,
            }
        }
        ("PUT", "/api/v1/settings", HttpMode::Ap) => {
            let Some(rec) = rec else {
                return respond(sock, 503, "text/plain", b"no settings").await;
            };
            handle_settings_put(sock, rec, body).await
        }
        ("POST", "/api/v1/programming-mode/off", HttpMode::Ap) => {
            respond(sock, 200, "application/json", b"{\"ok\":true}").await?;
            exit_programming_mode(500).await
        }
        _ => respond(sock, 404, "text/plain", b"not found").await,
    }
}

async fn handle_settings_put(
    sock: &mut TcpSocket<'_>,
    rec: &'static Mutex<CriticalSectionRawMutex, PersistRecord>,
    body: &[u8],
) -> Result<(), &'static str> {
    let put = deserialize_settings_put(body).map_err(|_| "bad json")?;
    let mut guard = rec.lock().await;
    let apply_err = apply_settings_put(&mut *guard, &put);
    let msg: &'static str = match apply_err {
        Ok(()) => "",
        Err(longfred_proto::provisioning::ApplyError::HostnameTooLong) => "hostname too long",
        Err(longfred_proto::provisioning::ApplyError::LoginTooLong) => "login too long",
        Err(longfred_proto::provisioning::ApplyError::PinTooLong) => "pin too long",
        Err(longfred_proto::provisioning::ApplyError::RosterAddrTooLong) => "roster addr too long",
        Err(longfred_proto::provisioning::ApplyError::RosterNameTooLong) => "roster name too long",
        Err(longfred_proto::provisioning::ApplyError::RosterFull) => "roster full",
    };
    if !msg.is_empty() {
        return respond(sock, 400, "text/plain", msg.as_bytes()).await;
    }
    let snapshot = guard.clone();
    drop(guard);
    let tx = STORAGE_CTRL.sender();
    if tx.try_send(StorageCmd::ReplaceRecord(snapshot)).is_err() {
        return respond(sock, 503, "text/plain", b"storage busy").await;
    }
    if !STORAGE_ACK.wait().await {
        return respond(sock, 500, "text/plain", b"storage persist failed").await;
    }
    respond(sock, 200, "application/json", b"{\"ok\":true}").await
}

async fn handle_firmware(
    sock: &mut TcpSocket<'_>,
    flash: &'static SharedFlash,
    already: &[u8],
    content_len: usize,
) -> Result<(), &'static str> {
    if content_len == 0 {
        return respond(sock, 400, "text/plain", b"empty image").await;
    }
    sock.set_timeout(Some(Duration::from_secs(FW_TIMEOUT_SECS)));
    HTTP_OTA_BUSY.sender().send(true);
    show_updating();
    let result = {
        let mut g = flash.lock().await;
        ota::flash_from_socket(&mut g, sock, already, content_len).await
    };
    HTTP_OTA_BUSY.sender().send(false);
    match result {
        Ok(()) => {
            respond(sock, 200, "application/json", b"{\"ok\":true}").await?;
            let _ = sock.flush().await;
            Timer::after(Duration::from_millis(500)).await;
            esp_hal::system::software_reset();
        }
        Err(msg) => {
            warn!("ota: {msg}");
            respond(sock, 400, "text/plain", msg.as_bytes()).await
        }
    }
}

async fn read_headers(sock: &mut TcpSocket<'_>, buf: &mut [u8]) -> Result<usize, &'static str> {
    let mut n = 0usize;
    loop {
        if n >= buf.len() {
            return Err("headers too large");
        }
        match sock.read(&mut buf[n..]).await {
            Ok(0) => return Err("eof"),
            Ok(k) => {
                n += k;
                if find_header_end(&buf[..n]).is_some() {
                    return Ok(n);
                }
            }
            Err(_) => return Err("read"),
        }
    }
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
}

fn parse_request(buf: &[u8]) -> Result<(&str, &str, usize), &'static str> {
    let header_end = find_header_end(buf).ok_or("incomplete")?;
    let head = core::str::from_utf8(&buf[..header_end]).map_err(|_| "utf8")?;
    let mut lines = head.split("\r\n");
    let req = lines.next().ok_or("no request line")?;
    let mut parts = req.split_whitespace();
    let method = parts.next().ok_or("no method")?;
    let path = parts.next().ok_or("no path")?;
    let path = path.split('?').next().unwrap_or(path);

    let mut content_len = 0usize;
    for line in lines {
        let lower = line.as_bytes();
        if lower.len() >= 15 && lower[..15].eq_ignore_ascii_case(b"content-length:") {
            let v = line[15..].trim();
            content_len = v.parse().map_err(|_| "bad content-length")?;
        }
    }
    Ok((method, path, content_len))
}

async fn respond(
    sock: &mut TcpSocket<'_>,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<(), &'static str> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let mut hdr = heapless::String::<160>::new();
    if core::fmt::write(
        &mut hdr,
        format_args!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        ),
    )
    .is_err()
    {
        return Err("header overflow");
    }
    write_all(sock, hdr.as_bytes()).await?;
    write_all(sock, body).await?;
    sock.flush().await.map_err(|_| "flush")?;
    Ok(())
}

async fn write_all(sock: &mut TcpSocket<'_>, mut data: &[u8]) -> Result<(), &'static str> {
    while !data.is_empty() {
        match sock.write(data).await {
            Ok(0) => return Err("write zero"),
            Ok(n) => data = &data[n..],
            Err(_) => return Err("write"),
        }
    }
    Ok(())
}
