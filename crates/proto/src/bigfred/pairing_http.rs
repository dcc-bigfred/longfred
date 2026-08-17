//! Bounded JSON DTOs for the firmware-only handset pairing HTTP client.

use serde::{Deserialize, Serialize};

pub const HANDSET_PAIRING_PATH: &str = "/api/v1/remotes/handset-pairing";
pub const HANDSET_SESSION_PATH: &str = "/api/v1/remotes/handset-session";

#[derive(Serialize)]
struct HandsetPairingRequest<'a> {
    login: &'a str,
    pin: &'a str,
    #[serde(rename = "deviceId")]
    device_id: &'a str,
}

#[derive(Deserialize)]
struct HandsetPairingResponseWire<'a> {
    #[serde(borrow)]
    #[serde(rename = "pairingCode")]
    pairing_code: &'a str,
    #[serde(rename = "expiresAt")]
    expires_at: u64,
    #[serde(rename = "layoutId")]
    layout_id: u32,
    #[serde(rename = "commandStationId")]
    command_station_id: u32,
}

/// Fields consumed by firmware after a successful HTTP 201 response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandsetPairingResponse {
    pub pairing_code: heapless::String<6>,
    pub expires_at: u64,
    pub layout_id: u32,
    pub command_station_id: u32,
}

#[derive(Deserialize)]
struct HandsetSessionResponseWire {
    paired: bool,
}

/// Session-status fields the handset needs from HTTP 200.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandsetSessionResponse {
    pub paired: bool,
}

#[derive(Deserialize)]
struct HandsetErrorWire<'a> {
    #[serde(borrow)]
    error: &'a str,
}

/// Serialize a request into caller-owned fixed storage.
pub fn encode_request(out: &mut [u8], login: &str, pin: &str, device_id: &str) -> Option<usize> {
    serde_json_core::to_slice(
        &HandsetPairingRequest {
            login,
            pin,
            device_id,
        },
        out,
    )
    .ok()
}

/// Parse only the four fields the handset needs.
#[must_use]
pub fn parse_response(body: &[u8]) -> Option<HandsetPairingResponse> {
    let (wire, _) = serde_json_core::from_slice::<HandsetPairingResponseWire<'_>>(body).ok()?;
    if wire.pairing_code.len() != 6
        || !wire.pairing_code.as_bytes().iter().all(u8::is_ascii_digit)
        || wire.layout_id == 0
        || wire.command_station_id == 0
    {
        return None;
    }
    let mut pairing_code = heapless::String::new();
    pairing_code.push_str(wire.pairing_code).ok()?;
    Some(HandsetPairingResponse {
        pairing_code,
        expires_at: wire.expires_at,
        layout_id: wire.layout_id,
        command_station_id: wire.command_station_id,
    })
}

/// Parse `{ "paired": bool }` from HTTP 200.
#[must_use]
pub fn parse_session_response(body: &[u8]) -> Option<HandsetSessionResponse> {
    let (wire, _) = serde_json_core::from_slice::<HandsetSessionResponseWire>(body).ok()?;
    Some(HandsetSessionResponse {
        paired: wire.paired,
    })
}

/// Parse `{ "error": "code" }` from an error body.
#[must_use]
pub fn parse_error_code(body: &[u8]) -> Option<heapless::String<32>> {
    let (wire, _) = serde_json_core::from_slice::<HandsetErrorWire<'_>>(body).ok()?;
    if wire.error.is_empty() || wire.error.len() > 32 {
        return None;
    }
    let mut out = heapless::String::new();
    out.push_str(wire.error).ok()?;
    Some(out)
}

/// Split status line and body from a raw HTTP response.
#[must_use]
pub fn split_response(response: &[u8]) -> Option<(u16, &[u8])> {
    let header_end = response.windows(4).position(|w| w == b"\r\n\r\n")?;
    let status_end = response.windows(2).position(|w| w == b"\r\n")?;
    let status = core::str::from_utf8(response.get(..status_end)?).ok()?;
    let mut parts = status.split_ascii_whitespace();
    let version = parts.next()?;
    let code = parts.next()?.parse::<u16>().ok()?;
    if !matches!(version, "HTTP/1.0" | "HTTP/1.1") {
        return None;
    }
    Some((code, response.get(header_end + 4..)?))
}

/// Return the body of an HTTP response with exactly the expected status.
#[must_use]
pub fn response_body(response: &[u8], expected_status: u16) -> Option<&[u8]> {
    let (code, body) = split_response(response)?;
    (code == expected_status).then_some(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_escapes_credentials() {
        let mut out = [0u8; 128];
        let n = encode_request(&mut out, "o\"ps", "1234", "4242").unwrap();
        let json = core::str::from_utf8(&out[..n]).unwrap();
        assert!(json.contains(r#""login":"o\"ps""#));
        assert!(json.contains(r#""deviceId":"4242""#));
    }

    #[test]
    fn parses_201_body_fields() {
        let raw = b"HTTP/1.1 201 Created\r\nContent-Type: application/json\r\n\r\n\
{\"pairingCode\":\"120945\",\"expiresAt\":1720000000000,\"layoutId\":1,\"commandStationId\":2}";
        let body = response_body(raw, 201).unwrap();
        let parsed = parse_response(body).unwrap();
        assert_eq!(parsed.pairing_code.as_str(), "120945");
        assert_eq!(parsed.expires_at, 1_720_000_000_000);
        assert_eq!(parsed.layout_id, 1);
        assert_eq!(parsed.command_station_id, 2);
    }

    #[test]
    fn rejects_wrong_status_or_invalid_code() {
        assert!(response_body(b"HTTP/1.1 401 Nope\r\n\r\n{}", 201).is_none());
        assert!(
            parse_response(
                br#"{"pairingCode":"12x","expiresAt":1,"layoutId":1,"commandStationId":2}"#
            )
            .is_none()
        );
    }

    #[test]
    fn parses_session_status() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n\
{\"paired\":true,\"expiresAt\":1,\"layoutId\":1,\"commandStationId\":2,\"userId\":7}";
        let (code, body) = split_response(raw).unwrap();
        assert_eq!(code, 200);
        assert!(parse_session_response(body).unwrap().paired);
        let unpaired = br#"{"paired":false,"layoutId":1,"commandStationId":2}"#;
        assert!(!parse_session_response(unpaired).unwrap().paired);
    }

    #[test]
    fn parses_error_code() {
        assert_eq!(
            parse_error_code(br#"{"error":"invalid_credentials"}"#)
                .unwrap()
                .as_str(),
            "invalid_credentials"
        );
    }
}
