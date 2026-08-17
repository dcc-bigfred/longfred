//! Protocol capabilities: ask “can it do X?”, never “is it WiThrottle?”.

use crate::command::Protocol;

/// How the handset obtains locomotives for the current session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocoSource {
    /// Live roster pushed by the station.
    ServerRoster,
    /// `persist.static_roster` from programming / Soft-AP.
    StaticRoster,
    /// Manual DCC address; no list.
    AddressOnly,
}

impl LocoSource {
    /// Short diagnostics / log label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ServerRoster => "server",
            Self::StaticRoster => "static",
            Self::AddressOnly => "address",
        }
    }

    const fn bit(self) -> u8 {
        match self {
            Self::ServerRoster => 1 << 0,
            Self::StaticRoster => 1 << 1,
            Self::AddressOnly => 1 << 2,
        }
    }
}

/// Bitmask of [`LocoSource`] values a protocol can honour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocoSourceMask(u8);

impl LocoSourceMask {
    pub const NONE: Self = Self(0);
    pub const SERVER_ROSTER: Self = Self(LocoSource::ServerRoster.bit());
    pub const STATIC_ROSTER: Self = Self(LocoSource::StaticRoster.bit());
    pub const ADDRESS_ONLY: Self = Self(LocoSource::AddressOnly.bit());

    /// Shared by every protocol (invariant).
    pub const SHARED: Self = Self(Self::STATIC_ROSTER.0 | Self::ADDRESS_ONLY.0);

    /// WiThrottle / BigFred: live roster plus the shared pair.
    pub const ALL: Self = Self(Self::SERVER_ROSTER.0 | Self::SHARED.0);

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn contains(self, src: LocoSource) -> bool {
        self.0 & src.bit() != 0
    }
}

/// TCP vs UDP for the drive session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transport {
    Tcp,
    Udp,
}

/// What to send *before* the drive session is configured. Firmware executes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Probe {
    None,
    HttpGet {
        port: u16,
        path: &'static str,
        /// Substring that MUST appear in the HTTP body.
        expect: &'static str,
    },
}

/// Validate a bounded HTTP probe response without parsing JSON.
#[must_use]
pub fn http_probe_matches(response: &[u8], expect: &[u8]) -> bool {
    let Some(header_end) = response.windows(4).position(|w| w == b"\r\n\r\n") else {
        return false;
    };
    let Some(status_end) = response.windows(2).position(|w| w == b"\r\n") else {
        return false;
    };
    let status = &response[..status_end];
    let ok = status.starts_with(b"HTTP/1.1 200 ") || status.starts_with(b"HTTP/1.0 200 ");
    ok && !expect.is_empty()
        && response[header_end + 4..]
            .windows(expect.len())
            .any(|w| w == expect)
}

/// Closed set of protocol abilities. UI and domain consult this, not `Protocol` identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtocolCaps {
    pub loco_sources: LocoSourceMask,
    pub steal: bool,
    pub heartbeat: bool,
    pub function_labels: bool,
    pub pairing: bool,
    pub transport: Transport,
    pub default_port: u16,
    pub mdns_service: &'static str,
}

impl ProtocolCaps {
    #[must_use]
    pub const fn supports_source(self, src: LocoSource) -> bool {
        self.loco_sources.contains(src)
    }

    #[must_use]
    pub const fn supports_pairing(self) -> bool {
        self.pairing
    }

    #[must_use]
    pub const fn supports_steal(self) -> bool {
        self.steal
    }
}

const WIT_CAPS: ProtocolCaps = ProtocolCaps {
    loco_sources: LocoSourceMask::ALL,
    steal: true,
    heartbeat: true,
    function_labels: true,
    pairing: false,
    transport: Transport::Tcp,
    default_port: 12090,
    mdns_service: crate::network::WITHROTTLE_SERVICE,
};

const Z21_CAPS: ProtocolCaps = ProtocolCaps {
    loco_sources: LocoSourceMask::SHARED,
    steal: false,
    heartbeat: false,
    function_labels: false,
    pairing: false,
    transport: Transport::Udp,
    default_port: 21105,
    mdns_service: crate::network::Z21_SERVICE,
};

const BIGFRED_CAPS: ProtocolCaps = ProtocolCaps {
    pairing: true,
    ..WIT_CAPS
};

impl Protocol {
    /// Every known protocol. Used by the shared-source invariant test.
    pub const ALL: [Self; 3] = [Self::WiThrottle, Self::Z21, Self::BigFred];

    #[must_use]
    pub const fn caps(self) -> ProtocolCaps {
        match self {
            Self::WiThrottle => WIT_CAPS,
            Self::Z21 => Z21_CAPS,
            Self::BigFred => BIGFRED_CAPS,
        }
    }

    #[must_use]
    pub const fn probe(self) -> Probe {
        match self {
            Self::BigFred => Probe::HttpGet {
                port: 8080,
                path: "/api/v1/version",
                expect: "\"product\":\"bigfred\"",
            },
            Self::WiThrottle | Self::Z21 => Probe::None,
        }
    }

    /// Short OLED / log label. Not a substitute for [`Self::caps`].
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::WiThrottle => "WiThrottle",
            Self::Z21 => "Z21",
            Self::BigFred => "BigFred",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_protocol_includes_shared_loco_sources() {
        for p in Protocol::ALL {
            let caps = p.caps();
            assert!(
                caps.supports_source(LocoSource::StaticRoster),
                "{p:?} missing StaticRoster"
            );
            assert!(
                caps.supports_source(LocoSource::AddressOnly),
                "{p:?} missing AddressOnly"
            );
        }
    }

    #[test]
    fn z21_has_no_server_roster() {
        assert!(
            !Protocol::Z21
                .caps()
                .supports_source(LocoSource::ServerRoster)
        );
        assert!(
            Protocol::WiThrottle
                .caps()
                .supports_source(LocoSource::ServerRoster)
        );
        assert!(
            Protocol::BigFred
                .caps()
                .supports_source(LocoSource::ServerRoster)
        );
    }

    #[test]
    fn only_bigfred_pairs_and_probes() {
        assert!(!Protocol::WiThrottle.caps().supports_pairing());
        assert!(!Protocol::Z21.caps().supports_pairing());
        assert!(Protocol::BigFred.caps().supports_pairing());
        assert_eq!(Protocol::WiThrottle.probe(), Probe::None);
        assert_eq!(Protocol::Z21.probe(), Probe::None);
        assert_eq!(
            Protocol::BigFred.probe(),
            Probe::HttpGet {
                port: 8080,
                path: "/api/v1/version",
                expect: "\"product\":\"bigfred\"",
            }
        );
    }

    #[test]
    fn bigfred_is_withrottle_plus_pairing() {
        let w = Protocol::WiThrottle.caps();
        let b = Protocol::BigFred.caps();
        assert_eq!(w.loco_sources, b.loco_sources);
        assert_eq!(w.transport, b.transport);
        assert_eq!(w.mdns_service, b.mdns_service);
        assert!(b.pairing && !w.pairing);
    }

    #[test]
    fn http_probe_requires_200_and_product_in_body() {
        let expect = br#""product":"bigfred""#;
        assert!(http_probe_matches(
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"product\":\"bigfred\"}",
            expect
        ));
        assert!(!http_probe_matches(
            b"HTTP/1.1 404 Not Found\r\n\r\n{\"product\":\"bigfred\"}",
            expect
        ));
        assert!(!http_probe_matches(
            b"HTTP/1.1 200 OK\r\nX-Product: \"product\":\"bigfred\"\r\n\r\n{}",
            expect
        ));
    }
}
