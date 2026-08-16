//! Headless shell for heiko-wifred (no menu / permanent drive mode).

use crate::domain::actions::Action;
use crate::domain::state::DomainState;
use crate::input::InputEvent;
use crate::ui::menu::Intent;
use crate::ui::view::{Line, ThrottleView, UiView};

/// Passes only drive / programming events; ignores Nav, Menu, Digit, etc.
pub struct HeadlessShell;

impl HeadlessShell {
    pub const fn new() -> Self {
        Self
    }

    /// Whether this event is relevant in headless drive mode.
    pub fn should_pass(ev: &InputEvent) -> bool {
        matches!(
            ev,
            InputEvent::SpeedAbsolute(_)
                | InputEvent::DirectionSet(_)
                | InputEvent::DirectionToggle
                | InputEvent::FnPress(_)
                | InputEvent::FnRelease(_)
                | InputEvent::EStop
                | InputEvent::Stop
                | InputEvent::LocoSlot(_, _)
                | InputEvent::EnterProgrammingMode
        )
    }

    /// Map a drive event to a domain intent.
    ///
    /// `SpeedAbsolute` and `EnterProgrammingMode` return [`Intent::None`] —
    /// the domain task applies them from the raw [`InputEvent`] directly.
    pub fn handle(&mut self, ev: InputEvent, _domain: &DomainState) -> Intent {
        if !Self::should_pass(&ev) {
            return Intent::None;
        }
        match ev {
            InputEvent::DirectionSet(dir) => {
                if dir == longfred_proto::model::Direction::Forward {
                    Intent::Action(Action::DirectionForward)
                } else {
                    Intent::Action(Action::DirectionReverse)
                }
            }
            InputEvent::DirectionToggle => Intent::Action(Action::DirectionToggle),
            InputEvent::FnPress(f) => Intent::Function(f),
            InputEvent::FnRelease(_) => Intent::None,
            InputEvent::EStop | InputEvent::Stop => Intent::Action(Action::EStop),
            InputEvent::LocoSlot(slot, on) if on => Intent::Action(Action::Throttle(slot)),
            InputEvent::SpeedAbsolute(_)
            | InputEvent::EnterProgrammingMode
            | InputEvent::LocoSlot(_, _) => Intent::None,
            _ => Intent::None,
        }
    }

    /// Minimal throttle view from domain state (no grid / menu screens).
    pub fn view(&self, domain: &DomainState) -> UiView {
        let slot = domain.current_slot();
        let mut functions: u32 = 0;
        for (i, on) in slot.functions.iter().enumerate().take(32) {
            if *on {
                functions |= 1u32 << i;
            }
        }
        let mut loco = Line::new();
        if let Some(addr) = slot.consist.first() {
            let _ = loco.push_str(addr.as_str());
        }
        UiView::Throttle(ThrottleView {
            current: domain.current as u8,
            speed: slot.speed,
            forward: domain.current_forward(),
            consist_len: slot.consist.len() as u8,
            power_on: domain.track_power_on(),
            heartbeat_on: domain.heartbeat_enabled(),
            functions,
            loco,
            footer: Line::new(),
            next_hint: Line::new(),
            battery: None,
            battery_show_percent: false,
        })
    }
}

impl Default for HeadlessShell {
    fn default() -> Self {
        Self::new()
    }
}
