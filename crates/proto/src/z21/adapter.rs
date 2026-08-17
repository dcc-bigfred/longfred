//! Z21 LAN protocol adapter (UDP X-BUS, host-testable).

use crate::adapter::WireBuf;
use crate::command::{ClientCommand, LocoId};
use crate::events::ServerEvent;
use crate::model::{Direction, LocoAddr, LongText, TrackPower, throttle_char};

const HDR_XBUS: u16 = 0x0040;
const MAX_LOCOS: usize = 16;

#[derive(Clone, Copy)]
struct Slot {
    throttle: u8,
    addr: u16,
    long: bool,
    steps: u8,
    speed: u8,
    dir: Direction,
    funcs: u32,
}

#[derive(Default)]
pub struct Z21Adapter {
    locos: heapless::Vec<Slot, MAX_LOCOS>,
}

fn xor_sum(x: &[u8]) -> u8 {
    x.iter().fold(0, |a, b| a ^ b)
}

fn put_frame(out: &mut WireBuf, header: u16, data: &[u8]) {
    let len = (4 + data.len()) as u16;
    let _ = out.extend_from_slice(&len.to_le_bytes());
    let _ = out.extend_from_slice(&header.to_le_bytes());
    let _ = out.extend_from_slice(data);
}

fn put_xbus(out: &mut WireBuf, xbus: &[u8]) {
    let len = (4 + xbus.len() + 1) as u16;
    let _ = out.extend_from_slice(&len.to_le_bytes());
    let _ = out.extend_from_slice(&HDR_XBUS.to_le_bytes());
    let _ = out.extend_from_slice(xbus);
    let _ = out.push(xor_sum(xbus));
}

pub fn addr_bytes(addr: u16, long: bool) -> [u8; 2] {
    let mut msb = ((addr >> 8) & 0x3F) as u8;
    if long || addr >= 128 {
        msb |= 0xC0;
    }
    [msb, (addr & 0xFF) as u8]
}

fn steps_db0(steps: u8) -> u8 {
    match steps {
        14 => 0x10,
        28 => 0x12,
        _ => 0x13,
    }
}

/// Domain speed 0..=126 (128-step UI scale) → Z21 DB3 for the given speed-step mode.
pub fn encode_db3(speed: u8, dir: Direction, steps: u8) -> u8 {
    let r = if dir == Direction::Forward {
        0x80
    } else {
        0x00
    };
    if speed == 0 {
        return r;
    }
    match steps {
        14 => {
            let v = ((speed as u16 * 14 / 126) as u8 + 1).min(15);
            r | v
        }
        28 => r | encode28(speed),
        _ => {
            let v = (speed as u16 + 1).min(127) as u8;
            r | v
        }
    }
}

fn encode28(domain_speed: u8) -> u8 {
    if domain_speed == 0 {
        return 0;
    }
    let s = ((domain_speed as u16 * 28 / 126) as u8).max(1).min(28);
    let speed_bits = (s + 3) / 2;
    let speed_bit5 = (s + 3) % 2;
    (speed_bit5 << 4) | (speed_bits & 0x0F)
}

/// Decode DB2/DB3 from `LAN_X_LOCO_INFO` into domain speed + direction.
pub fn decode_db3(db2: u8, db3: u8) -> (u8, Direction) {
    let forward = if db3 & 0x80 != 0 {
        Direction::Forward
    } else {
        Direction::Reverse
    };
    let v = db3 & 0x7F;
    match db2 & 0x07 {
        0 => {
            let raw = v & 0x0F;
            let speed = if raw <= 1 {
                0
            } else {
                ((raw as u16 - 1) * 126 / 14).min(126) as u8
            };
            (speed, forward)
        }
        2 => {
            let speed_bits = v & 0x0F;
            let speed_bit5 = (v >> 4) & 0x01;
            let raw = speed_bits * 2 + speed_bit5;
            let speed = match raw {
                0 | 1 => 0,
                2 | 3 => 0,
                r => ((r - 3) as u16 * 126 / 28).min(126) as u8,
            };
            (speed, forward)
        }
        _ => {
            let speed = if v <= 1 { 0 } else { (v - 1).min(126) };
            (speed, forward)
        }
    }
}

