//! NVS persistence record serialization (host-testable).

pub const MAGIC: u32 = 0x4C46_5031; // "LFP1"
pub const VERSION: u16 = 3;
pub const MAX_CREDENTIALS: usize = 8;
pub const MAX_SAVED_LOCOS: usize = 12;
pub const MAX_DEVICE_NAME_LEN: usize = 32;
pub const MAX_WIFI_HOSTNAME_LEN: usize = 16;
pub const WIFI_HOSTNAME_PREFIX: &str = "longred_";
pub const WIFI_HOSTNAME_SUFFIX_LEN: usize = 6;
pub const DEVICE_ID_MIN: u16 = 1000;
pub const DEVICE_ID_MAX: u16 = 9999;

const TAG_CRED: u8 = 1;
const TAG_LOCO: u8 = 2;
const TAG_NET: u8 = 3;
const TAG_DEV: u8 = 4;
const TAG_HOST: u8 = 5;
const TAG_LANG: u8 = 6;

/// UI language (stored in NVS).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Language {
    #[default]
    En = 0,
    Pl = 1,
    De = 2,
}

impl Language {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::En),
            1 => Some(Self::Pl),
            2 => Some(Self::De),
            _ => None,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Credential {
    pub ssid: heapless::String<32>,
    pub password: heapless::String<64>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SavedLoco {
    pub throttle: u8,
    pub slot: u8,
    pub addr: heapless::String<8>,
}

/// Client IPv4 configuration (DHCP or static).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StaticIpConfig {
    pub dhcp: bool,
    pub ip: [u8; 4],
    pub prefix_len: u8,
    pub gateway: Option<[u8; 4]>,
    pub dns: Option<[u8; 4]>,
}

impl Default for StaticIpConfig {
    fn default() -> Self {
        Self {
            dhcp: true,
            ip: [0, 0, 0, 0],
            prefix_len: 24,
            gateway: None,
            dns: None,
        }
    }
}

/// WiThrottle client identity (`N{name}` / `HU{id}`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DeviceIdentity {
    pub name: heapless::String<MAX_DEVICE_NAME_LEN>,
    /// `0` = not yet assigned (generate on first boot).
    pub id: u16,
}

impl Default for DeviceIdentity {
    fn default() -> Self {
        let mut name = heapless::String::new();
        let _ = name.push_str("LongFred");
        Self { name, id: 0 }
    }
}

impl DeviceIdentity {
    pub const fn empty() -> Self {
        Self {
            name: heapless::String::new(),
            id: 0,
        }
    }

    pub fn id_wire(&self) -> heapless::String<8> {
        let mut s = heapless::String::new();
        let _ = push_u16(&mut s, self.id);
        s
    }
}

/// Map hardware RNG output to WiThrottle client id range (1000..=9999).
pub fn id_from_entropy(entropy: u32) -> u16 {
    let span = (DEVICE_ID_MAX - DEVICE_ID_MIN + 1) as u32;
    DEVICE_ID_MIN + (entropy % span) as u16
}

const WIFI_HOSTNAME_ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";

/// DHCP client hostname (`longred_` + 6 alphanumeric chars).
pub fn wifi_hostname_from_entropy(entropy: u32) -> heapless::String<MAX_WIFI_HOSTNAME_LEN> {
    let mut s = heapless::String::new();
    let _ = s.push_str(WIFI_HOSTNAME_PREFIX);
    let mut state = entropy;
    for _ in 0..WIFI_HOSTNAME_SUFFIX_LEN {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let idx = (state >> 16) as usize % WIFI_HOSTNAME_ALPHABET.len();
        let _ = s.push(WIFI_HOSTNAME_ALPHABET[idx] as char);
    }
    s
}

