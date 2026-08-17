//! Minimal mDNS client: PTR query builder + response parser (host-testable).
//! Socket I/O lives in firmware (`net/mdns.rs`).

use crate::command::Protocol;

/// Advertised while STA HTTP OTA is enabled.
pub const OTA_HTTP_SERVICE: &str = "_longfred-ota._tcp.local";
pub const MDNS_MULTICAST_V4: [u8; 4] = [224, 0, 0, 251];
pub const MDNS_PORT: u16 = 5353;

const TYPE_A: u16 = 1;
const TYPE_TXT: u16 = 16;
const TYPE_SRV: u16 = 33;

/// Dotted host / service name, e.g. `JMRI._withrottle._tcp.local`.
pub type Name = heapless::String<128>;

/// Discovered command station from SRV (+A) records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitServer {
    /// First instance label (e.g. "JMRI") for display.
    pub label: heapless::String<32>,
    pub port: u16,
    pub ipv4: Option<[u8; 4]>,
    pub protocol: Protocol,
}

/// Builds a PTR query for the given service name. Returns bytes written to `buf`.
pub fn build_ptr_query(service: &str, buf: &mut [u8]) -> usize {
    let mut n = 0usize;
    let mut put = |b: u8| {
        if n < buf.len() {
            buf[n] = b;
        }
        n += 1;
    };
    for b in [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0] {
        put(b);
    }
    for label in service.split('.') {
        put(label.len() as u8);
        for &c in label.as_bytes() {
            put(c);
        }
    }
    put(0);
    for b in [0x00, 0x0c, 0x00, 0x01] {
        put(b);
    }
    n.min(buf.len())
}

fn be16(pkt: &[u8], off: usize) -> Option<u16> {
    Some(((*pkt.get(off)? as u16) << 8) | (*pkt.get(off + 1)? as u16))
}

fn first_label(name: &str) -> heapless::String<32> {
    let mut label = heapless::String::<32>::new();
    for c in name.split('.').next().unwrap_or("").chars() {
        let _ = label.push(c);
    }
    label
}

/// `owner` is `{instance}.{service}` (DNS labels are case-insensitive).
fn is_service_instance(owner: &str, service: &str) -> bool {
    let Some(split) = owner.len().checked_sub(service.len()) else {
        return false;
    };
    if split < 2 {
        return false;
    }
    if owner.as_bytes().get(split - 1) != Some(&b'.') {
        return false;
    }
    let instance = &owner[..split - 1];
    let suffix = &owner[split..];
    !instance.is_empty() && suffix.eq_ignore_ascii_case(service)
}

/// TXT strings used by BigFred's `microdns` (`layoutId=` + `commandStationId=`).
fn txt_marks_bigfred(rdata: &[u8]) -> bool {
    let mut has_layout = false;
    let mut has_station = false;
    let mut i = 0usize;
    while i < rdata.len() {
        let n = usize::from(rdata[i]);
        i += 1;
        if i.saturating_add(n) > rdata.len() {
            break;
        }
        let chunk = &rdata[i..i + n];
        i += n;
        if chunk.starts_with(b"layoutId=") {
            has_layout = true;
        }
        if chunk.starts_with(b"commandStationId=") {
            has_station = true;
        }
    }
    has_layout && has_station
}

fn read_name(pkt: &[u8], start: usize) -> Option<(Name, usize)> {
    let mut name = Name::new();
    let mut off = start;
    let mut next_after: Option<usize> = None;
    let mut jumps = 0usize;
    loop {
        let len = *pkt.get(off)?;
        if len == 0 {
            off += 1;
            break;
        }
        if len & 0xc0 == 0xc0 {
            let ptr = (((len & 0x3f) as usize) << 8) | *pkt.get(off + 1)? as usize;
            if next_after.is_none() {
                next_after = Some(off + 2);
            }
            jumps += 1;
            if jumps > 16 {
                return None;
            }
            off = ptr;
            continue;
        }
        let l = len as usize;
        if !name.is_empty() {
            let _ = name.push('.');
        }
        for i in 0..l {
            let _ = name.push(*pkt.get(off + 1 + i)? as char);
        }
        off += 1 + l;
    }
    Some((name, next_after.unwrap_or(off)))
}

