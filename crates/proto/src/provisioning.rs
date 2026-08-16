//! HTTP provisioning settings DTOs (serde-json-core, no heapless feature).

#[cfg(test)]
use crate::persist::{MAX_BIGFRED_LOGIN_LEN, MAX_WIFI_HOSTNAME_LEN};
use crate::persist::{
    MAX_CREDENTIALS, MAX_SAVED_LOCOS, PersistRecord, RosterMode, StaticRosterEntry,
};

use serde::{Deserialize, Serialize};

/// Device identity in a GET settings response.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct DeviceView<'a> {
    pub name: &'a str,
    pub id: u16,
}

/// Wi-Fi related fields in a GET settings response.
#[derive(Clone, Debug, Serialize)]
pub struct WifiView<'a> {
    pub hostname: &'a str,
    /// Saved network SSIDs (passwords of the last-used network are also returned).
    pub networks: NetworksView<'a>,
    pub password: &'a str,
}

/// Serialize saved SSIDs as a JSON array (variable length).
#[derive(Clone, Copy, Debug)]
pub struct NetworksView<'a> {
    pub ssids: &'a [&'a str],
}

impl Serialize for NetworksView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.ssids.len()))?;
        for s in self.ssids {
            seq.serialize_element(s)?;
        }
        seq.end()
    }
}

/// BigFred credentials in a GET settings response.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct BigfredView<'a> {
    pub login: &'a str,
    pub pin: &'a str,
    pub pin_set: bool,
}

/// One static roster entry for GET.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct RosterEntryView<'a> {
    pub addr: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<&'a str>,
}

/// Roster section for GET.
#[derive(Clone, Debug, Serialize)]
pub struct RosterView<'a> {
    pub mode: RosterModeName,
    pub entries: RosterEntriesView<'a>,
}

/// Serialize roster entries as a JSON array (variable length).
#[derive(Clone, Copy, Debug)]
pub struct RosterEntriesView<'a> {
    pub entries: &'a [StaticRosterEntry],
}

impl Serialize for RosterEntriesView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.entries.len()))?;
        for e in self.entries {
            let name = if e.name.is_empty() {
                None
            } else {
                Some(e.name.as_str())
            };
            seq.serialize_element(&RosterEntryView {
                addr: e.addr.as_str(),
                name,
            })?;
        }
        seq.end()
    }
}

/// Wire name for [`RosterMode`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RosterModeName {
    Auto,
    Static,
}

impl From<RosterMode> for RosterModeName {
    fn from(m: RosterMode) -> Self {
        match m {
            RosterMode::Auto => Self::Auto,
            RosterMode::Static => Self::Static,
        }
    }
}

impl From<RosterModeName> for RosterMode {
    fn from(m: RosterModeName) -> Self {
        match m {
            RosterModeName::Auto => Self::Auto,
            RosterModeName::Static => Self::Static,
        }
    }
}

/// Full GET `/settings` body (borrows into [`PersistRecord`] / scratch).
#[derive(Clone, Debug, Serialize)]
pub struct SettingsGet<'a> {
    pub device: DeviceView<'a>,
    pub wifi: WifiView<'a>,
    pub bigfred: BigfredView<'a>,
    pub roster: RosterView<'a>,
    pub programming_mode: bool,
    pub firmware: FirmwareView<'a>,
}

/// Firmware identity in a GET settings response.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct FirmwareView<'a> {
    pub version: &'a str,
}

/// Serialize current settings into `buf`. Returns bytes written.
///
/// `network_ssids` is a scratch array of `&str` pointing at `rec.credentials[*].ssid`.
pub fn serialize_settings(
    buf: &mut [u8],
    rec: &PersistRecord,
    network_ssids: &[&str],
    firmware_version: &str,
) -> Result<usize, serde_json_core::ser::Error> {
    let view = SettingsGet {
        device: DeviceView {
            name: rec.device.name.as_str(),
            id: rec.device.id,
        },
        wifi: WifiView {
            hostname: rec.wifi_hostname.as_str(),
            networks: NetworksView {
                ssids: network_ssids,
            },
            password: rec
                .last_credential()
                .map(|c| c.password.as_str())
                .unwrap_or(""),
        },
        bigfred: BigfredView {
            login: rec.bigfred_login.as_str(),
            pin: rec.bigfred_pin.as_str(),
            pin_set: !rec.bigfred_pin.is_empty(),
        },
        roster: RosterView {
            mode: rec.roster_mode.into(),
            entries: RosterEntriesView {
                entries: rec.static_roster.as_slice(),
            },
        },
        programming_mode: rec.programming_mode,
        firmware: FirmwareView {
            version: firmware_version,
        },
    };
    serde_json_core::to_slice(&view, buf)
}

