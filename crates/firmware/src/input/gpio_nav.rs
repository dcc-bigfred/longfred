//! Direct GPIO nav cluster: Up/Down/Left/Right/Ok/Back/Menu (active-low).
//! Emits [`RawEvent`] for the board ControlSurface bridge.

use embassy_time::{Duration, Timer};
use esp_hal::gpio::{Input, InputConfig, InputPin, Pull};

use crate::board::raw::{ButtonId, RawEvent, RawSender};

const POLL_MS: u64 = 20;
const DEBOUNCE_TICKS: u8 = 2;

pub struct Pins {
    pub up: Input<'static>,
    pub down: Input<'static>,
    pub left: Input<'static>,
    pub right: Input<'static>,
    pub ok: Input<'static>,
    pub back: Input<'static>,
    pub menu: Input<'static>,
}

/// Call once from `main` with the real GPIO peripherals (not `AnyPin::steal`).
pub fn build(
    up: impl InputPin + 'static,
    down: impl InputPin + 'static,
    left: impl InputPin + 'static,
    right: impl InputPin + 'static,
    ok: impl InputPin + 'static,
    back: impl InputPin + 'static,
    menu: impl InputPin + 'static,
) -> Pins {
    let cfg = InputConfig::default().with_pull(Pull::Up);
    Pins {
        up: Input::new(up, cfg),
        down: Input::new(down, cfg),
        left: Input::new(left, cfg),
        right: Input::new(right, cfg),
        ok: Input::new(ok, cfg),
        back: Input::new(back, cfg),
        menu: Input::new(menu, cfg),
    }
}

struct Btn {
    stable_high: bool,
    debounce: u8,
}

impl Btn {
    fn new(initial_high: bool) -> Self {
        Self {
            stable_high: initial_high,
            debounce: 0,
        }
    }

    /// Returns `Some(true)` on press (high→low), `Some(false)` on release.
    fn update(&mut self, raw_high: bool) -> Option<bool> {
        if raw_high == self.stable_high {
            self.debounce = 0;
            return None;
        }
        self.debounce = self.debounce.saturating_add(1);
        if self.debounce < DEBOUNCE_TICKS {
            return None;
        }
        let was_high = self.stable_high;
        self.stable_high = raw_high;
        self.debounce = 0;
        if was_high && !raw_high {
            Some(true)
        } else if !was_high && raw_high {
            Some(false)
        } else {
            None
        }
    }
}

fn emit(sender: &RawSender, id: ButtonId, pressed: bool) {
    let _ = sender.try_send(RawEvent::Button(id, pressed));
}

#[embassy_executor::task]
pub async fn task(pins: Pins, sender: RawSender) {
    let mut up = Btn::new(pins.up.is_high());
    let mut down = Btn::new(pins.down.is_high());
    let mut left = Btn::new(pins.left.is_high());
    let mut right = Btn::new(pins.right.is_high());
    let mut ok = Btn::new(pins.ok.is_high());
    let mut back = Btn::new(pins.back.is_high());
    let mut menu = Btn::new(pins.menu.is_high());

    loop {
        // Transitional GPIO map → ButtonId (ControlSurface → InputEvent).
        if let Some(p) = up.update(pins.up.is_high()) {
            emit(&sender, ButtonId::JoyUp, p);
        }
        if let Some(p) = down.update(pins.down.is_high()) {
            emit(&sender, ButtonId::JoyDown, p);
        }
        if let Some(p) = left.update(pins.left.is_high()) {
            emit(&sender, ButtonId::JoyLeft, p);
        }
        if let Some(p) = right.update(pins.right.is_high()) {
            emit(&sender, ButtonId::JoyRight, p);
        }
        // Old Ok → JoyMenu (surface emits Ok/select).
        if let Some(p) = ok.update(pins.ok.is_high()) {
            emit(&sender, ButtonId::JoyMenu, p);
        }
        // Old Back → Stop (shell maps Stop → EStop/Cancel).
        if let Some(p) = back.update(pins.back.is_high()) {
            emit(&sender, ButtonId::Stop, p);
        }
        if let Some(p) = menu.update(pins.menu.is_high()) {
            emit(&sender, ButtonId::Menu, p);
        }

        Timer::after(Duration::from_millis(POLL_MS)).await;
    }
}
