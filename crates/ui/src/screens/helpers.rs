//! Shared list / IP helpers used by multiple screens.

use core::fmt::Write as _;

use longfred_proto::command::Protocol;
use longfred_proto::network::WitServer;
use longfred_proto::persist::StaticIpConfig;

use crate::context::{MAX_COMPILED_NETWORKS, ScreenCtx};
use crate::nav::{PageDir, Step};
use crate::session::NetField;
use crate::view::{GridView, LINE_LEN, Line, push_oled};
use crate::widgets::PagedList;

/// Keypad digit `0..=9` from a character. Non-digits yield `None`.
#[must_use]
pub fn digit_key(c: char) -> Option<u8> {
    c.to_digit(10).and_then(|n| u8::try_from(n).ok())
}

/// Leading shortcut digit from a menu label (`"6 Language"` → `6`, `"0-4 Fn"` → `0`).
#[must_use]
pub fn label_shortcut_digit(label: &str) -> Option<u8> {
    let trimmed = label.trim_start();
    let Some(c) = trimmed.chars().next() else {
        return None;
    };
    if !c.is_ascii_digit() {
        return None;
    }
    c.to_digit(10).and_then(|d| u8::try_from(d).ok())
}

/// True when the current throttle slot has an acquired loco.
pub fn has_loco(cx: &ScreenCtx<'_>) -> bool {
    cx.drive
        .slots
        .get(cx.drive.current)
        .is_some_and(longfred_proto::model::ThrottleSlot::has_loco)
}

/// Next encoder step multiplier (`1 → 2 → 4 → 1`), matching firmware.
#[must_use]
pub fn next_speed_multiplier(current: u8) -> u8 {
    match current {
        1 => 2,
        2 => 4,
        _ => 1,
    }
}

/// `prefix` plus a small decimal (overlay status lines).
#[must_use]
pub fn overlay_prefixed_count(prefix: &str, n: usize) -> heapless::String<64> {
    let mut s = heapless::String::new();
    let _ = s.push_str(prefix);
    let _ = write!(s, "{n}");
    s
}

/// Overlay body: `prefix` `{n}` `suffix`.
#[must_use]
pub fn overlay_count_message(prefix: &str, n: usize, suffix: &str) -> heapless::String<64> {
    let mut s = heapless::String::new();
    let _ = s.push_str(prefix);
    let _ = write!(s, "{n}");
    let _ = s.push_str(suffix);
    s
}

/// OLED height in pixels (paging).
pub fn height(cx: &ScreenCtx<'_>) -> u16 {
    cx.env.geometry.height
}

/// Move the list cursor one row.
pub fn step_list(list: &mut PagedList, d: Step, items: &[&str], h: u16) {
    match d {
        Step::Prev => list.list_prev(items, h),
        Step::Next => list.list_next(items, h),
    }
}

/// Flip one page of a paged list.
pub fn page_list(list: &mut PagedList, d: PageDir, items: &[&str], h: u16) {
    match d {
        PageDir::Prev => list.page_prev(items, h),
        PageDir::Next => list.page_next(items, h),
    }
}

/// Digit: append in `*` mode, otherwise numbered-row shortcut.
pub fn list_digit(list: &mut PagedList, d: u8, items: &[&str], h: u16) -> Option<usize> {
    if list.buffer_digit(d, items, h) {
        return None;
    }
    list.select_digit(d, items, h)
}

/// Digit: append in `*` mode, otherwise leading-label shortcut.
pub fn list_label_digit(list: &mut PagedList, d: u8, items: &[&str], h: u16) -> Option<usize> {
    if list.buffer_digit(d, items, h) {
        return None;
    }
    list.select_label_digit(d, items, h)
}

/// `*` confirmed a row (caller should activate the focused item).
pub fn list_star_confirms(list: &mut PagedList, items: &[&str], h: u16) -> bool {
    matches!(list.star(items, h), crate::widgets::StarIndex::Confirm(_))
}

