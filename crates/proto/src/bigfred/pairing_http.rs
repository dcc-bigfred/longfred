//! Bounded JSON DTOs for the firmware-only handset pairing HTTP client.

use serde::{Deserialize, Serialize};

pub const HANDSET_PAIRING_PATH: &str = "/api/v1/remotes/handset-pairing";

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

/// Return the body of an HTTP response with exactly the expected status.
#[must_use]
pub fn response_body(response: &[u8], expected_status: u16) -> Option<&[u8]> {
    let header_end = response.windows(4).position(|w| w == b"\r\n\r\n")?;
    let status_end = response.windows(2).position(|w| w == b"\r\n")?;
    let status = core::str::from_utf8(response.get(..status_end)?).ok()?;
    let mut parts = status.split_ascii_whitespace();
    let version = parts.next()?;
    let code = parts.next()?.parse::<u16>().ok()?;
    if !matches!(version, "HTTP/1.0" | "HTTP/1.1") || code != expected_status {
        return None;
    }
    response.get(header_end + 4..)
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
}
