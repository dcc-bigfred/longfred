//! Shared list / IP helpers used by multiple screens.

use longfred_proto::persist::StaticIpConfig;

use crate::context::ScreenCtx;
use crate::nav::{PageDir, Step};
use crate::view::Line;
use crate::widgets::PagedList;

/// True when the current throttle slot has an acquired loco.
pub fn has_loco(cx: &ScreenCtx<'_>) -> bool {
    cx.drive
        .slots
        .get(cx.drive.current)
        .is_some_and(longfred_proto::model::ThrottleSlot::has_loco)
}

/// OLED height in pixels (paging).
pub fn height(cx: &ScreenCtx<'_>) -> u16 {
    cx.env.geometry.height
}

/// Move the list cursor one row.
pub fn step_list(list: &mut PagedList, d: Step, items: &[&str], numbered: bool, h: u16) {
    match d {
        Step::Prev => list.list_prev(items, numbered, h),
        Step::Next => list.list_next(items, numbered, h),
    }
}

/// Flip one page of a paged list.
pub fn page_list(list: &mut PagedList, d: PageDir, items: &[&str], numbered: bool, h: u16) {
    match d {
        PageDir::Prev => list.page_prev(items, h),
        PageDir::Next => list.page_next(items, numbered, h),
    }
}

/// Append `a.b.c.d` with zero-padded octets.
pub fn write_ip_line(line: &mut Line, ip: [u8; 4]) {
    for (i, oct) in ip.iter().enumerate() {
        if i > 0 {
            let _ = line.push('.');
        }
        let _ = line.push((b'0' + oct / 100) as char);
        let _ = line.push((b'0' + (oct / 10) % 10) as char);
        let _ = line.push((b'0' + oct % 10) as char);
    }
}

/// Append a 4-digit decimal (leading zeros).
pub fn write_u16_padded(line: &mut Line, n: u16) {
    let _ = line.push((b'0' + ((n / 1000) % 10) as u8) as char);
    let _ = line.push((b'0' + ((n / 100) % 10) as u8) as char);
    let _ = line.push((b'0' + ((n / 10) % 10) as u8) as char);
    let _ = line.push((b'0' + (n % 10) as u8) as char);
}

/// Append `aa:bb:cc:dd:ee:ff`.
pub fn write_mac(line: &mut Line, mac: [u8; 6]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (i, b) in mac.iter().enumerate() {
        if i > 0 {
            let _ = line.push(':');
        }
        let _ = line.push(HEX[(b >> 4) as usize] as char);
        let _ = line.push(HEX[(b & 0x0f) as usize] as char);
    }
}

/// Append a 3-digit decimal octet (leading zeros).
pub fn push_ip_octet(buf: &mut heapless::String<17>, oct: u8) {
    let _ = buf.push((b'0' + oct / 100) as char);
    let _ = buf.push((b'0' + (oct / 10) % 10) as char);
    let _ = buf.push((b'0' + oct % 10) as char);
}

/// Append a 5-digit port (leading zeros).
pub fn push_port_digits(buf: &mut heapless::String<17>, port: u16) {
    let _ = buf.push((b'0' + ((port / 10000) % 10) as u8) as char);
    let _ = buf.push((b'0' + ((port / 1000) % 10) as u8) as char);
    let _ = buf.push((b'0' + ((port / 100) % 10) as u8) as char);
    let _ = buf.push((b'0' + ((port / 10) % 10) as u8) as char);
    let _ = buf.push((b'0' + (port % 10) as u8) as char);
}

/// Parse 12 digit characters as four 3-digit octets.
pub fn parse_ip_digits(digits: &str) -> Option<[u8; 4]> {
    if digits.len() != 12 {
        return None;
    }
    let oct = |s: &str| s.parse::<u8>().ok();
    Some([
        oct(digits.get(0..3)?)?,
        oct(digits.get(3..6)?)?,
        oct(digits.get(6..9)?)?,
        oct(digits.get(9..12)?)?,
    ])
}

/// Write four zero-padded octets into a 12-char digit buffer.
pub fn push_ip_digits(buf: &mut heapless::String<12>, ip: [u8; 4]) {
    buf.clear();
    for o in ip {
        let _ = buf.push((b'0' + o / 100) as char);
        let _ = buf.push((b'0' + (o / 10) % 10) as char);
        let _ = buf.push((b'0' + o % 10) as char);
    }
}

/// Default `aaabbbcccdddppppp` digits for WIT or Z21 manual entry.
pub fn default_server_digits(cx: &ScreenCtx<'_>, z21: bool) -> heapless::String<17> {
    let (ip, port) = if z21 {
        (cx.env.default_z21_ip, cx.env.default_z21_port)
    } else {
        (cx.env.default_wit_ip, cx.env.default_wit_port)
    };
    let mut buf = heapless::String::new();
    for oct in ip {
        push_ip_octet(&mut buf, oct);
    }
    push_port_digits(&mut buf, port);
    buf
}

