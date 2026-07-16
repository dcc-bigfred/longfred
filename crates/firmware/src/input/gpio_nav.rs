//! Direct GPIO nav cluster: Up/Down/Left/Right/Ok/Back/Menu (active-low).

use embassy_time::{Duration, Timer};
use esp_hal::gpio::{Input, InputConfig, InputPin, Pull};

use super::{InputEvent, InputSender, NavDir};

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

    /// Returns true on a stable high→low edge (press with pull-up).
    fn update(&mut self, raw_high: bool) -> bool {
        if raw_high == self.stable_high {
            self.debounce = 0;
            return false;
        }
        self.debounce = self.debounce.saturating_add(1);
        if self.debounce < DEBOUNCE_TICKS {
            return false;
        }
        let was_high = self.stable_high;
        self.stable_high = raw_high;
        self.debounce = 0;
        was_high && !raw_high
    }
}

#[embassy_executor::task]
pub async fn task(pins: Pins, sender: InputSender) {
    let mut up = Btn::new(pins.up.is_high());
    let mut down = Btn::new(pins.down.is_high());
    let mut left = Btn::new(pins.left.is_high());
    let mut right = Btn::new(pins.right.is_high());
    let mut ok = Btn::new(pins.ok.is_high());
    let mut back = Btn::new(pins.back.is_high());
    let mut menu = Btn::new(pins.menu.is_high());

    loop {
        if up.update(pins.up.is_high()) {
            let _ = sender.try_send(InputEvent::Nav(NavDir::Up));
        }
        if down.update(pins.down.is_high()) {
            let _ = sender.try_send(InputEvent::Nav(NavDir::Down));
        }
        if left.update(pins.left.is_high()) {
            let _ = sender.try_send(InputEvent::Nav(NavDir::Left));
        }
        if right.update(pins.right.is_high()) {
            let _ = sender.try_send(InputEvent::Nav(NavDir::Right));
        }
        if ok.update(pins.ok.is_high()) {
            let _ = sender.try_send(InputEvent::Ok);
        }
        if back.update(pins.back.is_high()) {
            let _ = sender.try_send(InputEvent::Back);
        }
        if menu.update(pins.menu.is_high()) {
            let _ = sender.try_send(InputEvent::Menu);
        }

        Timer::after(Duration::from_millis(POLL_MS)).await;
    }
}
