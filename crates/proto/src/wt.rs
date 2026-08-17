//! WiThrottle protocol adapter (wraps `protocol` builders + line parser).

use crate::adapter::WireBuf;
use crate::command::ClientCommand;
use crate::events::ServerEvent;
use crate::model::throttle_char_u8;
use crate::{parser, protocol};

pub struct WtAdapter {
    name: heapless::String<32>,
    id: heapless::String<32>,
    line: heapless::String<256>,
    pub heartbeat_period: u32,
    leading_crlf: bool,
    send_leading_crlf: bool,
    heartbeat_enabled: bool,
}

impl WtAdapter {
    pub fn new(
        name: &str,
        id: &str,
        hb_period: u32,
        send_leading_crlf: bool,
        heartbeat_enabled: bool,
    ) -> Self {
        let mut n = heapless::String::new();
        let _ = n.push_str(name);
        let mut i = heapless::String::new();
        let _ = i.push_str(id);
        Self {
            name: n,
            id: i,
            line: heapless::String::new(),
            heartbeat_period: hb_period.max(1),
            leading_crlf: false,
            send_leading_crlf,
            heartbeat_enabled,
        }
    }

    pub fn on_connect(&mut self, out: &mut WireBuf, _emit: &mut dyn FnMut(ServerEvent)) {
        self.push_line(out, &protocol::handshake_name(self.name.as_str()));
        self.push_line(out, &protocol::handshake_id(self.id.as_str()));
        self.push_line(out, &protocol::heartbeat_enable(self.heartbeat_enabled));
    }

    pub fn encode(
        &mut self,
        cmd: &ClientCommand,
        out: &mut WireBuf,
        _emit: &mut dyn FnMut(ServerEvent),
    ) {
        match cmd {
            ClientCommand::AddLoco {
                throttle,
                loco,
                name,
            } => {
                let a = loco.to_wire();
                self.push_line(
                    out,
                    &protocol::add_loco(throttle_char_u8(*throttle), a.as_str(), name.as_str()),
                );
            }
            ClientCommand::ReleaseThrottle { throttle } => self.push_line(
                out,
                &protocol::release_loco(throttle_char_u8(*throttle), "*"),
            ),
            ClientCommand::SetSpeed { throttle, speed } => self.push_line(
                out,
                &protocol::set_speed(throttle_char_u8(*throttle), *speed),
            ),
            ClientCommand::SetDirection {
                throttle,
                loco,
                dir,
            } => {
                let owned = loco.map(|l| l.to_wire());
                let addr = owned.as_ref().map(|s| s.as_str()).unwrap_or("*");
                self.push_line(
                    out,
                    &protocol::set_direction(throttle_char_u8(*throttle), addr, *dir),
                );
            }
            ClientCommand::EStop { throttle } => {
                self.push_line(out, &protocol::estop(throttle_char_u8(*throttle), "*"));
            }
            ClientCommand::SetFunction {
                throttle, func, on, ..
            } => {
                self.push_line(
                    out,
                    &protocol::set_function(throttle_char_u8(*throttle), "*", *func, *on, false),
                );
            }
            ClientCommand::TrackPower(on) => {
                self.push_line(out, &protocol::track_power(*on));
            }
            ClientCommand::SetHeartbeat(on) => {
                self.heartbeat_enabled = *on;
                self.push_line(out, &protocol::heartbeat_enable(*on));
            }
            ClientCommand::Steal { throttle, loco } => {
                let a = loco.to_wire();
                self.push_line(
                    out,
                    &protocol::steal_loco(throttle_char_u8(*throttle), a.as_str()),
                );
            }
            ClientCommand::Pair { .. } => {}
        }
    }

    pub fn decode(&mut self, data: &[u8], emit: &mut dyn FnMut(ServerEvent)) {
        for &b in data {
            if b == b'\n' {
                let s = self.line.as_str().trim_end_matches(['\r', '\n']);
                parser::parse(s, |ev| {
                    if let ServerEvent::HeartbeatConfig { seconds } = &ev {
                        self.heartbeat_period = (*seconds).max(1);
                    }
                    emit(ev);
                });
                self.line.clear();
            } else if b != b'\r' {
                if self.line.push(b as char).is_err() {
                    self.line.clear();
                }
            }
        }
    }

    pub fn on_tick(&mut self, out: &mut WireBuf) -> bool {
        self.push_line(out, &protocol::heartbeat());
        true
    }

    fn push_line(&mut self, out: &mut WireBuf, cmd: &protocol::Cmd) {
        if self.send_leading_crlf && !self.leading_crlf {
            let _ = out.extend_from_slice(b"\r\n");
            self.leading_crlf = true;
        }
        let _ = out.extend_from_slice(cmd.as_bytes());
        let _ = out.extend_from_slice(b"\r\n");
    }
}