/// SSIDs compiled into the firmware (`NETWORKS`).
pub fn compiled_ssids(cx: &ScreenCtx<'_>) -> heapless::Vec<&'static str, 16> {
    let mut v = heapless::Vec::new();
    for n in cx.env.compiled_networks {
        if v.push(n.ssid).is_err() {
            break;
        }
    }
    v
}

/// Password from NVS, else compiled network, else empty.
pub fn password_for_ssid<'a>(cx: &'a ScreenCtx<'_>, ssid: &str) -> &'a str {
    cx.drive
        .persist
        .find_password(ssid)
        .or_else(|| {
            cx.env
                .compiled_networks
                .iter()
                .find(|n| n.ssid == ssid)
                .map(|n| n.password)
        })
        .unwrap_or("")
}

/// Load the digit buffer for one IP-config field (DHCP/IP/mask/GW/DNS).
pub fn load_net_field_digits(cfg: &StaticIpConfig, field: u8) -> heapless::String<12> {
    let mut digits = heapless::String::new();
    match field {
        0 => {
            let _ = digits.push(if cfg.dhcp { '0' } else { '1' });
        }
        1 => push_ip_digits(&mut digits, cfg.ip),
        2 => {
            let _ = digits.push((b'0' + cfg.prefix_len / 10) as char);
            let _ = digits.push((b'0' + cfg.prefix_len % 10) as char);
        }
        3 => {
            if let Some(gw) = cfg.gateway {
                push_ip_digits(&mut digits, gw);
            }
        }
        4 => {
            if let Some(dns) = cfg.dns {
                push_ip_digits(&mut digits, dns);
            }
        }
        _ => {}
    }
    digits
}

/// Max digit length for an IP-config field.
pub fn net_field_max_len(field: u8) -> usize {
    match field {
        0 => 1,
        2 => 2,
        _ => 12,
    }
}

/// Write a field's digits back into `cfg`. IP also auto-fills prefix/gateway.
pub fn commit_net_field(cfg: &mut StaticIpConfig, field: u8, digits: &str, default_prefix: u8) {
    match field {
        0 => {
            cfg.dhcp = digits.as_bytes().first() != Some(&b'1');
        }
        1 => {
            if let Some(ip) = parse_ip_digits(digits) {
                cfg.ip = ip;
                auto_fill_from_ip(cfg, default_prefix);
            }
        }
        2 => {
            if digits.is_empty() {
                cfg.prefix_len = default_prefix;
            } else if digits.len() <= 2 {
                let mut prefix = 0u8;
                for b in digits.as_bytes() {
                    let Some(d) = (*b as char).to_digit(10) else {
                        return;
                    };
                    prefix = prefix.saturating_mul(10).saturating_add(d as u8);
                }
                if prefix <= 32 {
                    cfg.prefix_len = prefix;
                }
            }
        }
        3 => {
            cfg.gateway = if digits.is_empty() {
                None
            } else {
                parse_ip_digits(digits)
            };
        }
        4 => {
            cfg.dns = if digits.is_empty() {
                None
            } else {
                parse_ip_digits(digits)
            };
        }
        _ => {}
    }
}

/// If prefix/gateway are unset after typing an IP, fill `.1` gateway and default prefix.
pub fn auto_fill_from_ip(cfg: &mut StaticIpConfig, default_prefix: u8) {
    if cfg.prefix_len == 0 {
        cfg.prefix_len = default_prefix;
    }
    if cfg.gateway.is_none() {
        let mut gw = cfg.ip;
        gw[3] = 1;
        cfg.gateway = Some(gw);
    }
}

/// Store the chosen SSID; clear a stale password if the SSID changed.
pub fn pick_ssid(cx: &mut ScreenCtx<'_>, ssid: &str) {
    if cx.session.selected_ssid.as_str() != ssid {
        cx.session.password.clear();
    }
    cx.session.selected_ssid.clear();
    let _ = cx.session.selected_ssid.push_str(ssid);
}

#[cfg(test)]
mod tests {
    use super::*;
    use longfred_proto::persist::StaticIpConfig;

    #[test]
    fn commit_prefix_ignores_non_digits() {
        let mut cfg = StaticIpConfig {
            prefix_len: 24,
            ..StaticIpConfig::default()
        };
        commit_net_field(&mut cfg, 2, "x", 16);
        assert_eq!(cfg.prefix_len, 24);
    }

    #[test]
    fn commit_prefix_accepts_two_digits() {
        let mut cfg = StaticIpConfig::default();
        commit_net_field(&mut cfg, 2, "24", 16);
        assert_eq!(cfg.prefix_len, 24);
    }

    #[test]
    fn commit_prefix_rejects_over_32() {
        let mut cfg = StaticIpConfig {
            prefix_len: 24,
            ..StaticIpConfig::default()
        };
        commit_net_field(&mut cfg, 2, "33", 16);
        assert_eq!(cfg.prefix_len, 24);
    }

    #[test]
    fn parse_ip_digits_rejects_wrong_len() {
        assert!(parse_ip_digits("999").is_none());
        assert_eq!(parse_ip_digits("192168001001"), Some([192, 168, 1, 1]));
    }
}
