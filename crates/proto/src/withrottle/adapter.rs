//! WiThrottle protocol adapter (wraps `protocol` builders + line parser).

use super::{parser, protocol};
use crate::adapter::WireBuf;
use crate::command::ClientCommand;
use crate::events::ServerEvent;
use crate::model::throttle_char_u8;

pub struct WtAdapter {
    name: heapless::String<32>,
    id: heapless::String<32>,
    line: heapless::String<256>,
    pub heartbeat_period: u32,
    leading_crlf: bool,
    send_leading_crlf: bool,
    dead_man_switch_on: bool,
}

impl WtAdapter {
    pub fn new(
        name: &str,
        id: &str,
        hb_period: u32,
        send_leading_crlf: bool,
        dead_man_switch_on: bool,
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
            dead_man_switch_on,
        }
    }

    pub fn on_connect(&mut self, out: &mut WireBuf, _emit: &mut dyn FnMut(ServerEvent)) {
        self.push_line(out, &protocol::handshake_id(self.id.as_str()));
        self.push_line(out, &protocol::handshake_name(self.name.as_str()));
        self.push_line(out, &protocol::dead_man_switch_enable(self.dead_man_switch_on));
    }

    pub fn on_disconnect(&mut self, out: &mut WireBuf) {
        self.push_line(out, &protocol::quit());
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
                self.encode_function(*throttle, *func, *on, true, out);
            }
            ClientCommand::TrackPower(on) => {
                self.push_line(out, &protocol::track_power(*on));
            }
            ClientCommand::SetDeadManSwitch(on) => {
                self.dead_man_switch_on = *on;
                self.push_line(out, &protocol::dead_man_switch_enable(*on));
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

    pub(crate) fn encode_function(
        &mut self,
        throttle: u8,
        func: u8,
        on: bool,
        force: bool,
        out: &mut WireBuf,
    ) {
        self.push_line(
            out,
            &protocol::set_function(throttle_char_u8(throttle), "*", func, on, force),
        );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_identifies_device_before_sending_name() {
        let mut adapter = WtAdapter::new("WiFred", "device-1", 10, false, true);
        let mut out = WireBuf::new();
        adapter.on_connect(&mut out, &mut |_| {});
        assert_eq!(
            core::str::from_utf8(out.as_slice()),
            Ok("HUdevice-1\r\nNWiFred\r\n*+\r\n")
        );
    }

    #[test]
    fn function_commands_use_absolute_force_form() {
        let mut adapter = WtAdapter::new("", "", 10, false, true);
        let mut out = WireBuf::new();
        adapter.encode(
            &ClientCommand::SetFunction {
                throttle: 0,
                func: 5,
                on: true,
                all: true,
            },
            &mut out,
            &mut |_| {},
        );
        assert_eq!(core::str::from_utf8(out.as_slice()), Ok("M0A*<;>f15\r\n"));
    }

    #[test]
    fn disconnect_sends_quit() {
        let mut adapter = WtAdapter::new("", "", 10, false, true);
        let mut out = WireBuf::new();
        adapter.on_disconnect(&mut out);
        assert_eq!(core::str::from_utf8(out.as_slice()), Ok("Q\r\n"));
    }
}