/// Hint on the last visible content row (6 on 128×64, 3 on 128×32).
pub fn set_list_hint(g: &mut GridView, cx: &ScreenCtx<'_>, hint: &str) {
    g.set(
        crate::view::list_hint_row(cx.env.geometry.height),
        hint,
        false,
    );
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

/// ASCII digit `0..=9` from the least significant decimal digit of `n`.
fn dec_digit(n: u16) -> char {
    char::from(b'0' + u8::try_from(n % 10).unwrap_or(0))
}

/// Append a decimal `u16` without leading zeros.
pub fn write_u16(line: &mut Line, n: u16) {
    if n >= 10000 {
        let _ = line.push(dec_digit(n / 10000));
    }
    if n >= 1000 {
        let _ = line.push(dec_digit(n / 1000));
    }
    if n >= 100 {
        let _ = line.push(dec_digit(n / 100));
    }
    if n >= 10 {
        let _ = line.push(dec_digit(n / 10));
    }
    let _ = line.push(dec_digit(n));
}

/// Append `a.b.c.d` without zero-padding.
pub fn write_ip_compact(line: &mut Line, ip: [u8; 4]) {
    for (i, oct) in ip.iter().enumerate() {
        if i > 0 {
            let _ = line.push('.');
        }
        write_u16(line, u16::from(*oct));
    }
}

/// Append a 4-digit decimal (leading zeros).
pub fn write_u16_padded(line: &mut Line, n: u16) {
    let _ = line.push(dec_digit(n / 1000));
    let _ = line.push(dec_digit(n / 100));
    let _ = line.push(dec_digit(n / 10));
    let _ = line.push(dec_digit(n));
}

/// Protocol mark after `{layoutName}/BigFred`.
#[must_use]
pub fn layout_protocol_mark(protocol: Protocol) -> &'static str {
    match protocol {
        Protocol::BigFred => "B",
        Protocol::Z21 => "Z21",
        Protocol::WiThrottle => "W",
    }
}

/// List / confirm label: `{layoutName}/BigFred B|Z21` or `{label} W|Z`.
#[must_use]
pub fn format_found_server_name(s: &WitServer) -> Line {
    let mut line = Line::new();
    if s.layout_name.is_empty() {
        let mut name = Line::new();
        push_oled(&mut name, s.label.as_str());
        while name.len() > LINE_LEN.saturating_sub(2) {
            let _ = name.pop();
        }
        push_oled(&mut line, name.as_str());
        let _ = line.push(' ');
        let _ = line.push(s.protocol.glyph());
    } else {
        let mark = layout_protocol_mark(s.protocol);
        let budget = LINE_LEN.saturating_sub(1 + "BigFred".len() + 1 + mark.len());
        let mut name = Line::new();
        push_oled(&mut name, s.layout_name.as_str());
        while name.len() > budget {
            let _ = name.pop();
        }
        push_oled(&mut line, name.as_str());
        let _ = line.push('/');
        push_oled(&mut line, "BigFred");
        let _ = line.push(' ');
        push_oled(&mut line, mark);
    }
    line
}