fn push_u16(out: &mut heapless::String<8>, n: u16) {
    let mut buf = [0u8; 5];
    let mut len = 0usize;
    let mut v = n;
    if v == 0 {
        buf[0] = b'0';
        len = 1;
    } else {
        while v > 0 && len < buf.len() {
            buf[len] = (v % 10) as u8 + b'0';
            len += 1;
            v /= 10;
        }
    }
    while len > 0 {
        len -= 1;
        let _ = out.push(buf[len] as char);
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PersistRecord {
    pub credentials: heapless::Vec<Credential, MAX_CREDENTIALS>,
    pub locos: heapless::Vec<SavedLoco, MAX_SAVED_LOCOS>,
    /// `None` = use DHCP (no saved preference).
    pub network: Option<StaticIpConfig>,
    pub device: DeviceIdentity,
    pub wifi_hostname: heapless::String<MAX_WIFI_HOSTNAME_LEN>,
    pub language: Language,
}

impl Default for PersistRecord {
    fn default() -> Self {
        Self {
            credentials: heapless::Vec::new(),
            locos: heapless::Vec::new(),
            network: None,
            device: DeviceIdentity::default(),
            wifi_hostname: heapless::String::new(),
            language: Language::default(),
        }
    }
}

impl PersistRecord {
    pub fn find_password(&self, ssid: &str) -> Option<&str> {
        self.credentials
            .iter()
            .find(|c| c.ssid.as_str() == ssid)
            .map(|c| c.password.as_str())
    }

    /// Replace existing password or append; evict oldest when full.
    pub fn set_password(&mut self, ssid: &str, pw: &str) {
        if let Some(c) = self.credentials.iter_mut().find(|c| c.ssid.as_str() == ssid) {
            c.password.clear();
            let _ = c.password.push_str(pw);
            return;
        }
        let mut cred = Credential {
            ssid: heapless::String::new(),
            password: heapless::String::new(),
        };
        let _ = cred.ssid.push_str(ssid);
        let _ = cred.password.push_str(pw);
        if self.credentials.push(cred).is_err() {
            let _ = self.credentials.remove(0);
            let mut cred = Credential {
                ssid: heapless::String::new(),
                password: heapless::String::new(),
            };
            let _ = cred.ssid.push_str(ssid);
            let _ = cred.password.push_str(pw);
            let _ = self.credentials.push(cred);
        }
    }

    pub fn encode(&self, buf: &mut [u8]) -> Option<usize> {
        let mut off = 0;
        off = write_u32(buf, off, MAGIC)?;
        off = write_u16(buf, off, VERSION)?;
        off = write_u16(buf, off, self.credentials.len() as u16)?;
        off = write_u16(buf, off, self.locos.len() as u16)?;

        for c in &self.credentials {
            off = write_u8(buf, off, TAG_CRED)?;
            let ssid_len = c.ssid.len() as u8;
            let pw_len = c.password.len() as u8;
            off = write_u8(buf, off, ssid_len)?;
            off = write_u8(buf, off, pw_len)?;
            off = write_bytes(buf, off, c.ssid.as_bytes())?;
            off = write_bytes(buf, off, c.password.as_bytes())?;
        }
        for l in &self.locos {
            off = write_u8(buf, off, TAG_LOCO)?;
            off = write_u8(buf, off, l.throttle)?;
            off = write_u8(buf, off, l.slot)?;
            let addr_len = l.addr.len() as u8;
            off = write_u8(buf, off, addr_len)?;
            off = write_bytes(buf, off, l.addr.as_bytes())?;
        }

        if let Some(n) = &self.network {
            off = write_u8(buf, off, TAG_NET)?;
            off = write_u8(buf, off, n.dhcp as u8)?;
            off = write_bytes(buf, off, &n.ip)?;
            off = write_u8(buf, off, n.prefix_len)?;
            let gw = n.gateway.unwrap_or([0, 0, 0, 0]);
            off = write_u8(buf, off, n.gateway.is_some() as u8)?;
            off = write_bytes(buf, off, &gw)?;
            let dns = n.dns.unwrap_or([0, 0, 0, 0]);
            off = write_u8(buf, off, n.dns.is_some() as u8)?;
            off = write_bytes(buf, off, &dns)?;
        }

        off = write_u8(buf, off, TAG_DEV)?;
        let name_len = self.device.name.len() as u8;
        off = write_u8(buf, off, name_len)?;
        off = write_bytes(buf, off, self.device.name.as_bytes())?;
        off = write_u16(buf, off, self.device.id)?;

        if !self.wifi_hostname.is_empty() {
            off = write_u8(buf, off, TAG_HOST)?;
            let host_len = self.wifi_hostname.len() as u8;
            off = write_u8(buf, off, host_len)?;
            off = write_bytes(buf, off, self.wifi_hostname.as_bytes())?;
        }

        off = write_u8(buf, off, TAG_LANG)?;
        off = write_u8(buf, off, self.language.as_u8())?;

        let crc = crc32(&buf[0..off]);
        off = write_u32(buf, off, crc)?;
        Some(off)
    }

    pub fn decode(buf: &[u8]) -> Option<Self> {
        if buf.len() < 12 {
            return None;
        }
        let mut off = 0;
        let magic = read_u32(buf, &mut off)?;
        if magic != MAGIC {
            return None;
        }
        let version = read_u16(buf, &mut off)?;
        if version != 1 && version != 2 && version != 3 {
            return None;
        }
        let cred_count = read_u16(buf, &mut off)? as usize;
        let loco_count = read_u16(buf, &mut off)? as usize;

        let mut rec = PersistRecord::default();
        for _ in 0..cred_count {
            let tag = read_u8(buf, &mut off)?;
            if tag != TAG_CRED {
                return None;
            }
            let ssid_len = read_u8(buf, &mut off)? as usize;
            let pw_len = read_u8(buf, &mut off)? as usize;
            let ssid_bytes = read_slice(buf, &mut off, ssid_len)?;
            let pw_bytes = read_slice(buf, &mut off, pw_len)?;
            let mut cred = Credential {
                ssid: heapless::String::new(),
                password: heapless::String::new(),
            };
            let _ = cred.ssid.push_str(core::str::from_utf8(ssid_bytes).ok()?);
            let _ = cred.password.push_str(core::str::from_utf8(pw_bytes).ok()?);
            let _ = rec.credentials.push(cred);
        }
        for _ in 0..loco_count {
            let tag = read_u8(buf, &mut off)?;
            if tag != TAG_LOCO {
                return None;
            }
            let throttle = read_u8(buf, &mut off)?;
            let slot = read_u8(buf, &mut off)?;
            let addr_len = read_u8(buf, &mut off)? as usize;
            let addr_bytes = read_slice(buf, &mut off, addr_len)?;
            let mut loco = SavedLoco {
                throttle,
                slot,
                addr: heapless::String::new(),
            };
            let _ = loco.addr.push_str(core::str::from_utf8(addr_bytes).ok()?);
            let _ = rec.locos.push(loco);
        }

        let crc_pos = buf.len().checked_sub(4)?;
        if version >= 2 {
            while off < crc_pos {
                let tag = read_u8(buf, &mut off)?;
                match tag {
                    TAG_NET => {
                        let dhcp = read_u8(buf, &mut off)? != 0;
                        let mut ip = [0u8; 4];
                        read_bytes_into(buf, &mut off, &mut ip)?;
                        let prefix_len = read_u8(buf, &mut off)?;
                        let has_gw = read_u8(buf, &mut off)? != 0;
                        let mut gw = [0u8; 4];
                        read_bytes_into(buf, &mut off, &mut gw)?;
                        let gateway = has_gw.then_some(gw);
                        let has_dns = read_u8(buf, &mut off)? != 0;
                        let mut dns = [0u8; 4];
                        read_bytes_into(buf, &mut off, &mut dns)?;
                        let dns = has_dns.then_some(dns);
                        rec.network = Some(StaticIpConfig {
                            dhcp,
                            ip,
                            prefix_len,
                            gateway,
                            dns,
                        });
                    }
                    TAG_DEV => {
                        let name_len = read_u8(buf, &mut off)? as usize;
                        let name_bytes = read_slice(buf, &mut off, name_len)?;
                        rec.device.name.clear();
                        let _ = rec
                            .device
                            .name
                            .push_str(core::str::from_utf8(name_bytes).ok()?);
                        rec.device.id = read_u16(buf, &mut off)?;
                    }
                    TAG_HOST => {
                        let host_len = read_u8(buf, &mut off)? as usize;
                        let host_bytes = read_slice(buf, &mut off, host_len)?;
                        rec.wifi_hostname.clear();
                        let _ = rec
                            .wifi_hostname
                            .push_str(core::str::from_utf8(host_bytes).ok()?);
                    }
                    TAG_LANG => {
                        let lang = read_u8(buf, &mut off)?;
                        rec.language = Language::from_u8(lang)?;
                    }
                    _ => return None,
                }
            }
        }
        if off != crc_pos {
            return None;
        }
        let stored_crc = read_u32(buf, &mut off)?;
        let computed = crc32(&buf[0..crc_pos]);
        if stored_crc != computed {
            return None;
        }
        Some(rec)
    }
}

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

fn write_u8(buf: &mut [u8], off: usize, v: u8) -> Option<usize> {
    buf.get_mut(off).map(|b| {
        *b = v;
        off + 1
    })
}

fn write_u16(buf: &mut [u8], off: usize, v: u16) -> Option<usize> {
    let bytes = v.to_le_bytes();
    write_bytes(buf, off, &bytes)
}

fn write_u32(buf: &mut [u8], off: usize, v: u32) -> Option<usize> {
    let bytes = v.to_le_bytes();
    write_bytes(buf, off, &bytes)
}

fn write_bytes(buf: &mut [u8], off: usize, data: &[u8]) -> Option<usize> {
    let end = off.checked_add(data.len())?;
    buf.get_mut(off..end)?.copy_from_slice(data);
    Some(end)
}

fn read_u8(buf: &[u8], off: &mut usize) -> Option<u8> {
    let v = *buf.get(*off)?;
    *off += 1;
    Some(v)
}

fn read_u16(buf: &[u8], off: &mut usize) -> Option<u16> {
    let bytes = read_slice(buf, off, 2)?;
    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(buf: &[u8], off: &mut usize) -> Option<u32> {
    let bytes = read_slice(buf, off, 4)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_slice<'a>(buf: &'a [u8], off: &mut usize, len: usize) -> Option<&'a [u8]> {
    let end = off.checked_add(len)?;
    let slice = buf.get(*off..end)?;
    *off = end;
    Some(slice)
}

fn read_bytes_into(buf: &[u8], off: &mut usize, out: &mut [u8; 4]) -> Option<()> {
    let slice = read_slice(buf, off, 4)?;
    out.copy_from_slice(slice);
    Some(())
}

/// Encode v1 record bytes for backward-compat decode tests.
#[cfg(test)]
fn encode_v1(rec: &PersistRecord) -> Option<heapless::Vec<u8, 512>> {
    let mut buf = [0u8; 512];
    let mut off = 0;
    off = write_u32(&mut buf, off, MAGIC)?;
    off = write_u16(&mut buf, off, 1)?;
    off = write_u16(&mut buf, off, rec.credentials.len() as u16)?;
    off = write_u16(&mut buf, off, rec.locos.len() as u16)?;
    for c in &rec.credentials {
        off = write_u8(&mut buf, off, TAG_CRED)?;
        let ssid_len = c.ssid.len() as u8;
        let pw_len = c.password.len() as u8;
        off = write_u8(&mut buf, off, ssid_len)?;
        off = write_u8(&mut buf, off, pw_len)?;
        off = write_bytes(&mut buf, off, c.ssid.as_bytes())?;
        off = write_bytes(&mut buf, off, c.password.as_bytes())?;
    }
    for l in &rec.locos {
        off = write_u8(&mut buf, off, TAG_LOCO)?;
        off = write_u8(&mut buf, off, l.throttle)?;
        off = write_u8(&mut buf, off, l.slot)?;
        let addr_len = l.addr.len() as u8;
        off = write_u8(&mut buf, off, addr_len)?;
        off = write_bytes(&mut buf, off, l.addr.as_bytes())?;
    }
    let crc = crc32(&buf[0..off]);
    off = write_u32(&mut buf, off, crc)?;
    let mut out = heapless::Vec::new();
    let _ = out.extend_from_slice(&buf[..off]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_empty() {
        let rec = PersistRecord::default();
        let mut buf = [0u8; 512];
        let n = rec.encode(&mut buf).unwrap();
        let decoded = PersistRecord::decode(&buf[..n]).unwrap();
        assert_eq!(decoded, rec);
    }

    #[test]
    fn roundtrip_full() {
        let mut rec = PersistRecord::default();
        rec.set_password("HomeNet", "secret123");
        rec.set_password("ClubWiFi", "pass");
        let mut loco = SavedLoco {
            throttle: b'0',
            slot: 0,
            addr: heapless::String::new(),
        };
        let _ = loco.addr.push_str("S42");
        let _ = rec.locos.push(loco);
        let mut buf = [0u8; 512];
        let n = rec.encode(&mut buf).unwrap();
        let decoded = PersistRecord::decode(&buf[..n]).unwrap();
        assert_eq!(decoded, rec);
    }

    #[test]
    fn roundtrip_network_static() {
        let mut rec = PersistRecord::default();
        rec.network = Some(StaticIpConfig {
            dhcp: false,
            ip: [192, 168, 1, 50],
            prefix_len: 24,
            gateway: Some([192, 168, 1, 1]),
            dns: None,
        });
        let mut buf = [0u8; 512];
        let n = rec.encode(&mut buf).unwrap();
        let decoded = PersistRecord::decode(&buf[..n]).unwrap();
        assert_eq!(decoded, rec);
    }

    #[test]
    fn roundtrip_network_dhcp() {
        let mut rec = PersistRecord::default();
        rec.network = Some(StaticIpConfig {
            dhcp: true,
            ..Default::default()
        });
        let mut buf = [0u8; 512];
        let n = rec.encode(&mut buf).unwrap();
        let decoded = PersistRecord::decode(&buf[..n]).unwrap();
        assert_eq!(decoded, rec);
    }

    #[test]
    fn decode_v1_compat() {
        let mut rec = PersistRecord::default();
        rec.set_password("net", "pw");
        let bytes = encode_v1(&rec).unwrap();
        let decoded = PersistRecord::decode(&bytes).unwrap();
        assert_eq!(decoded.credentials, rec.credentials);
        assert_eq!(decoded.locos, rec.locos);
        assert!(decoded.network.is_none());
        assert_eq!(decoded.device.id, 0);
    }

    #[test]
    fn roundtrip_device() {
        let mut rec = PersistRecord::default();
        rec.device.name.clear();
        let _ = rec.device.name.push_str("MyPilot");
        rec.device.id = 4321;
        let mut buf = [0u8; 512];
        let n = rec.encode(&mut buf).unwrap();
        let decoded = PersistRecord::decode(&buf[..n]).unwrap();
        assert_eq!(decoded.device, rec.device);
    }

    #[test]
    fn decode_v2_without_device_tag() {
        let mut rec = PersistRecord::default();
        rec.network = Some(StaticIpConfig::default());
        let mut buf = [0u8; 512];
        let mut off = 0;
        off = write_u32(&mut buf, off, MAGIC).unwrap();
        off = write_u16(&mut buf, off, 2).unwrap();
        off = write_u16(&mut buf, off, 0).unwrap();
        off = write_u16(&mut buf, off, 0).unwrap();
        off = write_u8(&mut buf, off, TAG_NET).unwrap();
        off = write_u8(&mut buf, off, 1).unwrap();
        off = write_bytes(&mut buf, off, &[0, 0, 0, 0]).unwrap();
        off = write_u8(&mut buf, off, 24).unwrap();
        off = write_u8(&mut buf, off, 0).unwrap();
        off = write_bytes(&mut buf, off, &[0, 0, 0, 0]).unwrap();
        off = write_u8(&mut buf, off, 0).unwrap();
        off = write_bytes(&mut buf, off, &[0, 0, 0, 0]).unwrap();
        let crc = crc32(&buf[0..off]);
        off = write_u32(&mut buf, off, crc).unwrap();
        let decoded = PersistRecord::decode(&buf[..off]).unwrap();
        assert!(decoded.network.is_some());
        assert_eq!(decoded.device.id, 0);
    }

    #[test]
    fn wifi_hostname_from_entropy_format() {
        let host = wifi_hostname_from_entropy(0x1234_5678);
        assert!(host.starts_with(WIFI_HOSTNAME_PREFIX));
        assert_eq!(host.len(), WIFI_HOSTNAME_PREFIX.len() + WIFI_HOSTNAME_SUFFIX_LEN);
        for c in host[WIFI_HOSTNAME_PREFIX.len()..].chars() {
            assert!(c.is_ascii_digit() || ('a'..='z').contains(&c));
        }
    }

    #[test]
    fn roundtrip_wifi_hostname() {
        let mut rec = PersistRecord::default();
        rec.wifi_hostname = wifi_hostname_from_entropy(99);
        let mut buf = [0u8; 512];
        let n = rec.encode(&mut buf).unwrap();
        let decoded = PersistRecord::decode(&buf[..n]).unwrap();
        assert_eq!(decoded.wifi_hostname, rec.wifi_hostname);
    }

    #[test]
    fn roundtrip_language() {
        let mut rec = PersistRecord::default();
        rec.language = Language::Pl;
        let mut buf = [0u8; 512];
        let n = rec.encode(&mut buf).unwrap();
        let decoded = PersistRecord::decode(&buf[..n]).unwrap();
        assert_eq!(decoded.language, Language::Pl);
    }

    #[test]
    fn decode_without_lang_defaults_en() {
        let mut rec = PersistRecord::default();
        rec.device.id = 1234;
        let mut buf = [0u8; 512];
        let mut off = 0;
        off = write_u32(&mut buf, off, MAGIC).unwrap();
        off = write_u16(&mut buf, off, 3).unwrap();
        off = write_u16(&mut buf, off, 0).unwrap();
        off = write_u16(&mut buf, off, 0).unwrap();
        off = write_u8(&mut buf, off, TAG_DEV).unwrap();
        off = write_u8(&mut buf, off, 0).unwrap();
        off = write_u16(&mut buf, off, 1234).unwrap();
        let crc = crc32(&buf[0..off]);
        off = write_u32(&mut buf, off, crc).unwrap();
        let decoded = PersistRecord::decode(&buf[..off]).unwrap();
        assert_eq!(decoded.language, Language::En);
        assert_eq!(decoded.device.id, 1234);
    }

    #[test]
    fn id_from_entropy_range() {
        for e in [0, 1, 42, 0xFFFF_FFFF] {
            let id = id_from_entropy(e);
            assert!(id >= DEVICE_ID_MIN && id <= DEVICE_ID_MAX);
        }
    }

    #[test]
    fn set_password_replace() {
        let mut rec = PersistRecord::default();
        rec.set_password("net", "old");
        rec.set_password("net", "new");
        assert_eq!(rec.credentials.len(), 1);
        assert_eq!(rec.find_password("net"), Some("new"));
    }

    #[test]
    fn set_password_eviction() {
        let mut rec = PersistRecord::default();
        for i in 0..MAX_CREDENTIALS {
            let mut ssid = heapless::String::<8>::new();
            let _ = write_ssid(&mut ssid, i);
            rec.set_password(ssid.as_str(), "pw");
        }
        rec.set_password("overflow", "x");
        assert_eq!(rec.credentials.len(), MAX_CREDENTIALS);
        assert!(rec.find_password("0").is_none());
        assert_eq!(rec.find_password("overflow"), Some("x"));
    }

    fn write_ssid(s: &mut heapless::String<8>, i: usize) -> Result<(), ()> {
        let d = (b'0' + i as u8) as char;
        s.clear();
        s.push(d).map_err(|_| ())
    }

    #[test]
    fn decode_invalid() {
        assert!(PersistRecord::decode(&[]).is_none());
        assert!(PersistRecord::decode(&[0u8; 4]).is_none());
        let mut buf = [0u8; 32];
        buf[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        assert!(PersistRecord::decode(&buf).is_none());
    }
}
