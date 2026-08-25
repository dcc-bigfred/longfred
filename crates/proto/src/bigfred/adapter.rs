//! BigFred protocol adapter: WiThrottle drive traffic plus handset pairing.

use crate::adapter::WireBuf;
use crate::command::{ClientCommand, LocoId};
use crate::events::ServerEvent;
use crate::model::ShortText;
use crate::withrottle::WtAdapter;

pub const PAIRING_SENTINEL_NAME: &str = "Pair with BigFred";
pub const PAIRING_SENTINEL_ADDR: u16 = 3;
pub const PAIRING_CODE_LEN: usize = 6;
const PAIRING_TIMEOUT_TICKS: u8 = 60; // 15 s at firmware's 250 ms session tick.

#[derive(Clone, Debug, PartialEq, Eq)]
enum PairState {
    Idle,
    Sending {
        code: heapless::String<PAIRING_CODE_LEN>,
        step: u8,
    },
    AwaitingResult {
        ticks: u8,
    },
    ReleaseSentinel,
}

/// BigFred composes the regular WiThrottle adapter and owns pairing state.
pub struct BigFredAdapter {
    inner: WtAdapter,
    pairing: PairState,
}

impl BigFredAdapter {
    #[must_use]
    pub fn new(
        name: &str,
        id: &str,
        hb_period: u32,
        send_leading_crlf: bool,
        dead_man_switch_on: bool,
    ) -> Self {
        Self {
            inner: WtAdapter::new(name, id, hb_period, send_leading_crlf, dead_man_switch_on),
            pairing: PairState::Idle,
        }
    }

    pub fn on_connect(&mut self, out: &mut WireBuf, emit: &mut dyn FnMut(ServerEvent)) {
        self.inner.on_connect(out, emit);
    }

    pub fn on_disconnect(&mut self, out: &mut WireBuf) {
        self.inner.on_disconnect(out);
    }

    pub fn on_unpair(&mut self, out: &mut WireBuf) {
        self.inner.on_unpair(out);
    }

    pub fn encode(
        &mut self,
        cmd: &ClientCommand,
        out: &mut WireBuf,
        emit: &mut dyn FnMut(ServerEvent),
    ) {
        let ClientCommand::Pair { code } = cmd else {
            self.inner.encode(cmd, out, emit);
            return;
        };
        if code.len() != PAIRING_CODE_LEN || !code.as_bytes().iter().all(u8::is_ascii_digit) {
            emit(ServerEvent::PairingFailed);
            return;
        }
        self.pairing = PairState::Sending {
            code: code.clone(),
            step: 0,
        };
        let mut name = ShortText::new();
        let _ = name.push_str(PAIRING_SENTINEL_NAME);
        self.inner.encode(
            &ClientCommand::AddLoco {
                throttle: 0,
                loco: LocoId {
                    addr: PAIRING_SENTINEL_ADDR,
                    long: false,
                },
                name,
            },
            out,
            emit,
        );
    }

    pub fn decode(&mut self, data: &[u8], emit: &mut dyn FnMut(ServerEvent)) {
        let pairing = &mut self.pairing;
        self.inner.decode(data, &mut |event| match &event {
            ServerEvent::RosterEntry { name, .. } if name.as_str() == PAIRING_SENTINEL_NAME => {
                emit(event);
                emit(ServerEvent::PairingRequired);
            }
            ServerEvent::Message(message) if message.as_str().starts_with("Paired as ") => {
                *pairing = PairState::ReleaseSentinel;
                let mut user = ShortText::new();
                let _ = user.push_str(message.as_str().trim_start_matches("Paired as "));
                emit(event);
                emit(ServerEvent::PairingSucceeded(user));
            }
            _ => emit(event),
        });
    }