fn loco_addr_str(loco: LocoId) -> LocoAddr {
    let mut a = LocoAddr::new();
    let _ = a.push_str(loco.to_wire().as_str());
    a
}

fn empty_entry() -> LongText {
    LongText::new()
}

/// Decode the optional DB4..DB8 function bytes from LAN_X_LOCO_INFO.
/// Returns `(states, present)` so shorter legacy replies do not clear unknown functions.
fn decode_function_bits(x: &[u8]) -> (u32, u32) {
    let mut states = 0u32;
    let mut present = 0u32;
    if let Some(&db4) = x.get(5) {
        present |= 0x1F;
        if db4 & 0x10 != 0 {
            states |= 1;
        }
        states |= u32::from(db4 & 0x0F) << 1;
    }
    for (index, first_func) in [(6usize, 5u8), (7, 13), (8, 21)] {
        if let Some(&byte) = x.get(index) {
            let mask = 0xFFu32 << first_func;
            present |= mask;
            states |= u32::from(byte) << first_func;
        }
    }
    if let Some(&db8) = x.get(9) {
        present |= 0x07u32 << 29;
        states |= u32::from(db8 & 0x07) << 29;
    }
    (states, present)
}

impl Z21Adapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_connect(&mut self, out: &mut WireBuf, _emit: &mut dyn FnMut(ServerEvent)) {
        put_frame(out, 0x0050, &0x0000_0001u32.to_le_bytes());
        put_xbus(out, &[0x21, 0x24]);
        let subs: heapless::Vec<Slot, MAX_LOCOS> = self.locos.clone();
        for s in subs {
            let a = addr_bytes(s.addr, s.long);
            put_xbus(out, &[0xE3, 0xF0, a[0], a[1]]);
        }
    }

    pub fn encode(
        &mut self,
        cmd: &ClientCommand,
        out: &mut WireBuf,
        emit: &mut dyn FnMut(ServerEvent),
    ) {
        match cmd {
            ClientCommand::AddLoco { throttle, loco, .. } => {
                if !self
                    .locos
                    .iter()
                    .any(|slot| slot.throttle == *throttle && slot.addr == loco.addr)
                {
                    let _ = self.locos.push(Slot {
                        throttle: *throttle,
                        addr: loco.addr,
                        long: loco.long,
                        steps: 128,
                        speed: 0,
                        dir: Direction::Forward,
                        funcs: 0,
                    });
                }
                let a = addr_bytes(loco.addr, loco.long);
                put_xbus(out, &[0xE3, 0xF0, a[0], a[1]]);
                emit(ServerEvent::AddressAdded {
                    throttle: throttle_char(*throttle as usize),
                    addr: loco_addr_str(*loco),
                    entry: empty_entry(),
                });
            }
            ClientCommand::ReleaseThrottle { throttle } => {
                let removed: heapless::Vec<Slot, MAX_LOCOS> = self
                    .locos
                    .iter()
                    .filter(|s| s.throttle == *throttle)
                    .copied()
                    .collect();
                for s in removed {
                    let a = addr_bytes(s.addr, s.long);
                    put_xbus(out, &[0xE3, 0x44, a[0], a[1]]);
                    let mut addr = LocoAddr::new();
                    let _ = addr.push_str(
                        LocoId {
                            addr: s.addr,
                            long: s.long,
                        }
                        .to_wire()
                        .as_str(),
                    );
                    emit(ServerEvent::AddressRemoved {
                        throttle: throttle_char(s.throttle as usize),
                        addr,
                        entry: empty_entry(),
                    });
                }
                self.locos.retain(|s| s.throttle != *throttle);
            }
            ClientCommand::SetSpeed { throttle, speed } => {
                self.drive(*throttle, None, Some(*speed), None, out);
            }
            ClientCommand::SetDirection {
                throttle,
                loco,
                dir,
            } => {
                self.drive(*throttle, *loco, None, Some(*dir), out);
            }
            ClientCommand::EStop { throttle } => {
                for s in self.locos.iter_mut().filter(|s| s.throttle == *throttle) {
                    s.speed = 0;
                    let a = addr_bytes(s.addr, s.long);
                    put_xbus(out, &[0x92, a[0], a[1]]);
                    let direction = if s.dir == Direction::Forward {
                        0x80
                    } else {
                        0x00
                    };
                    put_xbus(
                        out,
                        &[0xE4, steps_db0(s.steps), a[0], a[1], direction | 0x01],
                    );
                }
            }
            ClientCommand::SetFunction {
                throttle, func, on, ..
            } => {
                for s in self.locos.iter_mut().filter(|s| s.throttle == *throttle) {
                    let a = addr_bytes(s.addr, s.long);
                    let tt = if *on { 0x40 } else { 0x00 };
                    put_xbus(out, &[0xE4, 0xF8, a[0], a[1], tt | (func & 0x3F)]);
                    if *on {
                        s.funcs |= 1u32 << func;
                    } else {
                        s.funcs &= !(1u32 << func);
                    }
                }
            }
            ClientCommand::TrackPower(on) => {
                put_xbus(out, &[0x21, if *on { 0x81 } else { 0x80 }]);
            }
            ClientCommand::SetHeartbeat(_)
            | ClientCommand::Steal { .. }
            | ClientCommand::Pair { .. } => {}
        }
    }

    fn drive(
        &mut self,
        throttle: u8,
        loco: Option<LocoId>,
        speed: Option<u8>,
        dir: Option<Direction>,
        out: &mut WireBuf,
    ) {
        for s in self.locos.iter_mut().filter(|s| s.throttle == throttle) {
            if let Some(target) = loco {
                if s.addr != target.addr {
                    continue;
                }
            }
            if let Some(v) = speed {
                s.speed = v;
            }
            if let Some(d) = dir {
                s.dir = d;
            }
            let a = addr_bytes(s.addr, s.long);
            put_xbus(
                out,
                &[
                    0xE4,
                    steps_db0(s.steps),
                    a[0],
                    a[1],
                    encode_db3(s.speed, s.dir, s.steps),
                ],
            );
        }
    }

    pub fn decode(&mut self, data: &[u8], emit: &mut dyn FnMut(ServerEvent)) {
        let mut b = data;
        while b.len() >= 4 {
            let len = u16::from_le_bytes([b[0], b[1]]) as usize;
            if len < 4 || len > b.len() {
                break;
            }
            let (frame, rest) = b.split_at(len);
            b = rest;
            if frame.len() < 4 {
                continue;
            }
            let header = u16::from_le_bytes([frame[2], frame[3]]);
            if header != HDR_XBUS || frame.len() < 5 {
                continue;
            }
            match frame[4] {
                0xEF => self.on_loco_info(&frame[4..frame.len() - 1], emit),
                0x61 if frame.len() >= 7 => match frame[5] {
                    0x00 => emit(ServerEvent::TrackPower(TrackPower::Off)),
                    0x01 => emit(ServerEvent::TrackPower(TrackPower::On)),
                    _ => {}
                },
                0x62 if frame.len() >= 8 && frame[5] == 0x22 => {
                    let power = if frame[6] & 0x02 != 0 {
                        TrackPower::Off
                    } else {
                        TrackPower::On
                    };
                    emit(ServerEvent::TrackPower(power));
                }
                0x81 => {
                    let mut msg = LongText::new();
                    let _ = msg.push_str("E-STOP");
                    emit(ServerEvent::Message(msg));
                }
                _ => {}
            }
        }
    }

    fn on_loco_info(&mut self, x: &[u8], emit: &mut dyn FnMut(ServerEvent)) {
        if x.len() < 6 {
            return;
        }
        let addr = ((x[1] as u16 & 0x3F) << 8) | x[2] as u16;
        let steps = match x[3] & 0x07 {
            0 => 14,
            2 => 28,
            _ => 128,
        };
        let (speed, dir) = decode_db3(x[3], x[4]);
        for s in self.locos.iter_mut().filter(|s| s.addr == addr) {
            s.steps = steps;
            s.speed = speed;
            s.dir = dir;
            let t = throttle_char(s.throttle as usize);
            emit(ServerEvent::Speed { throttle: t, speed });
            emit(ServerEvent::DirectionLead { throttle: t, dir });
            let (functions, present) = decode_function_bits(x);
            for func in 0..32u8 {
                let mask = 1u32 << func;
                if present & mask != 0 {
                    let on = functions & mask != 0;
                    if on != ((s.funcs & mask) != 0) {
                        emit(ServerEvent::FunctionState {
                            throttle: t,
                            func,
                            on,
                        });
                        if on {
                            s.funcs |= mask;
                        } else {
                            s.funcs &= !mask;
                        }
                    }
                }
            }
        }
    }

    pub fn on_tick(&mut self, out: &mut WireBuf) -> bool {
        put_frame(out, 0x0085, &[]);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addr_bytes_short_and_long() {
        assert_eq!(addr_bytes(3, false), [0x00, 0x03]);
        assert_eq!(addr_bytes(31, false), [0x00, 0x1F]);
        assert_eq!(addr_bytes(128, false), [0xC0, 0x80]);
        assert_eq!(addr_bytes(9999, true), [0xC0 | 0x27, 0x0F]);
    }

    #[test]
    fn drive_stop_rev_128_golden() {
        let mut out = WireBuf::new();
        put_xbus(
            &mut out,
            &[
                0xE4,
                steps_db0(128),
                0x00,
                0x1F,
                encode_db3(0, Direction::Reverse, 128),
            ],
        );
        assert_eq!(
            out.as_slice(),
            &[0x0A, 0x00, 0x40, 0x00, 0xE4, 0x13, 0x00, 0x1F, 0x00, 0xE8]
        );
    }

    #[test]
    fn encode_db3_128_forward_speed() {
        assert_eq!(encode_db3(0, Direction::Forward, 128), 0x80);
        assert_eq!(encode_db3(50, Direction::Forward, 128), 0x80 | 51);
        assert_eq!(encode_db3(10, Direction::Reverse, 128), 11);
    }

    #[test]
    fn decode_db3_roundtrip_128() {
        for speed in [0u8, 10, 50, 126] {
            for forward in [true, false] {
                let dir = if forward {
                    Direction::Forward
                } else {
                    Direction::Reverse
                };
                let db3 = encode_db3(speed, dir, 128);
                let (got_speed, got_dir) = decode_db3(0x13, db3);
                assert_eq!(got_speed, speed, "speed {speed}");
                assert_eq!(got_dir, dir);
            }
        }
    }

    #[test]
    fn set_function_packet() {
        let mut out = WireBuf::new();
        put_xbus(&mut out, &[0xE4, 0xF8, 0x00, 0x1F, 0x40 | 5]);
        assert_eq!(out[9], 0xE4 ^ 0xF8 ^ 0x00 ^ 0x1F ^ 0x45);
    }

    #[test]
    fn loco_info_decodes_functions_f0_through_f31() {
        let x = [
            0xEF,
            0x00,
            0x03,
            0x04,
            0x80,
            0x10 | 0x01 | 0x08, // F0, F1, F4
            0x81,               // F5, F12
            0x00,
            0x80, // F28
            0x04, // F31
        ];
        let (states, present) = decode_function_bits(&x);
        for func in [0u8, 1, 4, 5, 12, 28, 31] {
            assert_ne!(states & (1u32 << func), 0, "F{func}");
        }
        assert_eq!(present, u32::MAX);
    }

    #[test]
    fn loco_info_ignores_smartsearch_and_double_traction_bits() {
        let x = [0xEF, 0x00, 0x03, 0x04, 0x80, 0x60];
        let (states, present) = decode_function_bits(&x);
        assert_eq!(states, 0);
        assert_eq!(present, 0x1F);
    }

    #[test]
    fn estop_clears_cached_speed_and_sends_spec_plus_drive_fallback() {
        let mut adapter = Z21Adapter::new();
        let _ = adapter.locos.push(Slot {
            throttle: 0,
            addr: 31,
            long: false,
            steps: 128,
            speed: 50,
            dir: Direction::Forward,
            funcs: 0,
        });
        let mut out = WireBuf::new();
        adapter.encode(&ClientCommand::EStop { throttle: 0 }, &mut out, &mut |_| {});

        assert_eq!(adapter.locos[0].speed, 0);
        assert_eq!(&out[..8], &[0x08, 0x00, 0x40, 0x00, 0x92, 0x00, 0x1F, 0x8D]);
        assert_eq!(out[12], 0xE4);
        assert_eq!(out[16] & 0x7F, 1);
    }

    #[test]
    fn estop_burst_fits_all_sixteen_locomotives() {
        let mut adapter = Z21Adapter::new();
        for addr in 1..=16 {
            let _ = adapter.locos.push(Slot {
                throttle: 0,
                addr,
                long: false,
                steps: 128,
                speed: 50,
                dir: Direction::Forward,
                funcs: 0,
            });
        }
        let mut out = WireBuf::new();
        adapter.encode(&ClientCommand::EStop { throttle: 0 }, &mut out, &mut |_| {});
        assert_eq!(out.len(), 16 * (8 + 10));
    }

    #[test]
    fn split_datagram_two_frames() {
        let mut adapter = Z21Adapter::new();
        adapter
            .locos
            .push(Slot {
                throttle: 0,
                addr: 31,
                long: false,
                steps: 128,
                speed: 0,
                dir: Direction::Forward,
                funcs: 0,
            })
            .ok();
        let mut events = heapless::Vec::<ServerEvent, 8>::new();
        let mut emit = |ev: ServerEvent| {
            let _ = events.push(ev);
        };
        let mut pkt = WireBuf::new();
        put_xbus(&mut pkt, &[0x61, 0x01]);
        put_xbus(&mut pkt, &[0xEF, 0x00, 0x1F, 0x13, 0x80 | 40, 0x00]);
        adapter.decode(pkt.as_slice(), &mut emit);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ServerEvent::TrackPower(TrackPower::On)))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ServerEvent::Speed { speed: 39, .. }))
        );
    }

    #[test]
    fn add_loco_emits_address_added() {
        let mut adapter = Z21Adapter::new();
        let mut out = WireBuf::new();
        let mut events = heapless::Vec::<ServerEvent, 4>::new();
        let mut emit = |ev: ServerEvent| {
            let _ = events.push(ev);
        };
        adapter.encode(
            &ClientCommand::AddLoco {
                throttle: 0,
                loco: LocoId {
                    addr: 31,
                    long: false,
                },
                name: heapless::String::new(),
            },
            &mut out,
            &mut emit,
        );
        assert!(!out.is_empty());
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ServerEvent::AddressAdded { .. }))
        );
    }

    #[test]
    fn add_loco_deduplicates_same_throttle_and_address() {
        let mut adapter = Z21Adapter::new();
        let mut out = WireBuf::new();
        let command = ClientCommand::AddLoco {
            throttle: 0,
            loco: LocoId {
                addr: 31,
                long: false,
            },
            name: heapless::String::new(),
        };
        adapter.encode(&command, &mut out, &mut |_| {});
        adapter.encode(&command, &mut out, &mut |_| {});
        assert_eq!(adapter.locos.len(), 1);
    }

    #[test]
    fn status_changed_updates_track_power() {
        let mut adapter = Z21Adapter::new();
        let mut packet = WireBuf::new();
        put_xbus(&mut packet, &[0x62, 0x22, 0x02]);
        let mut events = heapless::Vec::<ServerEvent, 2>::new();
        adapter.decode(packet.as_slice(), &mut |event| {
            let _ = events.push(event);
        });
        assert_eq!(
            events.first(),
            Some(&ServerEvent::TrackPower(TrackPower::Off))
        );
    }
}