/// Host shown on the confirm screen: DNS name if the SRV target is known, else IPv4.
#[must_use]
pub fn format_found_server_addr(s: &WitServer) -> Line {
    let mut line = Line::new();
    if !s.host.is_empty() {
        push_oled(&mut line, s.host.as_str());
        if !line.as_str().contains('.') {
            push_oled(&mut line, ".local");
        }
    } else if let Some(ip) = s.ipv4 {
        write_ip_compact(&mut line, ip);
    } else {
        return line;
    }
    if s.port != 0 && line.len() < LINE_LEN.saturating_sub(2) {
        let _ = line.push(':');
        write_u16(&mut line, s.port);
    }
    line
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
    let _ = buf.push(dec_digit(port / 10000));
    let _ = buf.push(dec_digit(port / 1000));
    let _ = buf.push(dec_digit(port / 100));
    let _ = buf.push(dec_digit(port / 10));
    let _ = buf.push(dec_digit(port));
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
pub fn compiled_ssids(cx: &ScreenCtx<'_>) -> heapless::Vec<&'static str, MAX_COMPILED_NETWORKS> {
    debug_assert!(
        cx.env.compiled_networks.len() <= MAX_COMPILED_NETWORKS,
        "compiled NETWORKS exceeds MAX_COMPILED_NETWORKS"
    );
    let mut v = heapless::Vec::new();
    for n in cx.env.compiled_networks.iter().take(MAX_COMPILED_NETWORKS) {
        let _ = v.push(n.ssid);
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
pub fn load_net_field_digits(cfg: &StaticIpConfig, field: NetField) -> heapless::String<12> {
    let mut digits = heapless::String::new();
    match field {
        NetField::Dhcp => {
            let _ = digits.push(if cfg.dhcp { '0' } else { '1' });
        }
        NetField::Ip => push_ip_digits(&mut digits, cfg.ip),
        NetField::Prefix => {
            let _ = digits.push((b'0' + cfg.prefix_len / 10) as char);
            let _ = digits.push((b'0' + cfg.prefix_len % 10) as char);
        }
        NetField::Gateway => {
            if let Some(gw) = cfg.gateway {
                push_ip_digits(&mut digits, gw);
            }
        }
        NetField::Dns => {
            if let Some(dns) = cfg.dns {
                push_ip_digits(&mut digits, dns);
            }
        }
    }
    digits
}

/// Write a field's digits back into `cfg`. IP also auto-fills prefix/gateway.
pub fn commit_net_field(
    cfg: &mut StaticIpConfig,
    field: NetField,
    digits: &str,
    default_prefix: u8,
) {
    match field {
        NetField::Dhcp => {
            cfg.dhcp = digits.as_bytes().first() != Some(&b'1');
        }
        NetField::Ip => {
            if let Some(ip) = parse_ip_digits(digits) {
                cfg.ip = ip;
                auto_fill_from_ip(cfg, default_prefix);
            }
        }
        NetField::Prefix => {
            if digits.is_empty() {
                cfg.prefix_len = default_prefix;
            } else if digits.len() <= 2 {
                let mut prefix = 0u8;
                for b in digits.as_bytes() {
                    let Some(digit) = digit_key(*b as char) else {
                        return;
                    };
                    prefix = prefix.saturating_mul(10).saturating_add(digit);
                }
                if prefix <= 32 {
                    cfg.prefix_len = prefix;
                }
            }
        }
        NetField::Gateway => {
            cfg.gateway = if digits.is_empty() {
                None
            } else {
                parse_ip_digits(digits)
            };
        }
        NetField::Dns => {
            cfg.dns = if digits.is_empty() {
                None
            } else {
                parse_ip_digits(digits)
            };
        }
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
    fn label_shortcut_digit_reads_leading_key() {
        assert_eq!(label_shortcut_digit("6 Jezyk"), Some(6));
        assert_eq!(label_shortcut_digit("0-4 Funkcja"), Some(0));
        assert_eq!(label_shortcut_digit("Firmware update"), None);
    }

    #[test]
    fn commit_prefix_ignores_non_digits() {
        let mut cfg = StaticIpConfig {
            prefix_len: 24,
            ..StaticIpConfig::default()
        };
        commit_net_field(&mut cfg, NetField::Prefix, "x", 16);
        assert_eq!(cfg.prefix_len, 24);
    }

    #[test]
    fn commit_prefix_accepts_two_digits() {
        let mut cfg = StaticIpConfig::default();
        commit_net_field(&mut cfg, NetField::Prefix, "24", 16);
        assert_eq!(cfg.prefix_len, 24);
    }

    #[test]
    fn commit_prefix_rejects_over_32() {
        let mut cfg = StaticIpConfig {
            prefix_len: 24,
            ..StaticIpConfig::default()
        };
        commit_net_field(&mut cfg, NetField::Prefix, "33", 16);
        assert_eq!(cfg.prefix_len, 24);
    }

    #[test]
    fn parse_ip_digits_rejects_wrong_len() {
        assert!(parse_ip_digits("999").is_none());
        assert_eq!(parse_ip_digits("192168001001"), Some([192, 168, 1, 1]));
    }

    #[test]
    fn parse_ip_digits_rejects_out_of_range_octet() {
        assert!(parse_ip_digits("999000000000").is_none());
        assert_eq!(parse_ip_digits("255255255255"), Some([255, 255, 255, 255]));
    }

    #[test]
    fn next_speed_multiplier_cycles_1_2_4() {
        assert_eq!(next_speed_multiplier(1), 2);
        assert_eq!(next_speed_multiplier(2), 4);
        assert_eq!(next_speed_multiplier(4), 1);
        assert_eq!(next_speed_multiplier(0), 1);
    }

    #[test]
    fn overlay_count_message_inserts_n() {
        assert_eq!(
            overlay_count_message("among ", 3, " locos").as_str(),
            "among 3 locos"
        );
    }
}
