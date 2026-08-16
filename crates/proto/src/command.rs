//! Protocol-agnostic client commands produced by the domain layer.

use crate::model::{Direction, ShortText};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Protocol {
    WiThrottle,
    Z21,
}

impl Protocol {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::WiThrottle => 0,
            Self::Z21 => 1,
        }
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::WiThrottle),
            1 => Some(Self::Z21),
            _ => None,
        }
    }
}

/// Numeric DCC loco identity (protocol-neutral).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LocoId {
    pub addr: u16,
    pub long: bool,
}

impl LocoId {
    /// Parse `S123` / `L341` / bare digits (long if value >= 128).
    pub fn parse(s: &str) -> Option<Self> {
        let (long, digits) = match s.as_bytes().first() {
            Some(b'S') | Some(b's') => (false, &s[1..]),
            Some(b'L') | Some(b'l') => (true, &s[1..]),
            _ => {
                let addr = s.parse::<u16>().ok()?;
                return Some(Self {
                    addr,
                    long: addr >= 128,
                });
            }
        };
        let addr = digits.parse::<u16>().ok()?;
        Some(Self { addr, long })
    }

    /// WiThrottle-style address string (`S123` / `L341`).
    pub fn to_wire(self) -> heapless::String<8> {
        let mut s = heapless::String::new();
        let _ = s.push(if self.long { 'L' } else { 'S' });
        let mut n = self.addr;
        let mut digits = [0u8; 5];
        let mut len = 0usize;
        if n == 0 {
            let _ = s.push('0');
            return s;
        }
        while n > 0 && len < digits.len() {
            digits[len] = (n % 10) as u8;
            len += 1;
            n /= 10;
        }
        while len > 0 {
            len -= 1;
            let _ = s.push((b'0' + digits[len]) as char);
        }
        s
    }
}

/// Protocol-agnostic command produced by the domain.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ClientCommand {
    AddLoco {
        throttle: u8,
        loco: LocoId,
        name: ShortText,
    },
    ReleaseThrottle {
        throttle: u8,
    },
    SetSpeed {
        throttle: u8,
        speed: u8,
    },
    SetDirection {
        throttle: u8,
        /// `None` → all locos on throttle (WiThrottle `"*"`).
        loco: Option<LocoId>,
        dir: Direction,
    },
    EStop {
        throttle: u8,
    },
    SetFunction {
        throttle: u8,
        func: u8,
        on: bool,
        all: bool,
    },
    TrackPower(bool),
    SetHeartbeat(bool),
    Steal {
        throttle: u8,
        loco: LocoId,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loco_id_parse_short_long() {
        assert_eq!(
            LocoId::parse("S31"),
            Some(LocoId {
                addr: 31,
                long: false
            })
        );
        assert_eq!(
            LocoId::parse("L341"),
            Some(LocoId {
                addr: 341,
                long: true
            })
        );
        assert_eq!(
            LocoId::parse("200"),
            Some(LocoId {
                addr: 200,
                long: true
            })
        );
    }

    #[test]
    fn loco_id_to_wire() {
        assert_eq!(LocoId::parse("S3").unwrap().to_wire().as_str(), "S3");
        assert_eq!(LocoId::parse("L341").unwrap().to_wire().as_str(), "L341");
    }
}
