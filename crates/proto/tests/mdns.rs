//! Integration tests for WiThrottle mDNS discovery helpers.

use longfred_proto::command::Protocol;
use longfred_proto::network::{build_ptr_query, collect_servers};
use longfred_proto::withrottle::MDNS_SERVICE;

fn push_label(pkt: &mut Vec<u8>, label: &str) {
    pkt.push(label.len() as u8);
    pkt.extend_from_slice(label.as_bytes());
}

fn push_name(pkt: &mut Vec<u8>, labels: &[&str]) {
    for label in labels {
        push_label(pkt, label);
    }
    pkt.push(0);
}

fn push_srv(pkt: &mut Vec<u8>, instance: &[&str], port: u16, target: &[&str]) {
    push_name(pkt, instance);
    pkt.extend_from_slice(&[0x00, 0x21]);
    pkt.extend_from_slice(&[0x00, 0x01]);
    pkt.extend_from_slice(&[0x00, 0x00, 0x00, 0x78]);
    let mut rdata = Vec::new();
    rdata.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    rdata.extend_from_slice(&port.to_be_bytes());
    for label in target {
        rdata.push(label.len() as u8);
        rdata.extend_from_slice(label.as_bytes());
    }
    rdata.push(0);
    pkt.extend_from_slice(&u16::try_from(rdata.len()).unwrap().to_be_bytes());
    pkt.extend_from_slice(&rdata);
}

fn push_a(pkt: &mut Vec<u8>, host: &[&str], ip: [u8; 4]) {
    push_name(pkt, host);
    pkt.extend_from_slice(&[0x00, 0x01]);
    pkt.extend_from_slice(&[0x00, 0x01]);
    pkt.extend_from_slice(&[0x00, 0x00, 0x00, 0x78]);
    pkt.extend_from_slice(&[0x00, 0x04]);
    pkt.extend_from_slice(&ip);
}

fn push_txt(pkt: &mut Vec<u8>, instance: &[&str], strings: &[&str]) {
    push_name(pkt, instance);
    pkt.extend_from_slice(&[0x00, 0x10]);
    pkt.extend_from_slice(&[0x00, 0x01]);
    pkt.extend_from_slice(&[0x00, 0x00, 0x00, 0x78]);
    let mut rdata = Vec::new();
    for s in strings {
        rdata.push(u8::try_from(s.len()).unwrap());
        rdata.extend_from_slice(s.as_bytes());
    }
    pkt.extend_from_slice(&u16::try_from(rdata.len()).unwrap().to_be_bytes());
    pkt.extend_from_slice(&rdata);
}

fn header(answers: u16) -> Vec<u8> {
    let mut pkt = vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    pkt[6..8].copy_from_slice(&answers.to_be_bytes());
    pkt
}

/// DNS response: one SRV (JMRI, port 12090, target host.local) + one A (192.168.1.50).
fn build_fixture() -> Vec<u8> {
    let mut pkt = header(2);
    push_srv(
        &mut pkt,
        &["JMRI", "_withrottle", "_tcp", "local"],
        12090,
        &["host", "local"],
    );
    push_a(&mut pkt, &["host", "local"], [192, 168, 1, 50]);
    pkt
}

fn build_googlecast_fixture() -> Vec<u8> {
    let mut pkt = header(2);
    push_srv(
        &mut pkt,
        &["Google-TV", "_googlecast", "_tcp", "local"],
        8009,
        &["tv", "local"],
    );
    push_a(&mut pkt, &["tv", "local"], [192, 168, 0, 182]);
    pkt
}

fn build_mixed_fixture() -> Vec<u8> {
    let mut pkt = header(4);
    push_srv(
        &mut pkt,
        &["JMRI", "_withrottle", "_tcp", "local"],
        12090,
        &["host", "local"],
    );
    push_srv(
        &mut pkt,
        &["Google-TV", "_googlecast", "_tcp", "local"],
        8009,
        &["tv", "local"],
    );
    push_a(&mut pkt, &["host", "local"], [192, 168, 1, 50]);
    push_a(&mut pkt, &["tv", "local"], [192, 168, 0, 182]);
    pkt
}

fn build_bigfred_txt_fixture() -> Vec<u8> {
    let mut pkt = header(3);
    push_srv(
        &mut pkt,
        &["BigFred #5", "_withrottle", "_tcp", "local"],
        12090,
        &["cs", "local"],
    );
    push_txt(
        &mut pkt,
        &["BigFred #5", "_withrottle", "_tcp", "local"],
        &[
            "proto=tcp",
            "layoutId=1",
            "commandStationId=5",
            "layoutName=Klubowa",
        ],
    );
    push_a(&mut pkt, &["cs", "local"], [192, 168, 0, 10]);
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

#[test]
fn googlecast_srv_is_rejected() {
    let pkt = build_googlecast_fixture();
    let servers = collect_servers::<4>(&pkt, Protocol::WiThrottle);
    assert!(servers.is_empty());
}

#[test]
fn mixed_packet_keeps_only_withrottle() {
    let pkt = build_mixed_fixture();
    let servers = collect_servers::<4>(&pkt, Protocol::WiThrottle);
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].label.as_str(), "JMRI");
    assert_eq!(servers[0].port, 12090);
    assert_eq!(servers[0].protocol, Protocol::WiThrottle);
}

#[test]
fn txt_layout_and_station_tags_bigfred() {
    let pkt = build_bigfred_txt_fixture();
    let servers = collect_servers::<4>(&pkt, Protocol::WiThrottle);
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].label.as_str(), "BigFred #5");
    assert_eq!(servers[0].port, 12090);
    assert_eq!(servers[0].ipv4, Some([192, 168, 0, 10]));
    assert_eq!(servers[0].protocol, Protocol::BigFred);
    assert_eq!(servers[0].layout_name.as_str(), "Klubowa");
}

#[test]
fn sort_bigfred_first_is_stable() {
    use longfred_proto::network::{WitServer, sort_bigfred_first};

    fn srv(label: &str, protocol: Protocol) -> WitServer {
        let mut l = heapless::String::new();
        let _ = l.push_str(label);
        WitServer {
            label: l,
            layout_name: heapless::String::new(),
            port: 12090,
            ipv4: None,
            protocol,
        }
    }
    let mut v = [
        srv("JMRI", Protocol::WiThrottle),
        srv("BigA", Protocol::BigFred),
        srv("Z21", Protocol::Z21),
        srv("BigB", Protocol::BigFred),
    ];
    sort_bigfred_first(&mut v);
    assert_eq!(v[0].label.as_str(), "BigA");
    assert_eq!(v[1].label.as_str(), "BigB");
    assert_eq!(v[2].label.as_str(), "JMRI");
    assert_eq!(v[3].label.as_str(), "Z21");
}

#[test]
fn srv_without_txt_stays_withrottle() {
    let pkt = build_fixture();
    let servers = collect_servers::<4>(&pkt, Protocol::WiThrottle);
    assert_eq!(servers[0].protocol, Protocol::WiThrottle);
}
