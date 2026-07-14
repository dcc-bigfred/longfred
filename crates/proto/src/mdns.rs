//! Minimalny klient mDNS: builder zapytania PTR + parser odpowiedzi (host-testowalny).
//! I/O (UdpSocket, multicast) jest w firmware (`net/mdns.rs`).

pub const WITHROTTLE_SERVICE: &str = "_withrottle._tcp.local";
pub const MDNS_MULTICAST_V4: [u8; 4] = [224, 0, 0, 251];
pub const MDNS_PORT: u16 = 5353;

const TYPE_A: u16 = 1;
const TYPE_SRV: u16 = 33;

/// Nazwa hosta/usługi w formacie kropkowym, np. `JMRI._withrottle._tcp.local`.
pub type Name = heapless::String<128>;

/// Serwer WiThrottle skorelowany z rekordów SRV (+A).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitServer {
    /// Pierwsza etykieta instancji (np. "JMRI") do wyświetlenia.
    pub label: heapless::String<32>,
    pub port: u16,
    pub ipv4: Option<[u8; 4]>,
}

/// Buduje zapytanie PTR o `_withrottle._tcp.local`. Zwraca długość zapisana do `buf`.
pub fn build_ptr_query(buf: &mut [u8]) -> usize {
    let mut n = 0usize;
    let mut put = |b: u8| {
        if n < buf.len() {
            buf[n] = b;
        }
        n += 1;
    };
    // Header: ID=0, flags=0, QD=1, AN=NS=AR=0.
    for b in [0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0] {
        put(b);
    }
    for label in WITHROTTLE_SERVICE.split('.') {
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

/// Czyta nazwę DNS (z obsługą kompresji). Zwraca (nazwa, offset za nazwą w strumieniu).
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

/// Parsuje odpowiedź i koreluje serwery. Zwraca listę (max N).
pub fn collect_servers<const N: usize>(pkt: &[u8]) -> heapless::Vec<WitServer, N> {
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
        let _ = servers.push(WitServer { label, port, ipv4 });
    }
    servers
}
