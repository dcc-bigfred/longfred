use crate::events::ServerEvent;
use crate::model::*;

/// Parses one line (without CR/LF). Lists call `emit` multiple times.
pub fn parse(line: &str, mut emit: impl FnMut(ServerEvent)) {
    let mut line = line;

    // Digitrax LnWi garbage prefix (strip repeatedly).
    let ignore = b"AT+CIPSENDBUF=";
    while line.as_bytes().starts_with(ignore) {
        line = &line[ignore.len()..];
    }

    let b = line.as_bytes();
    let len = b.len();

    match () {
        _ if starts(line, "VN") => emit(ServerEvent::Version(short(&line[2..]))),
        _ if starts(line, "HT") => emit(ServerEvent::ServerType(short(&line[2..]))),
        _ if starts(line, "Ht") => emit(ServerEvent::ServerDescription(long(&line[2..]))),
        _ if starts(line, "HM") => emit(ServerEvent::Alert(long(&line[2..]))),
        _ if starts(line, "Hm") => emit(ServerEvent::Message(long(&line[2..]))),
        _ if starts(line, "PW") => parse_web_port(&line[2..], &mut emit),
        _ if starts(line, "PPA") && len > 3 => {
            emit(ServerEvent::TrackPower(TrackPower::from_wire(b[3] as char)));
        }
        _ if starts(line, "*") => parse_heartbeat(&line[1..], &mut emit),
        _ if starts(line, "RL") => parse_roster_list(&line[2..], &mut emit),
        _ if starts(line, "PTL") => parse_named_list(&line[3..], ListKind::Turnout, &mut emit),
        _ if starts(line, "PRL") => parse_named_list(&line[3..], ListKind::Route, &mut emit),
        _ if starts(line, "PTA") => parse_turnout_action(&line[3..], &mut emit),
        _ if starts(line, "PRA") => parse_route_action(&line[3..], &mut emit),
        _ if len > 2 && b[0] == b'M' && b[2] == b'A' => {
            parse_loco_action(b[1] as char, &line[3..], &mut emit);
        }
        _ if len > 2 && b[0] == b'M' && b[2] == b'L' => {
            parse_fn_labels(b[1] as char, &line[3..], &mut emit);
        }
        _ if len > 2 && b[0] == b'M' && (b[2] == b'+' || b[2] == b'-') => {
            parse_add_remove(b[1] as char, &line[2..], &mut emit);
        }
        _ if len > 2 && b[0] == b'M' && b[2] == b'S' => {
            parse_steal(b[1] as char, &line[3..], &mut emit);
        }
        _ if starts(line, "AT+") => emit(ServerEvent::Unknown(long(line))),
        _ => emit(ServerEvent::Unknown(long(line))),
    }
}

enum ListKind {
    Turnout,
    Route,
}

fn parse_web_port(s: &str, emit: &mut impl FnMut(ServerEvent)) {
    if let Ok(port) = s.parse::<u16>() {
        emit(ServerEvent::WebPort(port));
    }
}

fn parse_heartbeat(s: &str, emit: &mut impl FnMut(ServerEvent)) {
    if s.is_empty() || s == "+" || s == "-" {
        return;
    }
    if let Ok(seconds) = s.parse::<u32>() {
        if seconds > 0 {
            emit(ServerEvent::HeartbeatConfig { seconds });
        }
    }
}

fn parse_roster_list(s: &str, emit: &mut impl FnMut(ServerEvent)) {
    let Some(sep_pos) = find_from(s, ENTRY_SEPARATOR, 1) else {
        return;
    };
    let count: u16 = s[..sep_pos].parse().unwrap_or(0);
    emit(ServerEvent::RosterEntriesCount(count));

    let mut entry_start = sep_pos + ENTRY_SEPARATOR.len();
    for i in 0..count {
        let entry_end = find_from(s, ENTRY_SEPARATOR, entry_start).unwrap_or(s.len());
        if entry_start >= entry_end {
            break;
        }
        let entry = &s[entry_start..entry_end];
        if let Some((name, address, length)) = parse_three_segments(entry) {
            emit(ServerEvent::RosterEntry {
                index: i,
                name: short(name),
                address: address.parse().unwrap_or(0),
                length: length.chars().next().unwrap_or(' '),
            });
        }
        entry_start = entry_end + ENTRY_SEPARATOR.len();
    }
}

