//! Domain state plus reduction of server events and menu intents.

use embassy_time::{Duration, Instant};
use log::warn;
use longfred_proto::command::{ClientCommand, LocoId};
use longfred_proto::events::ServerEvent;
use longfred_proto::model::{Direction, LocoAddr, LongText, ShortText, TrackPower};
use longfred_proto::persist::{MAX_SAVED_LOCOS, PersistRecord, SavedLoco};

use crate::config::{self, buttons, network, sizes};
use crate::domain::actions::Action;
use crate::domain::model::{
    self, MAX_SPEED, RosterEntry, SHORT_DCC_ADDRESS_LIMIT, ThrottleSlot, throttle_char,
    throttle_index,
};
use crate::ui::i18n;

pub const CMD_BUF: usize = 12;
const SPEED_ECHO_DEBOUNCE_MS: u64 = 500;

pub struct DomainState {
    pub throttles: [ThrottleSlot; config::sizes::MAX_THROTTLES],
    pub current: usize,
    pub max_throttles: usize,
    pub track_power: TrackPower,
    pub speed_multiplier: u8,
    pub roster: heapless::Vec<RosterEntry, { sizes::MAX_ROSTER }>,
    pub roster_count: u16,
    message: Option<(LongText, Instant)>,
    pub heartbeat_on: bool,
    pub drop_before_acquire: bool,
    pub persist: PersistRecord,
    last_speed_sent: u8,
    last_speed_throttle: usize,
    last_speed_sent_at: Option<Instant>,
    /// Coalesced speed not yet sent (throttle index, speed); overwrites cancel earlier values.
    pending_speed: Option<(usize, u8)>,
}

impl DomainState {
    pub fn new() -> Self {
        let speed_step = buttons::SPEED_STEP;
        Self {
            throttles: core::array::from_fn(|_| ThrottleSlot::new(speed_step)),
            current: 0,
            max_throttles: buttons::DEFAULT_THROTTLES,
            track_power: TrackPower::Unknown,
            speed_multiplier: 1,
            roster: heapless::Vec::new(),
            roster_count: 0,
            message: None,
            heartbeat_on: buttons::HEARTBEAT_ENABLED,
            drop_before_acquire: buttons::DROP_BEFORE_ACQUIRE,
            persist: PersistRecord::default(),
            last_speed_sent: 0,
            last_speed_throttle: 0,
            last_speed_sent_at: None,
            pending_speed: None,
        }
    }

    pub fn current_slot(&self) -> &ThrottleSlot {
        &self.throttles[self.current]
    }

    pub fn current_slot_has_loco(&self) -> bool {
        self.current_slot().has_loco()
    }

    pub fn current_forward(&self) -> bool {
        self.current_slot().direction == Direction::Forward
    }

    pub fn track_power_on(&self) -> bool {
        model::track_power_on(self.track_power)
    }

    pub fn heartbeat_enabled(&self) -> bool {
        self.heartbeat_on
    }

    pub fn active_broadcast(&self) -> Option<&str> {
        let (msg, at) = self.message.as_ref()?;
        if at.elapsed().as_millis() > i18n::BROADCAST_TIMEOUT_MS {
            return None;
        }
        Some(msg.as_str())
    }

    fn current_slot_mut(&mut self) -> &mut ThrottleSlot {
        &mut self.throttles[self.current]
    }

