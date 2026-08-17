//! Integration tests for WiThrottle mDNS discovery helpers.

use longfred_proto::command::Protocol;
use longfred_proto::network::{build_ptr_query, collect_servers};
use longfred_proto::withrottle::MDNS_SERVICE;

fn push_label(pkt: &mut Vec<u8>, label: &str) {
    pkt.push(label.len() as u8);
    pkt.extend_from_slice(label.as_bytes());
}

/// DNS response: one SRV (JMRI, port 12090, target host.local) + one A (192.168.1.50).
fn build_fixture() -> Vec<u8> {
    let mut pkt = Vec::new();
    pkt.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0]);

    push_label(&mut pkt, "JMRI");
    push_label(&mut pkt, "_withrottle");
    push_label(&mut pkt, "_tcp");
    push_label(&mut pkt, "local");
    pkt.push(0);
    pkt.extend_from_slice(&[0x00, 0x21]);
    pkt.extend_from_slice(&[0x00, 0x01]);
    pkt.extend_from_slice(&[0x00, 0x00, 0x00, 0x78]);
    pkt.extend_from_slice(&[0x00, 0x12]);
    pkt.extend_from_slice(&[0x00, 0x00]);
    pkt.extend_from_slice(&[0x00, 0x00]);
    pkt.extend_from_slice(&[0x2f, 0x3a]);
    push_label(&mut pkt, "host");
    push_label(&mut pkt, "local");
    pkt.push(0);

    push_label(&mut pkt, "host");
    push_label(&mut pkt, "local");
    pkt.push(0);
    pkt.extend_from_slice(&[0x00, 0x01]);
    pkt.extend_from_slice(&[0x00, 0x01]);
    pkt.extend_from_slice(&[0x00, 0x00, 0x00, 0x78]);
    pkt.extend_from_slice(&[0x00, 0x04]);
    pkt.extend_from_slice(&[192, 168, 1, 50]);

    pkt
}

#[test]
fn query_has_ptr_question() {
    let mut buf = [0u8; 64];
    let n = build_ptr_query(MDNS_SERVICE, &mut buf);
    assert!(n > 12);
    assert_eq!(&buf[4..6], &[0, 1]);
    assert_eq!(&buf[n - 4..n], &[0x00, 0x0c, 0x00, 0x01]);
    assert_eq!(buf[12] as usize, "_withrottle".len());
}

#[test]
fn parses_srv_and_a() {
    let pkt = build_fixture();
    let servers = collect_servers::<4>(&pkt, Protocol::WiThrottle);
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].label.as_str(), "JMRI");
    assert_eq!(servers[0].port, 12090);
    assert_eq!(servers[0].ipv4, Some([192, 168, 1, 50]));
    assert_eq!(servers[0].protocol, Protocol::WiThrottle);
}

#[test]
fn empty_packet_yields_no_servers() {
    let servers = collect_servers::<4>(&[], Protocol::WiThrottle);
    assert!(servers.is_empty());
}
