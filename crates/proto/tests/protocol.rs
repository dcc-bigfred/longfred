//! Integration tests for WiThrottle protocol framing helpers.

use longfred_proto::model::Direction;
use longfred_proto::withrottle::protocol as p;

#[test]
fn handshake_name() {
    assert_eq!(p::handshake_name("WiFred").as_str(), "NWiFred");
}

#[test]
fn handshake_id() {
    assert_eq!(p::handshake_id("1234").as_str(), "HU1234");
}

#[test]
fn quit_cmd() {
    assert_eq!(p::quit().as_str(), "Q");
}

#[test]
fn heartbeat() {
    assert_eq!(p::heartbeat().as_str(), "*");
}

#[test]
fn dead_man_switch_enable() {
    assert_eq!(p::dead_man_switch_enable(true).as_str(), "*+");
    assert_eq!(p::dead_man_switch_enable(false).as_str(), "*-");
}

#[test]
fn speed() {
    assert_eq!(p::set_speed('T', 0).as_str(), "MTA*<;>V0");
    assert_eq!(p::set_speed('T', 1).as_str(), "MTA*<;>V2");
    assert_eq!(p::set_speed('T', 63).as_str(), "MTA*<;>V63");
}

#[test]
fn speed_steps() {
    assert_eq!(p::set_speed_steps('T', 1).as_str(), "MTA*<;>s1");
}

#[test]
fn direction() {
    assert_eq!(
        p::set_direction('T', "*", Direction::Reverse).as_str(),
        "MTA*<;>R0"
    );
    assert_eq!(
        p::set_direction('T', "L341", Direction::Forward).as_str(),
        "MTAL341<;>R1"
    );
}

#[test]
fn function() {
    assert_eq!(
        p::set_function('T', "L341", 8, true, false).as_str(),
        "MTAL341<;>F18"
    );
    assert_eq!(
        p::set_function('T', "L341", 8, true, true).as_str(),
        "MTAL341<;>f18"
    );
    assert_eq!(
        p::set_function('0', "*", 1, true, false).as_str(),
        "M0A*<;>F11"
    );
    assert_eq!(
        p::set_function('0', "*", 1, false, false).as_str(),
        "M0A*<;>F01"
    );
}

#[test]
fn estop() {
    assert_eq!(p::estop('T', "*").as_str(), "MTA*<;>X");
}

#[test]
fn power() {
    assert_eq!(p::track_power(true).as_str(), "PPA1");
    assert_eq!(p::track_power(false).as_str(), "PPA0");
}

#[test]
fn add_loco() {
    assert_eq!(
        p::add_loco('T', "L341", "Big Boy").as_str(),
        "MT+L341<;>Big Boy"
    );
}

#[test]
fn release_loco() {
    assert_eq!(p::release_loco('T', "L341").as_str(), "MT-L341<;>r");
}

#[test]
fn steal_loco() {
    assert_eq!(p::steal_loco('T', "L341").as_str(), "MTSL341<;>L341");
}

#[test]
fn heartbeat_send_period_is_half_advertised_timeout() {
    assert_eq!(p::heartbeat_send_period_s(10), 5);
    assert_eq!(p::heartbeat_send_period_s(1), 1);
    assert_eq!(p::heartbeat_send_period_s(2), 1);
}