    /// Advance one momentary function edge or pairing timeout tick.
    pub fn poll(&mut self, out: &mut WireBuf, emit: &mut dyn FnMut(ServerEvent)) -> bool {
        let mut next = None;
        match &mut self.pairing {
            PairState::Sending { code, step } => {
                let digit_index = usize::from(*step / 2);
                let Some(digit) = code
                    .as_bytes()
                    .get(digit_index)
                    .map(|b| b.saturating_sub(b'0'))
                else {
                    self.pairing = PairState::Idle;
                    emit(ServerEvent::PairingFailed);
                    return false;
                };
                let on = *step % 2 == 0;
                self.inner.encode_function(0, digit, on, false, out);
                *step += 1;
                if usize::from(*step) == PAIRING_CODE_LEN * 2 {
                    next = Some(PairState::AwaitingResult { ticks: 0 });
                }
            }
            PairState::AwaitingResult { ticks } => {
                *ticks = ticks.saturating_add(1);
                if *ticks >= PAIRING_TIMEOUT_TICKS {
                    next = Some(PairState::Idle);
                    emit(ServerEvent::PairingFailed);
                }
            }
            PairState::ReleaseSentinel => {
                self.inner
                    .encode(&ClientCommand::ReleaseThrottle { throttle: 0 }, out, emit);
                next = Some(PairState::Idle);
            }
            PairState::Idle => {}
        }
        if let Some(state) = next {
            self.pairing = state;
        }
        !out.is_empty()
    }

    pub fn on_tick(&mut self, out: &mut WireBuf) -> bool {
        self.inner.on_tick(out)
    }

    #[must_use]
    pub fn tick_period_s(&self) -> u32 {
        self.inner.heartbeat_period
    }

    pub fn set_heartbeat_period(&mut self, seconds: u32) {
        self.inner.heartbeat_period = seconds.max(1);
    }
}

#[cfg(test)]
mod tests {
    use core::fmt::Write as _;

    use super::*;

    fn adapter() -> BigFredAdapter {
        BigFredAdapter::new("Handset", "4242", 10, false, true)
    }

    #[test]
    fn pair_acquires_sentinel_then_sends_momentary_digits() {
        let mut adapter = adapter();
        let mut code = heapless::String::new();
        let _ = code.push_str("120945");
        let mut out = WireBuf::new();
        adapter.encode(&ClientCommand::Pair { code }, &mut out, &mut |_| {});
        assert!(core::str::from_utf8(&out).is_ok_and(|s| s.contains("M0+S3")));

        let expected = [(1, true), (1, false), (2, true), (2, false)];
        for (func, on) in expected {
            let mut out = WireBuf::new();
            assert!(adapter.poll(&mut out, &mut |_| {}));
            let marker = if on { "F1" } else { "F0" };
            let suffix = format_func(func);
            assert!(
                core::str::from_utf8(&out)
                    .is_ok_and(|s| s.contains(marker) && s.ends_with(suffix.as_str()))
            );
        }
    }

    #[test]
    fn decode_does_not_truncate_multi_event_roster_line() {
        let mut adapter = adapter();
        let mut wire = heapless::String::<256>::new();
        let _ = wire.push_str("RL17");
        for address in 1..=17 {
            let _ = write!(wire, "]\\[A}}|{{{address}}}|{{S");
        }
        let _ = wire.push('\n');
        let mut events = 0usize;
        adapter.decode(wire.as_bytes(), &mut |_| events += 1);
        assert_eq!(events, 18);
    }

    fn format_func(func: u8) -> heapless::String<4> {
        let mut out = heapless::String::new();
        let _ = out.push((b'0' + func) as char);
        let _ = out.push('\r');
        let _ = out.push('\n');
        out
    }

    #[test]
    fn sentinel_and_success_become_typed_events() {
        let mut adapter = adapter();
        let mut events = heapless::Vec::<ServerEvent, 8>::new();
        adapter.decode(
            b"RL1]\\[Pair with BigFred}|{3}|{S\r\nHmPaired as ops\r\n",
            &mut |e| {
                let _ = events.push(e);
            },
        );
        assert!(events.iter().any(|e| *e == ServerEvent::PairingRequired));
        assert!(
            events.iter().any(
                |e| matches!(e, ServerEvent::PairingSucceeded(user) if user.as_str() == "ops")
            )
        );
    }

    #[test]
    fn invalid_code_fails_without_wire_output() {
        let mut adapter = adapter();
        let mut code = heapless::String::new();
        let _ = code.push_str("12x");
        let mut out = WireBuf::new();
        let mut failed = false;
        adapter.encode(&ClientCommand::Pair { code }, &mut out, &mut |e| {
            failed |= e == ServerEvent::PairingFailed
        });
        assert!(out.is_empty());
        assert!(failed);
    }
}
