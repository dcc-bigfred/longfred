//! Minimal mDNS client: PTR query builder + response parser (host-testable).
//! Socket I/O lives in firmware (`net/mdns.rs`).

use crate::command::Protocol;

pub const WITHROTTLE_SERVICE: &str = "_withrottle._tcp.local";
pub const Z21_SERVICE: &str = "_z21._udp.local";
pub const MDNS_MULTICAST_V4: [u8; 4] = [224, 0, 0, 251];
pub const MDNS_PORT: u16 = 5353;

const TYPE_A: u16 = 1;
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
    let mut servers: heapless::Vec<WitServer, N> = heapless::Vec::new();
    let mut addrs: heapless::Vec<(Name, [u8; 4]), N> = heapless::Vec::new();
    let mut srvs: heapless::Vec<(Name, u16, heapless::String<32>), N> = heapless::Vec::new();

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
                if let Some(port) = be16(pkt, rdata + 4) {
                    if let Some((target, _)) = read_name(pkt, rdata + 6) {
                        let mut label = heapless::String::<32>::new();
                        for c in owner.split('.').next().unwrap_or("").chars() {
                            let _ = label.push(c);
                        }
                        let _ = srvs.push((target, port, label));
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
        let _ = servers.push(WitServer {
            label,
            port,
            ipv4,
            protocol,
        });
    }
    servers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ptr_query_lengths() {
        let mut buf = [0u8; 64];
        let w = build_ptr_query(WITHROTTLE_SERVICE, &mut buf);
        let z = build_ptr_query(Z21_SERVICE, &mut buf);
        assert!(w > 12);
        assert!(z > 12);
        assert_ne!(w, z);
    }
}