/// Parse a response and correlate servers. Returns up to N entries tagged with `protocol`.
pub fn collect_servers<const N: usize>(
    pkt: &[u8],
    protocol: Protocol,
) -> heapless::Vec<WitServer, N> {
    let service = protocol.caps().mdns_service;
    let mut servers: heapless::Vec<WitServer, N> = heapless::Vec::new();
    let mut addrs: heapless::Vec<(Name, [u8; 4]), N> = heapless::Vec::new();
    let mut srvs: heapless::Vec<(Name, u16, heapless::String<32>), N> = heapless::Vec::new();
    let mut txt_bigfred: heapless::Vec<heapless::String<32>, N> = heapless::Vec::new();

    let qd = match be16(pkt, 4) {
        Some(v) => v,
        None => return servers,
    };
    let an = be16(pkt, 6).unwrap_or(0);
    let ns = be16(pkt, 8).unwrap_or(0);
    let ar = be16(pkt, 10).unwrap_or(0);
    let total = an as usize + ns as usize + ar as usize;

    let mut off = 12;
    for _ in 0..qd {
        let (_, next) = match read_name(pkt, off) {
            Some(v) => v,
            None => return servers,
        };
        off = next + 4;
    }

    for _ in 0..total {
        let (owner, next) = match read_name(pkt, off) {
            Some(v) => v,
            None => break,
        };
        let rtype = match be16(pkt, next) {
            Some(v) => v,
            None => break,
        };
        let rdlen = match be16(pkt, next + 8) {
            Some(v) => v as usize,
            None => break,
        };
        let rdata = next + 10;
        if rdata + rdlen > pkt.len() {
            break;
        }

        match rtype {
            TYPE_SRV => {
                if is_service_instance(owner.as_str(), service)
                    && let Some(port) = be16(pkt, rdata + 4)
                    && let Some((target, _)) = read_name(pkt, rdata + 6)
                {
                    let _ = srvs.push((target, port, first_label(owner.as_str())));
                }
            }
            TYPE_TXT => {
                if is_service_instance(owner.as_str(), service)
                    && txt_marks_bigfred(&pkt[rdata..rdata + rdlen])
                {
                    let label = first_label(owner.as_str());
                    if !label.is_empty() {
                        let _ = txt_bigfred.push(label);
                    }
                }
            }
            TYPE_A => {
                if rdlen >= 4 {
                    let ip = [pkt[rdata], pkt[rdata + 1], pkt[rdata + 2], pkt[rdata + 3]];
                    let _ = addrs.push((owner, ip));
                }
            }
            _ => {}
        }
        off = rdata + rdlen;
    }

    for (target, port, label) in srvs {
        let ipv4 = addrs.iter().find(|(n, _)| *n == target).map(|(_, ip)| *ip);
        let tagged = if protocol == Protocol::WiThrottle && txt_bigfred.iter().any(|l| l == &label)
        {
            Protocol::BigFred
        } else {
            protocol
        };
        let _ = servers.push(WitServer {
            label,
            port,
            ipv4,
            protocol: tagged,
        });
    }
    servers
}

fn mdns_put_byte(buf: &mut [u8], n: &mut usize, b: u8) {
    if *n < buf.len() {
        buf[*n] = b;
    }
    *n += 1;
}

fn mdns_put_slice(buf: &mut [u8], n: &mut usize, s: &[u8]) {
    for &b in s {
        mdns_put_byte(buf, n, b);
    }
}

fn mdns_put_name(buf: &mut [u8], n: &mut usize, labels: &[&str]) {
    for lab in labels {
        mdns_put_byte(buf, n, lab.len() as u8);
        mdns_put_slice(buf, n, lab.as_bytes());
    }
    mdns_put_byte(buf, n, 0);
}