    pub fn apply_action(
        &mut self,
        action: Action,
        pressed: bool,
        out: &mut heapless::Vec<ClientCommand, CMD_BUF>,
    ) -> bool {
        match action {
            Action::None => false,
            Action::Function(f) => {
                if pressed {
                    self.toggle_function(f, out)
                } else {
                    false
                }
            }
            Action::SpeedStop => self.speed_set(0, out),
            Action::SpeedUp => self.speed_up(false, out),
            Action::SpeedDown => self.speed_down(false, out),
            Action::SpeedUpFast => self.speed_up(true, out),
            Action::SpeedDownFast => self.speed_down(true, out),
            Action::SpeedMultiplier => {
                self.cycle_speed_multiplier();
                true
            }
            Action::SpeedStopThenToggleDirection => self.stop_then_toggle_direction(out),
            Action::EStop => self.estop_all(out),
            Action::EStopCurrentLoco => self.estop_current(out),
            Action::DirectionToggle => {
                let dir = opposite_slot_direction(self.current_slot().direction);
                self.change_direction(self.current, dir, out)
            }
            Action::DirectionForward => {
                self.change_direction(self.current, Direction::Forward, out)
            }
            Action::DirectionReverse => {
                self.change_direction(self.current, Direction::Reverse, out)
            }
            Action::MaxThrottleIncrease => {
                if self.max_throttles < config::sizes::MAX_THROTTLES {
                    self.max_throttles += 1;
                }
                true
            }
            Action::MaxThrottleDecrease => {
                if self.max_throttles > 1 {
                    let idx = self.max_throttles - 1;
                    self.release_throttle(idx, out);
                    self.max_throttles -= 1;
                    if self.current >= self.max_throttles {
                        self.current = self.max_throttles - 1;
                    }
                }
                true
            }
            Action::PowerToggle => {
                let on = !matches!(self.track_power, TrackPower::On);
                self.set_track_power(on, out)
            }
            Action::PowerOn => self.set_track_power(true, out),
            Action::PowerOff => self.set_track_power(false, out),
            Action::NextThrottle => {
                self.current = (self.current + 1) % self.max_throttles;
                true
            }
            Action::Throttle(n) if n >= 1 && (n as usize) <= self.max_throttles => {
                self.current = n as usize - 1;
                true
            }
            Action::ShowHideBattery | Action::Sleep => false,
            Action::Custom(n) => {
                warn!("custom command {} not configured", n);
                false
            }
            _ => false,
        }
    }

    pub fn acquire_addr(
        &mut self,
        digits: &str,
        out: &mut heapless::Vec<ClientCommand, CMD_BUF>,
    ) -> bool {
        if digits.is_empty() {
            return false;
        }
        let Some(loco_id) = parse_acquire_addr(digits) else {
            return false;
        };
        if self.drop_before_acquire {
            push_cmd(
                out,
                ClientCommand::ReleaseThrottle {
                    throttle: self.current as u8,
                },
            );
            self.clear_consist(self.current);
        }
        let loco_str = loco_id.to_wire();
        let mut name = ShortText::new();
        let _ = name.push_str(loco_str.as_str());
        push_cmd(
            out,
            ClientCommand::AddLoco {
                throttle: self.current as u8,
                loco: loco_id,
                name,
            },
        );
        true
    }

    pub fn acquire_roster(
        &mut self,
        index: usize,
        out: &mut heapless::Vec<ClientCommand, CMD_BUF>,
    ) -> bool {
        let Some(entry) = self.roster.get(index) else {
            return false;
        };
        let mut digits = heapless::String::<8>::new();
        let _ = write_roster_addr(entry.address, entry.length, &mut digits);
        self.acquire_addr(digits.as_str(), out)
    }

    pub fn release_all(&mut self, out: &mut heapless::Vec<ClientCommand, CMD_BUF>) -> bool {
        if !self.current_slot().has_loco() {
            return false;
        }
        push_cmd(
            out,
            ClientCommand::ReleaseThrottle {
                throttle: self.current as u8,
            },
        );
        self.clear_consist(self.current);
        self.current_slot_mut().speed = 0;
        true
    }

    /// Press toggles the function on or off. Key release is ignored by callers.
    pub fn toggle_function(
        &mut self,
        func: u8,
        out: &mut heapless::Vec<ClientCommand, CMD_BUF>,
    ) -> bool {
        let on = !self.function_latched(func);
        self.set_function(func, on, out)
    }

    fn function_latched(&self, func: u8) -> bool {
        self.current_slot()
            .functions
            .get(func as usize)
            .copied()
            .unwrap_or(false)
    }

    pub fn toggle_heartbeat(&mut self, out: &mut heapless::Vec<ClientCommand, CMD_BUF>) -> bool {
        self.heartbeat_on = !self.heartbeat_on;
        push_cmd(out, ClientCommand::SetHeartbeat(self.heartbeat_on));
        true
    }

    pub fn toggle_drop_before_acquire(&mut self) {
        self.drop_before_acquire = !self.drop_before_acquire;
    }

    pub fn show_message(&mut self, text: &str) {
        let mut msg = LongText::new();
        let _ = msg.push_str(text);
        self.message = Some((msg, Instant::now()));
    }

    pub fn load_persist(&mut self, rec: PersistRecord) {
        self.persist = rec;
    }