/// Helper: fill a ssid scratch buffer from `rec` then serialize.
pub fn serialize_settings_from_record(
    buf: &mut [u8],
    rec: &PersistRecord,
    firmware_version: &str,
) -> Result<usize, serde_json_core::ser::Error> {
    let mut ssids: [&str; MAX_CREDENTIALS] = [""; MAX_CREDENTIALS];
    let n = rec.credentials.len().min(MAX_CREDENTIALS);
    for (slot, cred) in ssids.iter_mut().zip(rec.credentials.iter()).take(n) {
        *slot = cred.ssid.as_str();
    }
    serialize_settings(buf, rec, &ssids[..n], firmware_version)
}

/// Optional Wi-Fi fields in a PUT body.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub struct WifiPut<'a> {
    #[serde(default)]
    #[serde(borrow)]
    pub ssid: Option<&'a str>,
    #[serde(default)]
    #[serde(borrow)]
    pub password: Option<&'a str>,
    #[serde(default)]
    #[serde(borrow)]
    pub hostname: Option<&'a str>,
}

/// Optional BigFred fields in a PUT body.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub struct BigfredPut<'a> {
    #[serde(default)]
    #[serde(borrow)]
    pub login: Option<&'a str>,
    #[serde(default)]
    #[serde(borrow)]
    pub pin: Option<&'a str>,
}

/// One roster entry in a PUT body.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct RosterEntryPut<'a> {
    #[serde(borrow)]
    pub addr: &'a str,
    #[serde(default)]
    #[serde(borrow)]
    pub name: Option<&'a str>,
}

/// PUT `/settings` body. All fields optional; missing tags leave persist unchanged.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct SettingsPut<'a> {
    #[serde(default)]
    #[serde(borrow)]
    pub wifi: Option<WifiPut<'a>>,
    #[serde(default)]
    #[serde(borrow)]
    pub bigfred: Option<BigfredPut<'a>>,
    #[serde(default)]
    pub programming_mode: Option<bool>,
    #[serde(default)]
    pub roster_mode: Option<RosterModeName>,
    /// Up to [`MAX_SAVED_LOCOS`] entries; shorter JSON arrays are accepted.
    #[serde(default)]
    #[serde(borrow)]
    #[serde(deserialize_with = "deserialize_roster_entries")]
    pub roster: [Option<RosterEntryPut<'a>>; MAX_SAVED_LOCOS],
}

fn deserialize_roster_entries<'de, D>(
    deserializer: D,
) -> Result<[Option<RosterEntryPut<'de>>; MAX_SAVED_LOCOS], D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Visitor;

    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = [Option<RosterEntryPut<'de>>; MAX_SAVED_LOCOS];

        fn expecting(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
            write!(f, "array of at most {MAX_SAVED_LOCOS} roster entries")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut out: [Option<RosterEntryPut<'de>>; MAX_SAVED_LOCOS] = [None; MAX_SAVED_LOCOS];
            let mut i = 0usize;
            while let Some(item) = seq.next_element::<RosterEntryPut<'de>>()? {
                if i >= MAX_SAVED_LOCOS {
                    return Err(serde::de::Error::custom("roster too long"));
                }
                out[i] = Some(item);
                i += 1;
            }
            Ok(out)
        }
    }

    deserializer.deserialize_seq(Visitor)
}

/// Deserialize a PUT body from JSON bytes.
pub fn deserialize_settings_put(buf: &[u8]) -> Result<SettingsPut<'_>, serde_json_core::de::Error> {
    let (put, _rest) = serde_json_core::from_slice(buf)?;
    Ok(put)
}

