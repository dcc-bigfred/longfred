use crate::model::*;

pub type Cmd = heapless::String<64>;

fn cmd(parts: &[&str]) -> Cmd {
    let mut s = Cmd::new();
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            let _ = s.push_str("");
        }
        let _ = s.push_str(p);
    }
    s
}

fn push_u8(buf: &mut heapless::String<8>, n: u8) {
    if n >= 100 {
        let _ = buf.push((b'0' + n / 100) as char);
    }
    if n >= 10 {
        let _ = buf.push((b'0' + (n / 10) % 10) as char);
    }
    let _ = buf.push((b'0' + n % 10) as char);
}

fn mta_cmd(throttle: char, addr: &str, action: &str) -> Cmd {
    let mut s = Cmd::new();
    let _ = s.push('M');
    let _ = s.push(throttle);
    let _ = s.push_str("A");
    let _ = s.push_str(addr);
    let _ = s.push_str(PROPERTY_SEPARATOR);
    let _ = s.push_str(action);
    s
}

pub fn handshake_name(name: &str) -> Cmd {
    cmd(&["N", name])
}

pub fn handshake_id(id: &str) -> Cmd {
    cmd(&["HU", id])
}

pub fn quit() -> Cmd {
    cmd(&["Q"])
}

pub fn heartbeat() -> Cmd {
    cmd(&["*"])
}

pub fn heartbeat_enable(on: bool) -> Cmd {
    if on {
        cmd(&["*", "+"])
    } else {
        cmd(&["*", "-"])
    }
}

pub fn add_loco(throttle: char, addr: &str, roster_name: &str) -> Cmd {
    let mut s = Cmd::new();
    let _ = s.push('M');
    let _ = s.push(throttle);
    let _ = s.push('+');
    let _ = s.push_str(addr);
    let _ = s.push_str(PROPERTY_SEPARATOR);
    let _ = s.push_str(roster_name);
    s
}

pub fn release_loco(throttle: char, addr: &str) -> Cmd {
    let mut s = Cmd::new();
    let _ = s.push('M');
    let _ = s.push(throttle);
    let _ = s.push('-');
    let _ = s.push_str(addr);
    let _ = s.push_str(PROPERTY_SEPARATOR);
    let _ = s.push('r');
    s
}

pub fn steal_loco(throttle: char, addr: &str) -> Cmd {
    let mut s = Cmd::new();
    let _ = s.push('M');
    let _ = s.push(throttle);
    let _ = s.push('S');
    let _ = s.push_str(addr);
    let _ = s.push_str(PROPERTY_SEPARATOR);
    let _ = s.push_str(addr);
    s
}

pub fn set_speed(throttle: char, speed: u8) -> Cmd {
    let mut act = heapless::String::<8>::new();
    let _ = act.push('V');
    // WiThrottle reserves V1 for emergency stop; the regular domain speed 1
    // must therefore use the first non-emergency wire value.
    push_u8(&mut act, if speed == 1 { 2 } else { speed });
    mta_cmd(throttle, "*", &act)
}

pub fn set_speed_steps(throttle: char, steps: u8) -> Cmd {
    let mut act = heapless::String::<8>::new();
    let _ = act.push('s');
    if steps >= 10 {
        let _ = act.push((b'0' + steps / 10) as char);
    }
    let _ = act.push((b'0' + steps % 10) as char);
    mta_cmd(throttle, "*", &act)
}

pub fn set_direction(throttle: char, addr: &str, dir: Direction) -> Cmd {
    let mut act = heapless::String::<4>::new();
    let _ = act.push('R');
    let _ = act.push(dir.to_wire());
    mta_cmd(throttle, addr, &act)
}

pub fn set_function(throttle: char, addr: &str, func: u8, pressed: bool, force: bool) -> Cmd {
    let mut act = heapless::String::<8>::new();
    let _ = act.push(if force { 'f' } else { 'F' });
    let _ = act.push(if pressed { '1' } else { '0' });
    if func >= 10 {
        let _ = act.push((b'0' + func / 10) as char);
    }
    let _ = act.push((b'0' + func % 10) as char);
    mta_cmd(throttle, addr, &act)
}

pub fn estop(throttle: char, addr: &str) -> Cmd {
    mta_cmd(throttle, addr, "X")
}

pub fn track_power(on: bool) -> Cmd {
    cmd(&["PPA", if on { "1" } else { "0" }])
}

pub fn send_raw(command: &str) -> Cmd {
    let mut s = Cmd::new();
    let _ = s.push_str(command);
    s
}