    pub fn collect_saved_locos(&self) -> heapless::Vec<SavedLoco, MAX_SAVED_LOCOS> {
        let mut out = heapless::Vec::new();
        for (ti, slot) in self.throttles.iter().enumerate().take(self.max_throttles) {
            for (si, loco) in slot.consist.iter().enumerate() {
                let mut entry = SavedLoco {
                    throttle: throttle_char(ti) as u8,
                    slot: si as u8,
                    addr: heapless::String::new(),
                };
                let s = loco.as_str();
                let digits = if s.len() > 1 && (s.starts_with('S') || s.starts_with('L')) {
                    &s[1..]
                } else {
                    s
                };
                let _ = entry.addr.push_str(digits);
                let _ = out.push(entry);
            }
        }
        out
    }

    pub fn restore_locos(&mut self, out: &mut heapless::Vec<ClientCommand, CMD_BUF>) {
        if !network::RESTORE_ACQUIRED_LOCOS {
            return;
        }
        for loco in &self.persist.locos {
            let idx = (loco.throttle as u8).saturating_sub(b'0') as usize;
            if idx >= config::sizes::MAX_THROTTLES {
                continue;
            }
            if let Some(addr) = build_loco_addr(loco.addr.as_str()) {
                let Some(loco_id) = LocoId::parse(addr.as_str()) else {
                    continue;
                };
                let a = addr.as_str();
                let mut name = ShortText::new();
                let _ = name.push_str(a);
                push_cmd(
                    out,
                    ClientCommand::AddLoco {
                        throttle: loco.throttle,
                        loco: loco_id,
                        name,
                    },
                );
            }
        }
        self.current = 0;
    }

    pub fn apply_event(
        &mut self,
        ev: ServerEvent,
        out: &mut heapless::Vec<ClientCommand, CMD_BUF>,
    ) -> bool {
        match ev {
            ServerEvent::AddressAdded { throttle, addr, .. } => {
                self.on_address_added(throttle, addr)
            }
            ServerEvent::AddressRemoved { throttle, addr, .. } => {
                self.on_address_removed(throttle, addr)
            }
            ServerEvent::Speed { throttle, speed } => self.on_speed_echo(throttle, speed),
            ServerEvent::DirectionLead { throttle, dir } => self.on_direction_lead(throttle, dir),
            ServerEvent::DirectionLoco {
                throttle,
                addr,
                dir,
            } => self.on_direction_loco(throttle, addr, dir),
            ServerEvent::FunctionState { throttle, func, on } => {
                self.on_function_state(throttle, func, on)
            }
            ServerEvent::RosterFunctionLabels { throttle, labels } => {
                self.on_roster_function_labels(throttle, labels)
            }
            ServerEvent::TrackPower(tp) => {
                self.track_power = tp;
                true
            }
            ServerEvent::RosterEntriesCount(n) => {
                self.roster_count = n;
                self.roster.clear();
                true
            }
            ServerEvent::RosterEntry {
                index,
                name,
                address,
                length,
            } => self.on_roster_entry(index, name, address, length),
            ServerEvent::Message(text) => self.on_broadcast(text),
            ServerEvent::Alert(text) => self.on_alert(text),
            ServerEvent::StealNeeded { throttle, addr, .. } => {
                let idx = throttle_index(throttle).unwrap_or(self.current);
                push_cmd(
                    out,
                    ClientCommand::Steal {
                        throttle: idx as u8,
                        loco: LocoId::parse(addr.as_str()).unwrap_or(LocoId {
                            addr: 0,
                            long: false,
                        }),
                    },
                );
                false
            }
            ServerEvent::HeartbeatConfig { .. }
            | ServerEvent::Version(_)
            | ServerEvent::ServerType(_)
            | ServerEvent::ServerDescription(_)
            | ServerEvent::WebPort(_)
            | ServerEvent::Unknown(_) => false,
        }
    }

    fn on_broadcast(&mut self, text: LongText) -> bool {
        let s = text.as_str();
        if s == "Connected" || s.starts_with("Connecting") {
            return false;
        }
        self.message = Some((text, Instant::now()));
        true
    }

    fn on_alert(&mut self, text: LongText) -> bool {
        if text.as_str().contains("steal") {
            return false;
        }
        self.message = Some((text, Instant::now()));
        true
    }

