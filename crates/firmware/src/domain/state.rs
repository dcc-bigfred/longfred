//! Stan domeny + redukcja zdarzeń wejścia i serwera.

use embassy_time::Instant;
use log::{info, warn};
use longfred_proto::events::ServerEvent;
use longfred_proto::model::{Direction, LocoAddr, TrackPower};
use longfred_proto::protocol::{self, Cmd};

use crate::config::{self, buttons};
use crate::domain::actions::Action;
use crate::domain::model::{
    self, throttle_char, throttle_index, FunctionFollow, RosterEntry, ThrottleSlot, DomainSnapshot,
    MAX_SPEED, SHORT_DCC_ADDRESS_LIMIT,
};
use crate::input::InputEvent;

pub const CMD_BUF: usize = 12;
const SPEED_ECHO_DEBOUNCE_MS: u64 = 500;

pub struct DomainState {
    pub throttles: [ThrottleSlot; config::sizes::MAX_THROTTLES],
    pub current: usize,
    pub max_throttles: usize,
    pub track_power: TrackPower,
    pub speed_multiplier: u8,
    pub roster: heapless::Vec<RosterEntry, { config::sizes::MAX_ROSTER }>,
    pub roster_count: u16,
    pub addr: heapless::String<4>,
    last_speed_sent: u8,
    last_speed_throttle: usize,
    last_speed_sent_at: Option<Instant>,
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
            addr: heapless::String::new(),
            last_speed_sent: 0,
            last_speed_throttle: 0,
            last_speed_sent_at: None,
        }
    }

    pub fn snapshot(&self) -> DomainSnapshot {
        let slot = &self.throttles[self.current];
        let mut snap_addr = heapless::String::<5>::new();
        let _ = snap_addr.push_str(self.addr.as_str());
        DomainSnapshot {
            current: self.current as u8,
            speed: slot.speed,
            forward: slot.direction == Direction::Forward,
            consist_len: slot.consist.len() as u8,
            power_on: model::track_power_on(self.track_power),
            has_loco: slot.has_loco(),
            acquiring: !slot.has_loco(),
            addr: snap_addr,
        }
    }

    fn current_slot(&self) -> &ThrottleSlot {
        &self.throttles[self.current]
    }

    fn current_slot_mut(&mut self) -> &mut ThrottleSlot {
        &mut self.throttles[self.current]
    }

    fn acquire_mode(&self) -> bool {
        !self.current_slot().has_loco()
    }

    pub fn apply_input(&mut self, ev: InputEvent, out: &mut heapless::Vec<Cmd, CMD_BUF>) -> bool {
        info!("domain input: {:?}", ev);
        let changed = match ev {
            InputEvent::KeyPress(c) => {
                if self.acquire_mode() {
                    self.handle_acquire_key_press(c, out)
                } else {
                    self.handle_operate_key_press(c, out)
                }
            }
            InputEvent::KeyRelease(c) => {
                if self.acquire_mode() {
                    false
                } else {
                    self.handle_operate_key_release(c, out)
                }
            }
            InputEvent::EncoderClockwise | InputEvent::EncoderCounterClockwise => {
                if self.acquire_mode() || !self.current_slot().has_loco() {
                    false
                } else {
                    let cw = matches!(ev, InputEvent::EncoderClockwise);
                    self.apply_encoder(cw, out)
                }
            }
            InputEvent::EncoderButton => {
                if self.acquire_mode() {
                    false
                } else {
                    self.apply_action(buttons::ENCODER_BUTTON_ACTION, true, out)
                }
            }
        };
        changed || !out.is_empty()
    }

    fn handle_acquire_key_press(&mut self, c: char, out: &mut heapless::Vec<Cmd, CMD_BUF>) -> bool {
        match c {
            '0'..='9' if self.addr.len() < 4 => {
                let _ = self.addr.push(c);
                true
            }
            '#' => self.acquire(out),
            '*' => {
                self.addr.clear();
                true
            }
            _ => false,
        }
    }

    fn handle_operate_key_press(&mut self, c: char, out: &mut heapless::Vec<Cmd, CMD_BUF>) -> bool {
        match c {
            '#' => self.release_all(out),
            '*' => {
                self.addr.clear();
                true
            }
            _ => {
                let action = buttons::default_action(c);
                self.apply_action(action, true, out)
            }
        }
    }

    fn handle_operate_key_release(&mut self, c: char, out: &mut heapless::Vec<Cmd, CMD_BUF>) -> bool {
        let action = buttons::default_action(c);
        if let Action::Function(f) = action {
            self.apply_function(f, false, false, out)
        } else {
            false
        }
    }

    fn apply_encoder(&mut self, clockwise: bool, out: &mut heapless::Vec<Cmd, CMD_BUF>) -> bool {
        let mut increase = clockwise == buttons::ENCODER_CLOCKWISE_INCREASES_SPEED;
        if buttons::ENCODER_INVERT_WHEN_REVERSED
            && self.current_slot().direction == Direction::Reverse
        {
            increase = !increase;
        }
        if increase {
            self.speed_up(false, out)
        } else {
            self.speed_down(false, out)
        }
    }

    pub fn apply_action(
        &mut self,
        action: Action,
        pressed: bool,
        out: &mut heapless::Vec<Cmd, CMD_BUF>,
    ) -> bool {
        match action {
            Action::None => false,
            Action::Function(f) => self.apply_function(f, pressed, false, out),
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
            Action::DirectionForward => self.change_direction(self.current, Direction::Forward, out),
            Action::DirectionReverse => self.change_direction(self.current, Direction::Reverse, out),
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
                warn!("custom command {} not configured (Etap 9)", n);
                false
            }
            _ => false,
        }
    }

    pub fn apply_event(
        &mut self,
        ev: ServerEvent,
        out: &mut heapless::Vec<Cmd, CMD_BUF>,
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
            ServerEvent::DirectionLoco { throttle, addr, dir } => {
                self.on_direction_loco(throttle, addr, dir)
            }
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
            ServerEvent::StealNeeded { throttle, addr, .. } => {
                let idx = throttle_index(throttle).unwrap_or(self.current);
                push_cmd(out, protocol::steal_loco(throttle_char(idx), addr.as_str()));
                false
            }
            ServerEvent::HeartbeatConfig { .. }
            | ServerEvent::Version(_)
            | ServerEvent::ServerType(_)
            | ServerEvent::ServerDescription(_)
            | ServerEvent::Message(_)
            | ServerEvent::Alert(_)
            | ServerEvent::WebPort(_)
            | ServerEvent::TurnoutEntriesCount(_)
            | ServerEvent::TurnoutEntry { .. }
            | ServerEvent::RouteEntriesCount(_)
            | ServerEvent::RouteEntry { .. }
            | ServerEvent::TurnoutAction { .. }
            | ServerEvent::RouteAction { .. }
            | ServerEvent::Unknown(_) => false,
        }
    }

    fn acquire(&mut self, out: &mut heapless::Vec<Cmd, CMD_BUF>) -> bool {
        if self.addr.is_empty() {
            return false;
        }
        let Some(loco) = build_loco_addr(self.addr.as_str()) else {
            return false;
        };
        let t = throttle_char(self.current);
        if buttons::DROP_BEFORE_ACQUIRE {
            push_cmd(out, protocol::release_loco(t, "*"));
            self.clear_consist(self.current);
        }
        let loco_str = loco.as_str();
        push_cmd(out, protocol::add_loco(t, loco_str, loco_str));
        self.addr.clear();
        true
    }

    fn release_all(&mut self, out: &mut heapless::Vec<Cmd, CMD_BUF>) -> bool {
        if !self.current_slot().has_loco() {
            return false;
        }
        let t = throttle_char(self.current);
        push_cmd(out, protocol::release_loco(t, "*"));
        self.clear_consist(self.current);
        self.current_slot_mut().speed = 0;
        true
    }

    fn release_throttle(&mut self, index: usize, out: &mut heapless::Vec<Cmd, CMD_BUF>) {
        if self.throttles[index].has_loco() {
            let t = throttle_char(index);
            push_cmd(out, protocol::release_loco(t, "*"));
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

    fn apply_function(
        &mut self,
        func: u8,
        pressed: bool,
        force: bool,
        out: &mut heapless::Vec<Cmd, CMD_BUF>,
    ) -> bool {
        if !self.current_slot().has_loco() {
            return false;
        }
        let t = throttle_char(self.current);
        let selector = function_loco_selector(self.current_slot(), func);
        push_cmd(
            out,
            protocol::set_function(t, selector, func, pressed, force),
        );
        if (func as usize) < config::sizes::MAX_FUNCTIONS {
            self.current_slot_mut().functions[func as usize] = pressed;
        }
        true
    }

    fn speed_up(&mut self, fast: bool, out: &mut heapless::Vec<Cmd, CMD_BUF>) -> bool {
        if !self.current_slot().has_loco() {
            return false;
        }
        let step = self.effective_speed_step(fast);
        let new_speed = self.current_slot().speed.saturating_add(step).min(MAX_SPEED);
        self.speed_set(new_speed, out)
    }

    fn speed_down(&mut self, fast: bool, out: &mut heapless::Vec<Cmd, CMD_BUF>) -> bool {
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
        base.saturating_mul(mult).saturating_mul(self.speed_multiplier)
    }

    fn speed_set(&mut self, speed: u8, out: &mut heapless::Vec<Cmd, CMD_BUF>) -> bool {
        if !self.current_slot().has_loco() {
            return false;
        }
        let speed = speed.min(MAX_SPEED);
        let t = throttle_char(self.current);
        push_cmd(out, protocol::set_speed(t, speed));
        self.current_slot_mut().speed = speed;
        self.last_speed_sent = speed;
        self.last_speed_throttle = self.current;
        self.last_speed_sent_at = Some(Instant::now());
        true
    }

    fn stop_then_toggle_direction(&mut self, out: &mut heapless::Vec<Cmd, CMD_BUF>) -> bool {
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
        out: &mut heapless::Vec<Cmd, CMD_BUF>,
    ) -> bool {
        let slot = &self.throttles[index];
        if slot.consist.is_empty() {
            return false;
        }
        let t = throttle_char(index);

        if slot.consist.len() == 1 {
            push_cmd(out, protocol::set_direction(t, "*", dir));
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
                push_cmd(out, protocol::set_direction(t, loco, target));
            }
            let lead = slot.consist[0].as_str();
            push_cmd(out, protocol::set_direction(t, lead, dir));
        }

        let slot = &mut self.throttles[index];
        slot.direction = dir;
        if !slot.facing.is_empty() {
            slot.facing[0] = dir;
        }
        true
    }

    fn estop_all(&mut self, out: &mut heapless::Vec<Cmd, CMD_BUF>) -> bool {
        let mut changed = false;
        for i in 0..self.max_throttles {
            if self.throttles[i].has_loco() {
                let t = throttle_char(i);
                push_cmd(out, protocol::estop(t, "*"));
                self.throttles[i].speed = 0;
                changed = true;
            }
        }
        changed
    }

    fn estop_current(&mut self, out: &mut heapless::Vec<Cmd, CMD_BUF>) -> bool {
        if !self.current_slot().has_loco() {
            return false;
        }
        let t = throttle_char(self.current);
        push_cmd(out, protocol::estop(t, "*"));
        self.current_slot_mut().speed = 0;
        true
    }

    fn set_track_power(&mut self, on: bool, out: &mut heapless::Vec<Cmd, CMD_BUF>) -> bool {
        push_cmd(out, protocol::track_power(on));
        self.track_power = if on {
            TrackPower::On
        } else {
            TrackPower::Off
        };
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

fn function_loco_selector(slot: &ThrottleSlot, func: u8) -> &'static str {
    match slot.follow.get(func as usize).copied().unwrap_or(FunctionFollow::Lead) {
        FunctionFollow::All => "*",
        FunctionFollow::Lead => "",
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

fn push_cmd(out: &mut heapless::Vec<Cmd, CMD_BUF>, cmd: Cmd) {
    if out.push(cmd).is_err() {
        warn!("command buffer full");
    }
}
