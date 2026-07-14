use longfred_proto::mdns::{build_ptr_query, collect_servers};

fn push_label(pkt: &mut Vec<u8>, label: &str) {
    pkt.push(label.len() as u8);
    pkt.extend_from_slice(label.as_bytes());
}

/// Odpowiedź DNS: 1× SRV (JMRI, port 12090, target host.local) + 1× A (192.168.1.50).
fn build_fixture() -> Vec<u8> {
    let mut pkt = Vec::new();
    // Header: AN=2.
    pkt.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0]);

    // --- SRV: JMRI._withrottle._tcp.local ---
    push_label(&mut pkt, "JMRI");
    push_label(&mut pkt, "_withrottle");
    push_label(&mut pkt, "_tcp");
    push_label(&mut pkt, "local");
    pkt.push(0);
    pkt.extend_from_slice(&[0x00, 0x21]); // TYPE SRV
    pkt.extend_from_slice(&[0x00, 0x01]); // CLASS IN
    pkt.extend_from_slice(&[0x00, 0x00, 0x00, 0x78]); // TTL
    // RDLENGTH = 18 (prio+weight+port + target name)
    pkt.extend_from_slice(&[0x00, 0x12]);
    pkt.extend_from_slice(&[0x00, 0x00]); // priority
    pkt.extend_from_slice(&[0x00, 0x00]); // weight
    pkt.extend_from_slice(&[0x2f, 0x3a]); // port 12090
    push_label(&mut pkt, "host");
    push_label(&mut pkt, "local");
    pkt.push(0);

    // --- A: host.local ---
    push_label(&mut pkt, "host");
    push_label(&mut pkt, "local");
    pkt.push(0);
    pkt.extend_from_slice(&[0x00, 0x01]); // TYPE A
    pkt.extend_from_slice(&[0x00, 0x01]); // CLASS IN
    pkt.extend_from_slice(&[0x00, 0x00, 0x00, 0x78]); // TTL
    pkt.extend_from_slice(&[0x00, 0x04]); // RDLENGTH
    pkt.extend_from_slice(&[192, 168, 1, 50]);

    pkt
}

#[test]
fn query_has_ptr_question() {
    let mut buf = [0u8; 64];
    let n = build_ptr_query(&mut buf);
    assert!(n > 12);
    assert_eq!(&buf[4..6], &[0, 1]);
    assert_eq!(&buf[n - 4..n], &[0x00, 0x0c, 0x00, 0x01]);
    assert_eq!(buf[12] as usize, "_withrottle".len());
}

#[test]
fn parses_srv_and_a() {
    let pkt = build_fixture();
    let servers = collect_servers::<4>(&pkt);
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].label.as_str(), "JMRI");
    assert_eq!(servers[0].port, 12090);
    assert_eq!(servers[0].ipv4, Some([192, 168, 1, 50]));
}

#[test]
fn empty_packet_yields_no_servers() {
    let servers = collect_servers::<4>(&[]);
    assert!(servers.is_empty());
}