/// Error returned by [`apply_settings_put`] when a field exceeds its
/// fixed-capacity storage in [`PersistRecord`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyError {
    /// `wifi.hostname` longer than `MAX_WIFI_HOSTNAME_LEN`.
    HostnameTooLong,
    /// `bigfred.login` longer than `MAX_BIGFRED_LOGIN_LEN`.
    LoginTooLong,
    /// `bigfred.pin` longer than `MAX_BIGFRED_PIN_LEN`.
    PinTooLong,
    /// A roster `addr` longer than the entry capacity.
    RosterAddrTooLong,
    /// A roster `name` longer than `MAX_STATIC_ROSTER_NAME_LEN`.
    RosterNameTooLong,
    /// More roster entries than `MAX_SAVED_LOCOS`.
    RosterFull,
}

/// Apply a PUT body onto a persist record.
///
/// Returns `Err(ApplyError)` when a string does not fit its fixed-capacity
/// storage; in that case the record is left in a partially-updated state
/// (callers should treat the whole PUT as rejected).
pub fn apply_settings_put(
    rec: &mut PersistRecord,
    put: &SettingsPut<'_>,
) -> Result<(), ApplyError> {
    if let Some(wifi) = &put.wifi {
        if let (Some(ssid), Some(password)) = (wifi.ssid, wifi.password) {
            rec.set_password(ssid, password);
        }
        if let Some(host) = wifi.hostname {
            rec.wifi_hostname.clear();
            if rec.wifi_hostname.push_str(host).is_err() {
                return Err(ApplyError::HostnameTooLong);
            }
        }
    }

    if let Some(bf) = &put.bigfred {
        if let Some(login) = bf.login {
            rec.bigfred_login.clear();
            if rec.bigfred_login.push_str(login).is_err() {
                return Err(ApplyError::LoginTooLong);
            }
        }
        if let Some(pin) = bf.pin {
            rec.bigfred_pin.clear();
            if rec.bigfred_pin.push_str(pin).is_err() {
                return Err(ApplyError::PinTooLong);
            }
        }
    }

    if let Some(pm) = put.programming_mode {
        rec.programming_mode = pm;
    }

    if let Some(mode) = put.roster_mode {
        rec.roster_mode = mode.into();
    }

    // Replace roster when any entry is present. Missing `roster` key deserializes
    // to all-None and leaves the existing roster unchanged.
    let has_entries = put.roster.iter().any(|e| e.is_some());
    if has_entries {
        rec.static_roster.clear();
        for slot in &put.roster {
            let Some(e) = slot else { continue };
            let mut entry = StaticRosterEntry::default();
            if entry.addr.push_str(e.addr).is_err() {
                return Err(ApplyError::RosterAddrTooLong);
            }
            if let Some(name) = e.name
                && entry.name.push_str(name).is_err()
            {
                return Err(ApplyError::RosterNameTooLong);
            }
            if rec.static_roster.push(entry).is_err() {
                return Err(ApplyError::RosterFull);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_empty_settings() {
        let rec = PersistRecord::default();
        let mut buf = [0u8; 512];
        let n = serialize_settings_from_record(&mut buf, &rec, "0.1.0").unwrap();
        let s = core::str::from_utf8(&buf[..n]).unwrap();
        assert!(s.contains("\"programming_mode\":false"));
        assert!(s.contains("\"version\":\"0.1.0\""));
        assert!(s.contains("\"mode\":\"auto\""));
        assert!(s.contains("\"pin_set\":false"));
        assert!(s.contains("\"networks\":[]"));
        assert!(s.contains("\"entries\":[]"));
    }

    #[test]
    fn serialize_populated() {
        let mut rec = PersistRecord::default();
        rec.device.name.clear();
        let _ = rec.device.name.push_str("Pilot");
        rec.device.id = 4242;
        let _ = rec.wifi_hostname.push_str("longred_abc123");
        rec.set_password("Home", "secret");
        let _ = rec.bigfred_login.push_str("bob");
        let _ = rec.bigfred_pin.push_str("9999");
        rec.programming_mode = true;
        rec.roster_mode = RosterMode::Static;
        let mut e = StaticRosterEntry::default();
        let _ = e.addr.push_str("L1");
        let _ = e.name.push_str("One");
        let _ = rec.static_roster.push(e);

        let mut buf = [0u8; 1024];
        let n = serialize_settings_from_record(&mut buf, &rec, "0.1.0").unwrap();
        let s = core::str::from_utf8(&buf[..n]).unwrap();
        assert!(s.contains("\"name\":\"Pilot\""));
        assert!(s.contains("\"id\":4242"));
        assert!(s.contains("\"hostname\":\"longred_abc123\""));
        assert!(s.contains("\"networks\":[\"Home\"]"));
        assert!(s.contains("\"login\":\"bob\""));
        assert!(s.contains("\"pin_set\":true"));
        assert!(s.contains("\"pin\":\"9999\""));
        assert!(s.contains("\"password\":\"secret\""));
        assert!(s.contains("\"mode\":\"static\""));
        assert!(s.contains("\"addr\":\"L1\""));
        assert!(s.contains("\"name\":\"One\""));
        assert!(s.contains("\"programming_mode\":true"));
    }

    #[test]
    fn deserialize_partial_put() {
        let json = br#"{"wifi":{"ssid":"Net","password":"pw"},"programming_mode":true}"#;
        let put = deserialize_settings_put(json).unwrap();
        assert_eq!(put.wifi.unwrap().ssid, Some("Net"));
        assert_eq!(put.wifi.unwrap().password, Some("pw"));
        assert_eq!(put.programming_mode, Some(true));
        assert!(put.bigfred.is_none());
        assert!(put.roster.iter().all(|e| e.is_none()));
    }

    #[test]
    fn deserialize_and_apply_roster() {
        let json = br#"{
            "roster_mode":"static",
            "roster":[{"addr":"S42","name":"Switch"},{"addr":"L7"}],
            "bigfred":{"login":"ops","pin":"1234"}
        }"#;
        let put = deserialize_settings_put(json).unwrap();
        let mut rec = PersistRecord::default();
        assert!(apply_settings_put(&mut rec, &put).is_ok());
        assert_eq!(rec.roster_mode, RosterMode::Static);
        assert_eq!(rec.static_roster.len(), 2);
        assert_eq!(rec.static_roster[0].addr.as_str(), "S42");
        assert_eq!(rec.static_roster[0].name.as_str(), "Switch");
        assert_eq!(rec.static_roster[1].addr.as_str(), "L7");
        assert!(rec.static_roster[1].name.is_empty());
        assert_eq!(rec.bigfred_login.as_str(), "ops");
        assert_eq!(rec.bigfred_pin.as_str(), "1234");
    }

    #[test]
    fn apply_wifi_password() {
        let json = br#"{"wifi":{"ssid":"Club","password":"x"}}"#;
        let put = deserialize_settings_put(json).unwrap();
        let mut rec = PersistRecord::default();
        assert!(apply_settings_put(&mut rec, &put).is_ok());
        assert_eq!(rec.find_password("Club"), Some("x"));
    }

    #[test]
    fn put_missing_fields_noop() {
        let put = deserialize_settings_put(br#"{}"#).unwrap();
        let mut rec = PersistRecord::default();
        let _ = rec.bigfred_login.push_str("keep");
        rec.programming_mode = true;
        assert!(apply_settings_put(&mut rec, &put).is_ok());
        assert_eq!(rec.bigfred_login.as_str(), "keep");
        assert!(rec.programming_mode);
    }

    #[test]
    fn apply_too_long_hostname_returns_typed_error() {
        let long = "x".repeat(MAX_WIFI_HOSTNAME_LEN + 1);
        let json = format!(r#"{{"wifi":{{"hostname":"{long}"}}}}"#);
        let put = deserialize_settings_put(json.as_bytes()).unwrap();
        let mut rec = PersistRecord::default();
        assert_eq!(
            apply_settings_put(&mut rec, &put),
            Err(ApplyError::HostnameTooLong)
        );
    }

    #[test]
    fn apply_too_long_login_returns_typed_error() {
        let long = "x".repeat(MAX_BIGFRED_LOGIN_LEN + 1);
        let json = format!(r#"{{"bigfred":{{"login":"{long}"}}}}"#);
        let put = deserialize_settings_put(json.as_bytes()).unwrap();
        let mut rec = PersistRecord::default();
        assert_eq!(
            apply_settings_put(&mut rec, &put),
            Err(ApplyError::LoginTooLong)
        );
    }
}
