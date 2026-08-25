//! BigFred protocol: WiThrottle drive traffic plus handset pairing.

use crate::caps::{Probe, ProtocolCaps, ProtocolInfo, ProtocolSpec};
use crate::withrottle::WiThrottle;

pub mod adapter;
pub mod pairing_http;

pub use adapter::{BigFredAdapter, PAIRING_CODE_LEN, PAIRING_SENTINEL_ADDR, PAIRING_SENTINEL_NAME};

/// Marker type that carries BigFred's [`ProtocolSpec`].
pub struct BigFred;

impl ProtocolSpec for BigFred {
    const INFO: ProtocolInfo = ProtocolInfo {
        caps: ProtocolCaps {
            pairing: true,
            steal: false,
            ..<WiThrottle as ProtocolSpec>::INFO.caps
        },
        probe: Probe::HttpGet {
            port: 8080,
            path: "/api/v1/version",
            expect: "\"product\":\"bigfred\"",
        },
        display_name: "BigFred",
        glyph: 'B',
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caps::LocoSource;

    #[test]
    fn only_bigfred_pairs_and_probes() {
        assert!(BigFred::INFO.caps.supports_pairing());
        assert_eq!(
            BigFred::INFO.probe,
            Probe::HttpGet {
                port: 8080,
                path: "/api/v1/version",
                expect: "\"product\":\"bigfred\"",
            }
        );
        assert_eq!(BigFred::INFO.display_name, "BigFred");
        assert_eq!(BigFred::INFO.glyph, 'B');
    }

    #[test]
    fn bigfred_is_withrottle_plus_pairing() {
        let w = <WiThrottle as ProtocolSpec>::INFO.caps;
        let b = BigFred::INFO.caps;
        assert_eq!(w.loco_sources, b.loco_sources);
        assert_eq!(w.transport, b.transport);
        assert_eq!(w.mdns_service, b.mdns_service);
        assert!(b.pairing && !w.pairing);
        assert!(!b.supports_steal());
        assert!(w.supports_steal());
        assert!(b.supports_source(LocoSource::ServerRoster));
    }
}