/// Unsolicited mDNS announcement for `_longfred-ota._tcp` (PTR + SRV + A).
pub fn build_ota_announce(hostname: &str, ipv4: [u8; 4], port: u16, buf: &mut [u8]) -> usize {
    let mut n = 0usize;

    // Header: response, authoritative, 0 questions, 3 answers.
    mdns_put_slice(buf, &mut n, &[0, 0, 0x84, 0, 0, 0, 0, 3, 0, 0, 0, 0]);

    // PTR _longfred-ota._tcp.local -> {hostname}._longfred-ota._tcp.local
    mdns_put_name(buf, &mut n, &["_longfred-ota", "_tcp", "local"]);
    mdns_put_slice(buf, &mut n, &[0, 12, 0, 1, 0, 0, 0, 120]);
    let instance_len =
        1 + hostname.len() + 1 + "_longfred-ota".len() + 1 + "_tcp".len() + 1 + "local".len() + 1;
    mdns_put_slice(
        buf,
        &mut n,
        &u16::try_from(instance_len).unwrap_or(0).to_be_bytes(),
    );
    mdns_put_name(buf, &mut n, &[hostname, "_longfred-ota", "_tcp", "local"]);

    // SRV {hostname}._longfred-ota._tcp.local -> {hostname}.local:port
    mdns_put_name(buf, &mut n, &[hostname, "_longfred-ota", "_tcp", "local"]);
    mdns_put_slice(buf, &mut n, &[0, 33, 0, 1, 0, 0, 0, 120]);
    let target_len = 6 + 1 + hostname.len() + 1 + "local".len() + 1;
    mdns_put_slice(
        buf,
        &mut n,
        &u16::try_from(target_len).unwrap_or(0).to_be_bytes(),
    );
    mdns_put_slice(buf, &mut n, &[0, 0, 0, 0]);
    mdns_put_slice(buf, &mut n, &port.to_be_bytes());
    mdns_put_name(buf, &mut n, &[hostname, "local"]);

    // A {hostname}.local
    mdns_put_name(buf, &mut n, &[hostname, "local"]);
    mdns_put_slice(buf, &mut n, &[0, 1, 0, 1, 0, 0, 0, 120, 0, 4]);
    mdns_put_slice(buf, &mut n, &ipv4);

    n.min(buf.len())
}

/// Hosts advertising `_longfred-ota._tcp` (A records in a response).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtaHost {
    pub hostname: heapless::String<32>,
    pub ipv4: [u8; 4],
    pub port: u16,
}

/// Collect A records from an mDNS packet (used by LAN firmware discovery).
pub fn collect_ota_hosts<const N: usize>(pkt: &[u8]) -> heapless::Vec<OtaHost, N> {
    let mut out: heapless::Vec<OtaHost, N> = heapless::Vec::new();
    if pkt.len() < 12 {
        return out;
    }
    let an = be16(pkt, 6).unwrap_or(0);
    let ns = be16(pkt, 8).unwrap_or(0);
    let ar = be16(pkt, 10).unwrap_or(0);
    let mut off = 12usize;
    let mut port = 80u16;
    for _ in 0..an.saturating_add(ns).saturating_add(ar) {
        let Some((name, nend)) = read_name(pkt, off) else {
            break;
        };
        off = nend;
        let Some(typ) = be16(pkt, off) else { break };
        off += 8;
        let Some(rdlen) = be16(pkt, off) else { break };
        off += 2;
        let rdata = off;
        off = off.saturating_add(rdlen as usize);
        if typ == TYPE_SRV && rdlen >= 6 {
            if let Some(p) = be16(pkt, rdata + 4) {
                port = p;
            }
        }
        if typ == TYPE_A && rdlen == 4 {
            if let (Some(a), Some(b), Some(c), Some(d)) = (
                pkt.get(rdata),
                pkt.get(rdata + 1),
                pkt.get(rdata + 2),
                pkt.get(rdata + 3),
            ) {
                let host = name.split('.').next().unwrap_or("longfred");
                let mut hostname = heapless::String::new();
                let _ = hostname.push_str(host);
                let _ = out.push(OtaHost {
                    hostname,
                    ipv4: [*a, *b, *c, *d],
                    port,
                });
            }
        }
        if out.is_full() {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ptr_query_lengths() {
        let mut buf = [0u8; 64];
        let w = build_ptr_query(crate::withrottle::MDNS_SERVICE, &mut buf);
        let z = build_ptr_query(crate::z21::MDNS_SERVICE, &mut buf);
        assert!(w > 12);
        assert!(z > 12);
        assert_ne!(w, z);
    }

    #[test]
    fn ota_announce_roundtrip_a_record() {
        let mut buf = [0u8; 512];
        let n = build_ota_announce("longred_ab12cd", [192, 168, 1, 40], 80, &mut buf);
        assert!(n > 40);
        let hosts = collect_ota_hosts::<4>(&buf[..n]);
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].ipv4, [192, 168, 1, 40]);
        assert_eq!(hosts[0].port, 80);
        assert_eq!(hosts[0].hostname.as_str(), "longred_ab12cd");
    }
}
