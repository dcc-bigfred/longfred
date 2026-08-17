//! WiThrottle protocol: line builders, parser, and session adapter.

use crate::caps::{LocoSourceMask, Probe, ProtocolCaps, ProtocolInfo, ProtocolSpec, Transport};

pub mod adapter;
pub mod parser;
pub mod protocol;

pub use adapter::WtAdapter;

/// mDNS PTR name advertised by WiThrottle stations.
pub const MDNS_SERVICE: &str = "_withrottle._tcp.local";

/// Marker type that carries WiThrottle's [`ProtocolSpec`].
pub struct WiThrottle;

impl ProtocolSpec for WiThrottle {
    const INFO: ProtocolInfo = ProtocolInfo {
        caps: ProtocolCaps {
            loco_sources: LocoSourceMask::ALL,
            steal: true,
            heartbeat: true,
            function_labels: true,
            pairing: false,
            transport: Transport::Tcp,
            default_port: 12090,
            mdns_service: MDNS_SERVICE,
        },
        probe: Probe::None,
        display_name: "WiThrottle",
        glyph: 'W',
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caps::LocoSource;

    #[test]
    fn withrottle_caps() {
        let caps = WiThrottle::INFO.caps;
        assert!(caps.supports_source(LocoSource::ServerRoster));
        assert!(caps.supports_source(LocoSource::StaticRoster));
        assert!(caps.supports_source(LocoSource::AddressOnly));
        assert!(caps.supports_steal());
        assert!(!caps.supports_pairing());
        assert_eq!(WiThrottle::INFO.probe, Probe::None);
        assert_eq!(WiThrottle::INFO.display_name, "WiThrottle");
        assert_eq!(WiThrottle::INFO.glyph, 'W');
        assert_eq!(caps.mdns_service, MDNS_SERVICE);
    }
}
