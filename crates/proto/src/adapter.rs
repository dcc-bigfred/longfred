//! Protocol adapter dispatch (WiThrottle / Z21 / BigFred).

use crate::command::ClientCommand;
use crate::events::ServerEvent;

pub type WireBuf = heapless::Vec<u8, 256>;

pub enum Adapter {
    Wt(crate::withrottle::WtAdapter),
    Z21(crate::z21::Z21Adapter),
    BigFred(crate::bigfred::BigFredAdapter),
}

impl Adapter {
    /// Bytes to send immediately after transport connect (handshake / broadcast flags).
    pub fn on_connect(&mut self, out: &mut WireBuf, emit: &mut dyn FnMut(ServerEvent)) {
        match self {
            Adapter::Wt(a) => a.on_connect(out, emit),
            Adapter::Z21(a) => a.on_connect(out, emit),
            Adapter::BigFred(a) => a.on_connect(out, emit),
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
            Adapter::BigFred(a) => a.encode(cmd, out, emit),
        }
    }

    /// Feed received bytes; emit decoded `ServerEvent`s.
    pub fn decode(&mut self, data: &[u8], emit: &mut dyn FnMut(ServerEvent)) {
        match self {
            Adapter::Wt(a) => a.decode(data, emit),
            Adapter::Z21(a) => a.decode(data, emit),
            Adapter::BigFred(a) => a.decode(data, emit),
        }
    }

    /// Advance protocol state that needs the firmware's fixed session cadence.
    pub fn poll(&mut self, out: &mut WireBuf, emit: &mut dyn FnMut(ServerEvent)) -> bool {
        match self {
            Adapter::BigFred(a) => a.poll(out, emit),
            Adapter::Wt(_) | Adapter::Z21(_) => false,
        }
    }

    /// Periodic keepalive / heartbeat; returns `true` if `out` was filled.
    pub fn on_tick(&mut self, out: &mut WireBuf) -> bool {
        match self {
            Adapter::Wt(a) => a.on_tick(out),
            Adapter::Z21(a) => a.on_tick(out),
            Adapter::BigFred(a) => a.on_tick(out),
        }
    }

    pub fn tick_period_s(&self) -> u32 {
        match self {
            Adapter::Wt(a) => a.heartbeat_period,
            Adapter::Z21(_) => 30,
            Adapter::BigFred(a) => a.tick_period_s(),
        }
    }

    pub fn set_heartbeat_period(&mut self, seconds: u32) {
        match self {
            Adapter::Wt(a) => a.heartbeat_period = seconds.max(1),
            Adapter::BigFred(a) => a.set_heartbeat_period(seconds),
            Adapter::Z21(_) => {}
        }
    }

    /// Capabilities of the live adapter.
    #[must_use]
    pub fn caps(&self) -> crate::caps::ProtocolCaps {
        use crate::caps::ProtocolSpec;
        match self {
            Adapter::Wt(_) => <crate::withrottle::WiThrottle as ProtocolSpec>::INFO.caps,
            Adapter::Z21(_) => <crate::z21::Z21 as ProtocolSpec>::INFO.caps,
            Adapter::BigFred(_) => <crate::bigfred::BigFred as ProtocolSpec>::INFO.caps,
        }
    }
}