fn parse_named_list(s: &str, kind: ListKind, emit: &mut impl FnMut(ServerEvent)) {
    let mut entries: u16 = 0;
    let mut entry_start = ENTRY_SEPARATOR.len() + 1; // skip leading count + first separator (position 4 in C++)
    if s.len() <= 3 {
        match kind {
            ListKind::Turnout => emit(ServerEvent::TurnoutEntriesCount(0)),
            ListKind::Route => emit(ServerEvent::RouteEntriesCount(0)),
        }
        return;
    }

    loop {
        if entry_start >= s.len() {
            break;
        }
        let entry_end = find_from(s, ENTRY_SEPARATOR, entry_start).unwrap_or(s.len());
        let entry = &s[entry_start..entry_end];
        if let Some((sys_name, user_name, state)) = parse_three_segments(entry) {
            let index = entries;
            entries = entries.saturating_add(1);
            match kind {
                ListKind::Turnout => emit(ServerEvent::TurnoutEntry {
                    index,
                    sys_name: short(sys_name),
                    user_name: short(user_name),
                    state: state.parse().unwrap_or(0),
                }),
                ListKind::Route => emit(ServerEvent::RouteEntry {
                    index,
                    sys_name: short(sys_name),
                    user_name: short(user_name),
                    state: state.parse().unwrap_or(0),
                }),
            }
        }
        if entry_end >= s.len() {
            break;
        }
        entry_start = entry_end + ENTRY_SEPARATOR.len();
    }

    match kind {
        ListKind::Turnout => emit(ServerEvent::TurnoutEntriesCount(entries)),
        ListKind::Route => emit(ServerEvent::RouteEntriesCount(entries)),
    }
}

fn parse_turnout_action(s: &str, emit: &mut impl FnMut(ServerEvent)) {
    if s.is_empty() {
        return;
    }
    let action = s.as_bytes()[0];
    let sys_name = if s.len() > 2 {
        &s[1..s.len() - 1]
    } else {
        &s[1..]
    };
    let state = match action {
        b'2' => TurnoutState::Closed,
        b'4' => TurnoutState::Thrown,
        b'1' => TurnoutState::Unknown,
        b'8' => TurnoutState::Inconsistent,
        _ => TurnoutState::Unknown,
    };
    emit(ServerEvent::TurnoutAction {
        sys_name: short(sys_name),
        state,
    });
}

fn parse_route_action(s: &str, emit: &mut impl FnMut(ServerEvent)) {
    if s.is_empty() {
        return;
    }
    let action = s.as_bytes()[0];
    let sys_name = if s.len() > 2 {
        &s[1..s.len() - 1]
    } else {
        &s[1..]
    };
    let state = match action {
        b'2' => RouteState::Active,
        b'4' => RouteState::Inactive,
        _ => RouteState::Inconsistent,
    };
    emit(ServerEvent::RouteAction {
        sys_name: short(sys_name),
        state,
    });
}

fn parse_loco_action(throttle: char, s: &str, emit: &mut impl FnMut(ServerEvent)) {
    let Some(sep) = s.find(PROPERTY_SEPARATOR) else {
        return;
    };
    let addr = &s[..sep];
    let act = &s[sep + PROPERTY_SEPARATOR.len()..];
    let Some(k) = act.chars().next() else {
        return;
    };

    match k {
        'V' => {
            if let Ok(speed) = act[1..].parse::<u8>() {
                emit(ServerEvent::Speed { throttle, speed });
            }
        }
        'R' => {
            let dir = Direction::from_wire(
                act.as_bytes()
                    .get(1)
                    .copied()
                    .unwrap_or(b'1') as char,
            );
            if addr == "*" {
                emit(ServerEvent::DirectionLead { throttle, dir });
            } else {
                emit(ServerEvent::DirectionLoco {
                    throttle,
                    addr: loco(addr),
                    dir,
                });
            }
        }
        'F' => {
            let on = act.as_bytes().get(1) == Some(&b'1');
            if let Ok(func) = act[2..].parse::<u8>() {
                emit(ServerEvent::FunctionState {
                    throttle,
                    func,
                    on,
                });
            }
        }
        // 's' speed steps — not surfaced in ServerEvent (domain uses local config).
        _ => {}
    }
}

