//! Host-testable helpers for menu FSM (stage 9).

pub const PW_BLANK_CHAR: u8 = 164;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuFinish {
    None,
    AcquireAddr(heapless::String<8>),
    RosterList,
    ReleaseAll,
    DirectionToggle,
    SpeedMultiplier,
    TurnoutThrowAddr(heapless::String<8>),
    TurnoutCloseAddr(heapless::String<8>),
    TurnoutList { throw: bool },
    RouteAddr(heapless::String<8>),
    RouteList,
    PowerToggle,
    FunctionPress(u8),
    FunctionList,
}

/// Interprets `menu_cmd` after `#` (without a leading asterisk).
pub fn finish_menu(cmd: &str) -> MenuFinish {
    if cmd.is_empty() {
        return MenuFinish::None;
    }
    let mut bytes = cmd.as_bytes().iter();
    let first = *bytes.next().unwrap() as char;
    let mut rest = heapless::String::<8>::new();
    let _ = rest.push_str(cmd.get(1..).unwrap_or(""));
    match first {
        '0' => {
            if rest.is_empty() {
                MenuFinish::FunctionList
            } else if let Ok(f) = rest.parse::<u8>() {
                MenuFinish::FunctionPress(f)
            } else {
                MenuFinish::None
            }
        }
        '1' => {
            if rest.is_empty() {
                MenuFinish::RosterList
            } else {
                MenuFinish::AcquireAddr(rest)
            }
        }
        '2' => MenuFinish::ReleaseAll,
        '3' => MenuFinish::DirectionToggle,
        '4' => MenuFinish::SpeedMultiplier,
        '5' => {
            if rest.is_empty() {
                MenuFinish::TurnoutList { throw: true }
            } else {
                MenuFinish::TurnoutThrowAddr(rest)
            }
        }
        '6' => {
            if rest.is_empty() {
                MenuFinish::TurnoutList { throw: false }
            } else {
                MenuFinish::TurnoutCloseAddr(rest)
            }
        }
        '7' => {
            if rest.is_empty() {
                MenuFinish::RouteList
            } else {
                MenuFinish::RouteAddr(rest)
            }
        }
        '8' => MenuFinish::PowerToggle,
        _ => MenuFinish::None,
    }
}

/// Parses 17 digits `###.###.###.###:#####` (no separators).
pub fn parse_ip_endpoint(digits: &str) -> Option<([u8; 4], u16)> {
    if digits.len() != 17 {
        return None;
    }
    let oct = |s: &str| s.parse::<u8>().ok();
    let ip = [
        oct(&digits[0..3])?,
        oct(&digits[3..6])?,
        oct(&digits[6..9])?,
        oct(&digits[9..12])?,
    ];
    let port: u16 = digits[12..17].parse().ok()?;
    Some((ip, port))
}

/// Password picker step (ASCII 32..126, blank = 164).
pub fn step_pw_char(cur: u8, cw: bool) -> u8 {
    if cur == PW_BLANK_CHAR {
        return if cw { b'B' } else { b'@' };
    }
    let mut c = cur as i32;
    if cw {
        c -= 1;
        if c < 32 {
            c = 126;
        }
    } else {
        c += 1;
        if c > 126 {
            c = 32;
        }
    }
    c as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_acquire_addr() {
        let f = finish_menu("1222");
        assert!(matches!(f, MenuFinish::AcquireAddr(ref a) if a.as_str() == "222"));
    }

    #[test]
    fn menu_roster_list() {
        assert_eq!(finish_menu("1"), MenuFinish::RosterList);
    }

    #[test]
    fn menu_turnout_list_throw() {
        assert_eq!(
            finish_menu("5"),
            MenuFinish::TurnoutList { throw: true }
        );
    }

    #[test]
    fn ip_endpoint() {
        let (ip, port) = parse_ip_endpoint("19216800400102560").unwrap();
        assert_eq!(ip, [192, 168, 4, 1]);
        assert_eq!(port, 2560);
    }

    #[test]
    fn pw_char_step() {
        assert_eq!(step_pw_char(PW_BLANK_CHAR, true), b'B');
        assert_eq!(step_pw_char(b'A', false), b'B');
    }
}