    fn release_throttle(&mut self, index: usize, out: &mut heapless::Vec<ClientCommand, CMD_BUF>) {
        if self.throttles[index].has_loco() {
            push_cmd(
                out,
                ClientCommand::ReleaseThrottle {
                    throttle: index as u8,
                },
            );
            self.clear_consist(index);
            self.throttles[index].speed = 0;
        }
    }

    fn clear_consist(&mut self, index: usize) {
        let slot = &mut self.throttles[index];
        slot.consist.clear();
        slot.facing.clear();
        slot.functions = [false; config::sizes::MAX_FUNCTIONS];
    }

    fn on_address_added(&mut self, throttle: char, addr: LocoAddr) -> bool {
        let Some(idx) = throttle_index(throttle) else {
            return false;
        };
        let slot = &mut self.throttles[idx];
        if slot.consist.push(addr).is_err() {
            warn!("consist full on throttle {}", throttle);
            return false;
        }
        let _ = slot.facing.push(Direction::Forward);
        true
    }

    fn on_address_removed(&mut self, throttle: char, addr: LocoAddr) -> bool {
        let Some(idx) = throttle_index(throttle) else {
            return false;
        };
        if addr.as_str() == "*" {
            self.clear_consist(idx);
            self.throttles[idx].speed = 0;
            return true;
        }
        let slot = &mut self.throttles[idx];
        if let Some(pos) = slot.consist.iter().position(|a| a == &addr) {
            slot.consist.remove(pos);
            if pos < slot.facing.len() {
                slot.facing.remove(pos);
            }
            if slot.consist.is_empty() {
                slot.speed = 0;
            }
            return true;
        }
        false
    }

    fn on_speed_echo(&mut self, throttle: char, speed: u8) -> bool {
        let Some(idx) = throttle_index(throttle) else {
            return false;
        };
        if let Some(at) = self.last_speed_sent_at {
            if idx == self.last_speed_throttle
                && speed == self.last_speed_sent
                && at.elapsed().as_millis() < SPEED_ECHO_DEBOUNCE_MS
            {
                return false;
            }
        }
        self.throttles[idx].speed = speed;
        true
    }

    fn on_direction_lead(&mut self, throttle: char, dir: Direction) -> bool {
        let Some(idx) = throttle_index(throttle) else {
            return false;
        };
        let slot = &mut self.throttles[idx];
        slot.direction = dir;
        if !slot.facing.is_empty() {
            slot.facing[0] = dir;
        }
        true
    }

    fn on_direction_loco(&mut self, throttle: char, addr: LocoAddr, dir: Direction) -> bool {
        let Some(idx) = throttle_index(throttle) else {
            return false;
        };
        let slot = &mut self.throttles[idx];
        if let Some(pos) = slot.consist.iter().position(|a| a == &addr) {
            if pos < slot.facing.len() {
                slot.facing[pos] = dir;
            }
            return true;
        }
        false
    }

    fn on_function_state(&mut self, throttle: char, func: u8, on: bool) -> bool {
        let Some(idx) = throttle_index(throttle) else {
            return false;
        };
        if (func as usize) < config::sizes::MAX_FUNCTIONS {
            self.throttles[idx].functions[func as usize] = on;
            return true;
        }
        false
    }

    fn on_roster_function_labels(
        &mut self,
        throttle: char,
        labels: [longfred_proto::model::ShortText; config::sizes::MAX_FUNCTIONS],
    ) -> bool {
        let Some(idx) = throttle_index(throttle) else {
            return false;
        };
        self.throttles[idx].labels = labels;
        true
    }

    fn on_roster_entry(
        &mut self,
        index: u16,
        name: longfred_proto::model::ShortText,
        address: i32,
        length: char,
    ) -> bool {
        let entry = RosterEntry {
            name,
            address,
            length,
        };
        if (index as usize) < self.roster.len() {
            self.roster[index as usize] = entry;
        } else if self.roster.push(entry).is_err() {
            warn!("roster full");
        }
        true
    }

    fn set_function(
        &mut self,
        func: u8,
        on: bool,
        out: &mut heapless::Vec<ClientCommand, CMD_BUF>,
    ) -> bool {
        if !self.current_slot().has_loco() {
            return false;
        }
        push_cmd(
            out,
            ClientCommand::SetFunction {
                throttle: self.current as u8,
                func,
                on,
                all: true,
            },
        );
        if (func as usize) < config::sizes::MAX_FUNCTIONS {
            self.current_slot_mut().functions[func as usize] = on;
        }
        true
    }

