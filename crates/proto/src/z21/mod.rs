//! Z21 LAN protocol: UDP X-BUS adapter.

use crate::caps::{LocoSourceMask, Probe, ProtocolCaps, ProtocolInfo, ProtocolSpec, Transport};

pub mod adapter;

pub use adapter::Z21Adapter;

/// mDNS PTR name advertised by Z21 stations.
pub const MDNS_SERVICE: &str = "_z21._udp.local";

/// Marker type that carries Z21's [`ProtocolSpec`].
pub struct Z21;

impl ProtocolSpec for Z21 {
    const INFO: ProtocolInfo = ProtocolInfo {
        caps: ProtocolCaps {
            loco_sources: LocoSourceMask::SHARED,
            steal: false,
            dead_man_switch: false,
            function_labels: false,
            pairing: false,
            transport: Transport::Udp,
            default_port: 21105,
            mdns_service: MDNS_SERVICE,
        },
        probe: Probe::None,
        display_name: "Z21",
        glyph: 'Z',
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caps::LocoSource;

    #[test]
    fn z21_has_no_server_roster() {
        let caps = Z21::INFO.caps;
        assert!(!caps.supports_source(LocoSource::ServerRoster));
        assert!(caps.supports_source(LocoSource::StaticRoster));
        assert!(caps.supports_source(LocoSource::AddressOnly));
        assert!(!caps.supports_steal());
        assert!(!caps.supports_pairing());
        assert_eq!(Z21::INFO.probe, Probe::None);
        assert_eq!(Z21::INFO.display_name, "Z21");
        assert_eq!(Z21::INFO.glyph, 'Z');
        assert_eq!(caps.mdns_service, MDNS_SERVICE);
        assert_eq!(caps.transport, Transport::Udp);
    }
}
