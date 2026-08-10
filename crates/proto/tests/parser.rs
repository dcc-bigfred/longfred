use longfred_proto::ServerEvent;
use longfred_proto::model::{Direction, TrackPower};
use longfred_proto::parser::parse;

fn collect(line: &str) -> Vec<ServerEvent> {
    let mut events = Vec::new();
    parse(line, |event| events.push(event));
    events
}

#[test]
fn version() {
    let events = collect("VN2.0");
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], ServerEvent::Version(_)));
}

#[test]
fn server_description() {
    let events = collect("HtDCC-EX v5.0");
    assert!(matches!(events[0], ServerEvent::ServerDescription(_)));
}

#[test]
fn heartbeat_config() {
    let events = collect("*10");
    assert_eq!(events[0], ServerEvent::HeartbeatConfig { seconds: 10 });
}

#[test]
fn power_on() {
    assert_eq!(collect("PPA1")[0], ServerEvent::TrackPower(TrackPower::On));
}

#[test]
fn power_off() {
    assert_eq!(collect("PPA0")[0], ServerEvent::TrackPower(TrackPower::Off));
}

#[test]
fn speed() {
    assert_eq!(
        collect("MTAL341<;>V63")[0],
        ServerEvent::Speed {
            throttle: 'T',
            speed: 63
        }
    );
}

#[test]
fn direction_lead_reverse() {
    assert_eq!(
        collect("MTA*<;>R0")[0],
        ServerEvent::DirectionLead {
            throttle: 'T',
            dir: Direction::Reverse
        }
    );
}

#[test]
fn direction_loco_forward() {
    assert_eq!(
        collect("MTAL341<;>R1")[0],
        ServerEvent::DirectionLoco {
            throttle: 'T',
            addr: {
                let mut a = longfred_proto::model::LocoAddr::new();
                let _ = a.push_str("L341");
                a
            },
            dir: Direction::Forward
        }
    );
}

#[test]
fn function_on() {
    assert_eq!(
        collect("MTAL341<;>F18")[0],
        ServerEvent::FunctionState {
            throttle: 'T',
            func: 8,
            on: true
        }
    );
}

#[test]
fn function_off() {
    assert_eq!(
        collect("MTAL341<;>F00")[0],
        ServerEvent::FunctionState {
            throttle: 'T',
            func: 0,
            on: false
        }
    );
}

#[test]
fn roster_list() {
    let line = "RL2]\\[Big Boy}|{4014}|{L]\\[Shay}|{12}|{S";
    let events = collect(line);
    assert_eq!(events[0], ServerEvent::RosterEntriesCount(2));
    assert!(matches!(
        events[1],
        ServerEvent::RosterEntry { index: 0, .. }
    ));
    assert!(matches!(
        events[2],
        ServerEvent::RosterEntry { index: 1, .. }
    ));

    if let ServerEvent::RosterEntry {
        name,
        address,
        length,
        ..
    } = &events[1]
    {
        assert_eq!(name.as_str(), "Big Boy");
        assert_eq!(*address, 4014);
        assert_eq!(*length, 'L');
    } else {
        panic!("expected roster entry");
    }
}

#[test]
fn turnout_list() {
    let line = "PTL1]\\[IT1}|{Turnout 1}|{2";
    let events = collect(line);
    assert!(matches!(events[0], ServerEvent::TurnoutEntry { .. }));
    assert_eq!(events[1], ServerEvent::TurnoutEntriesCount(1));
}

#[test]
fn route_list() {
    let line = "PRL1]\\[IO:001}|{Main Route}|{4";
    let events = collect(line);
    assert!(matches!(events[0], ServerEvent::RouteEntry { .. }));
    assert_eq!(events[1], ServerEvent::RouteEntriesCount(1));
}

#[test]
fn address_added() {
    assert!(matches!(
        collect("MT+L341<;>Big Boy")[0],
        ServerEvent::AddressAdded { .. }
    ));
}

#[test]
fn address_removed() {
    assert!(matches!(
        collect("MT-L341<;>r")[0],
        ServerEvent::AddressRemoved { .. }
    ));
}

#[test]
fn steal_needed() {
    assert!(matches!(
        collect("MTSL341<;>L341")[0],
        ServerEvent::StealNeeded { .. }
    ));
}

#[test]
fn roster_function_labels() {
    let line = "MTLL341<;>]\\[Headlight]\\[Bell";
    let events = collect(line);
    if let ServerEvent::RosterFunctionLabels { labels, .. } = &events[0] {
        assert_eq!(labels[0].as_str(), "Headlight");
        assert_eq!(labels[1].as_str(), "Bell");
    } else {
        panic!("expected function labels");
    }
}

#[test]
fn unknown_command() {
    assert!(matches!(collect("XYZZY")[0], ServerEvent::Unknown(_)));
}