    fn speed_up(&mut self, fast: bool, out: &mut heapless::Vec<ClientCommand, CMD_BUF>) -> bool {
        if !self.current_slot().has_loco() {
            return false;
        }
        let step = self.effective_speed_step(fast);
        let new_speed = self
            .current_slot()
            .speed
            .saturating_add(step)
            .min(MAX_SPEED);
        self.speed_set(new_speed, out)
    }

    fn speed_down(&mut self, fast: bool, out: &mut heapless::Vec<ClientCommand, CMD_BUF>) -> bool {
        if !self.current_slot().has_loco() {
            return false;
        }
        let step = self.effective_speed_step(fast);
        let new_speed = self.current_slot().speed.saturating_sub(step);
        self.speed_set(new_speed, out)
    }

    fn effective_speed_step(&self, fast: bool) -> u8 {
        let base = buttons::SPEED_STEP;
        let mult = if fast {
            buttons::SPEED_STEP_MULTIPLIER
        } else {
            1
        };
        base.saturating_mul(mult)
            .saturating_mul(self.speed_multiplier)
    }

    fn speed_set(&mut self, speed: u8, out: &mut heapless::Vec<ClientCommand, CMD_BUF>) -> bool {
        if !self.current_slot().has_loco() {
            return false;
        }
        let speed = speed.min(MAX_SPEED);
        // Always update local state immediately for responsive UI.
        self.current_slot_mut().speed = speed;

        let now = Instant::now();
        let window = Duration::from_millis(network::SPEED_COALESCE_WINDOW_MS);
        let due = self
            .last_speed_sent_at
            .map_or(true, |at| now.duration_since(at) >= window);

        // Stop is safety-critical: send immediately and drop any pending coalesce.
        if speed == 0 || due {
            let idx = self.current;
            self.emit_speed(idx, speed, out, now);
        } else {
            // Coalesce: overwrite pending (cancels intermediate values).
            self.pending_speed = Some((self.current, speed));
        }
        true
    }

    fn emit_speed(
        &mut self,
        throttle: usize,
        speed: u8,
        out: &mut heapless::Vec<ClientCommand, CMD_BUF>,
        now: Instant,
    ) {
        push_cmd(
            out,
            ClientCommand::SetSpeed {
                throttle: throttle as u8,
                speed,
            },
        );
        self.last_speed_sent = speed;
        self.last_speed_throttle = throttle;
        self.last_speed_sent_at = Some(now);
        self.pending_speed = None;
    }

    /// Trailing flush: send the last coalesced speed after the window elapses.
    pub fn flush_pending_speed(&mut self, out: &mut heapless::Vec<ClientCommand, CMD_BUF>) {
        let Some((idx, speed)) = self.pending_speed else {
            return;
        };
        let now = Instant::now();
        let window = Duration::from_millis(network::SPEED_COALESCE_WINDOW_MS);
        let due = self
            .last_speed_sent_at
            .map_or(true, |at| now.duration_since(at) >= window);
        if due {
            self.emit_speed(idx, speed, out, now);
        }
    }

    fn stop_then_toggle_direction(
        &mut self,
        out: &mut heapless::Vec<ClientCommand, CMD_BUF>,
    ) -> bool {
        if self.current_slot().speed != 0 {
            return self.speed_set(0, out);
        }
        if buttons::TOGGLE_DIRECTION_WHEN_STATIONARY {
            let dir = opposite_slot_direction(self.current_slot().direction);
            return self.change_direction(self.current, dir, out);
        }
        false
    }

