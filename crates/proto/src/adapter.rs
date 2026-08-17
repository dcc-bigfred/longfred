//! Protocol adapter dispatch (WiThrottle / Z21).

use crate::command::ClientCommand;
use crate::events::ServerEvent;

pub type WireBuf = heapless::Vec<u8, 256>;

pub enum Adapter {
    Wt(crate::wt::WtAdapter),
    Z21(crate::z21::Z21Adapter),
}

impl Adapter {
    /// Bytes to send immediately after transport connect (handshake / broadcast flags).
    pub fn on_connect(&mut self, out: &mut WireBuf, emit: &mut dyn FnMut(ServerEvent)) {
        match self {
            Adapter::Wt(a) => a.on_connect(out, emit),
            Adapter::Z21(a) => a.on_connect(out, emit),
        }
    }

    /// Encode one semantic command; may emit local echo events (e.g. Z21 `AddressAdded`).
    pub fn encode(
        &mut self,
        cmd: &ClientCommand,
        out: &mut WireBuf,
        emit: &mut dyn FnMut(ServerEvent),
    ) {
        match self {
            Adapter::Wt(a) => a.encode(cmd, out, emit),
            Adapter::Z21(a) => a.encode(cmd, out, emit),
        }
    }

    /// Feed received bytes; emit decoded `ServerEvent`s.
    pub fn decode(&mut self, data: &[u8], emit: &mut dyn FnMut(ServerEvent)) {
        match self {
            Adapter::Wt(a) => a.decode(data, emit),
            Adapter::Z21(a) => a.decode(data, emit),
        }
    }

    /// Periodic keepalive / heartbeat; returns `true` if `out` was filled.
    pub fn on_tick(&mut self, out: &mut WireBuf) -> bool {
        match self {
            Adapter::Wt(a) => a.on_tick(out),
            Adapter::Z21(a) => a.on_tick(out),
        }
    }

    pub fn tick_period_s(&self) -> u32 {
        match self {
            Adapter::Wt(a) => a.heartbeat_period,
            Adapter::Z21(_) => 30,
        }
    }

    pub fn set_heartbeat_period(&mut self, seconds: u32) {
        if let Adapter::Wt(a) = self {
            a.heartbeat_period = seconds.max(1);
        }
    }

    /// Capabilities of the live adapter. BigFred still rides `Wt` until its own type exists.
    #[must_use]
    pub fn caps(&self) -> crate::caps::ProtocolCaps {
        match self {
            Adapter::Wt(_) => crate::command::Protocol::WiThrottle.caps(),
            Adapter::Z21(_) => crate::command::Protocol::Z21.caps(),
        }
    }
}