fn parse_fn_labels(throttle: char, s: &str, emit: &mut impl FnMut(ServerEvent)) {
    // Strip "{addr}<;>" or "*<;>" prefix (processRosterFunctionList).
    let remainder = if let Some(sep) = s.find(PROPERTY_SEPARATOR) {
        &s[sep + PROPERTY_SEPARATOR.len()..]
    } else {
        s
    };

    if !remainder.starts_with(']') {
        return;
    }

    let mut labels = [const { ShortText::new() }; MAX_FUNCTIONS];
    let mut count = 0usize;
    let mut start = if remainder.starts_with(ENTRY_SEPARATOR) {
        ENTRY_SEPARATOR.len()
    } else {
        3
    };

    while count < MAX_FUNCTIONS && start < remainder.len() {
        let end = find_from(remainder, ENTRY_SEPARATOR, start).unwrap_or(remainder.len());
        let _ = labels[count].push_str(&remainder[start..end]);
        count += 1;
        if end >= remainder.len() {
            break;
        }
        start = end + ENTRY_SEPARATOR.len();
    }

    emit(ServerEvent::RosterFunctionLabels { throttle, labels });
}

fn parse_add_remove(throttle: char, s: &str, emit: &mut impl FnMut(ServerEvent)) {
    if s.is_empty() {
        return;
    }
    let add = s.as_bytes()[0] == b'+';
    let remove = s.as_bytes()[0] == b'-';
    let Some(sep) = s.find(PROPERTY_SEPARATOR) else {
        return;
    };
    let addr = trim(s[1..sep].as_ref());
    let entry = trim(&s[sep + PROPERTY_SEPARATOR.len()..]);

    if add {
        emit(ServerEvent::AddressAdded {
            throttle,
            addr: loco(addr),
            entry: long(entry),
        });
    } else if remove && (entry == "d" || entry == "r" || entry == "d\n" || entry == "r\n") {
        emit(ServerEvent::AddressRemoved {
            throttle,
            addr: loco(addr),
            entry: long(entry),
        });
    }
}

fn parse_steal(throttle: char, s: &str, emit: &mut impl FnMut(ServerEvent)) {
    let Some(sep) = s.find(PROPERTY_SEPARATOR) else {
        return;
    };
    let addr = trim(&s[..sep]);
    let entry = trim(&s[sep + PROPERTY_SEPARATOR.len()..]);
    emit(ServerEvent::StealNeeded {
        throttle,
        addr: loco(addr),
        entry: long(entry),
    });
}

fn parse_three_segments(entry: &str) -> Option<(&str, &str, &str)> {
    let mut parts = [""; 3];
    let mut start = 0usize;
    for (i, slot) in parts.iter_mut().enumerate() {
        let end = entry[start..]
            .find(SEGMENT_SEPARATOR)
            .map(|p| start + p)
            .unwrap_or(entry.len());
        *slot = &entry[start..end];
        start = end + SEGMENT_SEPARATOR.len();
        if i == 2 {
            break;
        }
    }
    Some((parts[0], parts[1], parts[2]))
}

fn find_from(haystack: &str, needle: &str, from: usize) -> Option<usize> {
    haystack[from..]
        .find(needle)
        .map(|pos| from + pos)
}

fn starts(s: &str, prefix: &str) -> bool {
    s.as_bytes().starts_with(prefix.as_bytes())
}

fn trim(s: &str) -> &str {
    s.trim()
}

fn short(s: &str) -> ShortText {
    let mut out = ShortText::new();
    let _ = out.push_str(s);
    out
}

fn long(s: &str) -> LongText {
    let mut out = LongText::new();
    let _ = out.push_str(s);
    out
}

fn loco(s: &str) -> LocoAddr {
    let mut out = LocoAddr::new();
    let _ = out.push_str(s);
    out
}