    fn change_direction(
        &mut self,
        index: usize,
        dir: Direction,
        out: &mut heapless::Vec<ClientCommand, CMD_BUF>,
    ) -> bool {
        let slot = &self.throttles[index];
        if slot.consist.is_empty() {
            return false;
        }
        if slot.consist.len() == 1 {
            push_cmd(
                out,
                ClientCommand::SetDirection {
                    throttle: index as u8,
                    loco: None,
                    dir,
                },
            );
        } else {
            let lead_facing = slot.facing.first().copied().unwrap_or(slot.direction);
            for i in 1..slot.consist.len() {
                let loco = slot.consist[i].as_str();
                let loco_facing = slot.facing.get(i).copied().unwrap_or(lead_facing);
                let target = if loco_facing == lead_facing {
                    dir
                } else {
                    model::opposite(dir)
                };
                push_cmd(
                    out,
                    ClientCommand::SetDirection {
                        throttle: index as u8,
                        loco: LocoId::parse(loco),
                        dir: target,
                    },
                );
            }
            let lead = slot.consist[0].as_str();
            push_cmd(
                out,
                ClientCommand::SetDirection {
                    throttle: index as u8,
                    loco: LocoId::parse(lead),
                    dir,
                },
            );
        }

        let slot = &mut self.throttles[index];
        slot.direction = dir;
        if !slot.facing.is_empty() {
            slot.facing[0] = dir;
        }
        true
    }

    fn estop_all(&mut self, out: &mut heapless::Vec<ClientCommand, CMD_BUF>) -> bool {
        let mut changed = false;
        self.pending_speed = None;
        for i in 0..self.max_throttles {
            if self.throttles[i].has_loco() {
                push_cmd(out, ClientCommand::EStop { throttle: i as u8 });
                self.throttles[i].speed = 0;
                changed = true;
            }
        }
        changed
    }

    fn estop_current(&mut self, out: &mut heapless::Vec<ClientCommand, CMD_BUF>) -> bool {
        if !self.current_slot().has_loco() {
            return false;
        }
        self.pending_speed = None;
        push_cmd(
            out,
            ClientCommand::EStop {
                throttle: self.current as u8,
            },
        );
        self.current_slot_mut().speed = 0;
        true
    }

    fn set_track_power(
        &mut self,
        on: bool,
        out: &mut heapless::Vec<ClientCommand, CMD_BUF>,
    ) -> bool {
        push_cmd(out, ClientCommand::TrackPower(on));
        self.track_power = if on { TrackPower::On } else { TrackPower::Off };
        true
    }

    fn cycle_speed_multiplier(&mut self) {
        let add = buttons::SPEED_STEP_ADDITIONAL_MULTIPLIER;
        self.speed_multiplier = match self.speed_multiplier {
            1 => add,
            m if m == add => add.saturating_mul(2),
            _ => 1,
        };
        let step = buttons::SPEED_STEP.saturating_mul(self.speed_multiplier);
        for i in 0..config::sizes::MAX_THROTTLES {
            self.throttles[i].speed_step = step;
        }
    }
}

fn opposite_slot_direction(dir: Direction) -> Direction {
    model::opposite(dir)
}

fn parse_acquire_addr(digits: &str) -> Option<LocoId> {
    match digits.as_bytes().first() {
        Some(b'S' | b's' | b'L' | b'l') => LocoId::parse(digits),
        _ => LocoId::parse(build_loco_addr(digits)?.as_str()),
    }
}

fn build_loco_addr(digits: &str) -> Option<LocoAddr> {
    let addr: u32 = digits.parse().ok()?;
    let mut s = LocoAddr::new();
    if addr > SHORT_DCC_ADDRESS_LIMIT || (digits.starts_with('0') && digits.len() > 1) {
        let _ = s.push('L');
    } else {
        let _ = s.push('S');
    }
    let _ = s.push_str(digits);
    Some(s)
}

fn write_roster_addr(address: i32, length: char, out: &mut heapless::String<8>) -> Result<(), ()> {
    let mut buf = heapless::String::<8>::new();
    let abs = address.unsigned_abs();
    if abs >= 10000 {
        return Err(());
    }
    if abs >= 1000 {
        let _ = buf.push((b'0' + (abs / 1000) as u8) as char);
    }
    if abs >= 100 {
        let _ = buf.push((b'0' + ((abs / 100) % 10) as u8) as char);
    }
    if abs >= 10 {
        let _ = buf.push((b'0' + ((abs / 10) % 10) as u8) as char);
    }
    let _ = buf.push((b'0' + (abs % 10) as u8) as char);
    let _ = out.push_str(buf.as_str());
    if length != 'S' && length != 's' {
        let _ = out.push(length);
    }
    Ok(())
}

fn push_cmd(out: &mut heapless::Vec<ClientCommand, CMD_BUF>, cmd: ClientCommand) {
    if out.push(cmd).is_err() {
        warn!("command buffer full");
    }
}
