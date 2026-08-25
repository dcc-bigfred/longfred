//! Drive HUD (loco acquired) and address-entry mode (no loco).

use core::fmt::Write as _;

use longfred_proto::catalog::{Catalog, LocoCatalog};
use longfred_proto::network::ConnState;
use longfred_proto::{LocoId, LocoSource};

use super::helpers::has_loco;
use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, PageDir, ScreenId};
use crate::screen::{KeyBindings, Screen};
use crate::view::{Line, ThrottleView, UiView, push_oled};
use crate::widgets::{KeyboardMode, TextKeyboard};

/// Drive HUD (loco acquired) and address-entry mode (no loco).
pub struct ThrottleScreen {
    addr_kbd: TextKeyboard<5>,
}

impl ThrottleScreen {
    /// Digit keyboard for DCC address when no loco is acquired.
    #[must_use]
    pub fn new() -> Self {
        Self {
            addr_kbd: TextKeyboard::new(KeyboardMode::Digits),
        }
    }
}

impl Default for ThrottleScreen {
    fn default() -> Self {
        Self::new()
    }
}

/// 1-based catalogue position and length for the HUD corner (`1/3`).
fn list_index(cx: &ScreenCtx<'_>) -> Option<(u8, u8)> {
    if cx.drive.effective_loco_source == LocoSource::AddressOnly {
        return None;
    }
    let cat = Catalog::for_source(
        cx.drive.effective_loco_source,
        cx.drive.roster,
        &cx.drive.persist.static_roster,
    );
    let n = u8::try_from(cat.len()).ok().filter(|&n| n > 0)?;
    let slot = cx.drive.slots.get(cx.drive.current)?;
    let idx = slot.list_idx.or_else(|| {
        let have = slot.consist.first()?.as_str();
        (0..usize::from(n)).find(|&i| cat.entry(i).is_some_and(|e| e.addr.as_str() == have))
    })?;
    let pos = u8::try_from(idx.saturating_add(1)).ok()?;
    (pos >= 1 && pos <= n).then_some((pos, n))
}

impl Screen for ThrottleScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Throttle
    }

    /// Always THROTTLE so Menu stays `MenuEnter`; address entry still uses Digit.
    fn key_bindings(&self, _cx: &ScreenCtx<'_>) -> KeyBindings {
        KeyBindings::THROTTLE
    }

    /// Restore a typed DCC address from the session (screen objects are not reused).
    fn on_enter(&mut self, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        if !cx.session.addr.is_empty() {
            self.addr_kbd.load(cx.session.addr.as_str());
        }
    }

    /// Drive HUD when a loco is acquired; otherwise address preview or "no loco".
    /// Packs speed, direction, consist length, function bitmask, and battery.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let slot = cx.drive.slots.get(cx.drive.current);
        let mut loco = Line::new();
        if let Some(s) = slot {
            if s.has_loco() {
                if let Some(addr) = s.consist.first() {
                    if let Some(id) = LocoId::parse(addr.as_str()) {
                        let _ = write!(loco, "{}", id.addr);
                    } else {
                        push_oled(&mut loco, addr.as_str());
                    }
                    if !s.name.is_empty() {
                        push_oled(&mut loco, ": ");
                        push_oled(&mut loco, s.name.as_str());
                    }
                }
            } else if !self.addr_kbd.buffer.is_empty() {
                push_oled(&mut loco, self.addr_kbd.value_preview().as_str());
            } else {
                push_oled(&mut loco, cx.s.msg_no_loco);
            }
        }
        let mut footer = Line::new();
        push_oled(&mut footer, cx.s.hint_throttle);
        let mut functions = 0u32;
        let (speed, forward, consist_len) = match slot {
            Some(s) => {
                for (i, on) in s.functions.iter().enumerate().take(32) {
                    if *on {
                        functions |= 1 << i;
                    }
                }
                (
                    s.speed,
                    s.direction == longfred_proto::model::Direction::Forward,
                    s.consist.len().try_into().unwrap_or(u8::MAX),
                )
            }
            None => (0, true, 0),
        };
        UiView::Throttle(ThrottleView {
            list_index: list_index(cx),
            speed,
            forward,
            consist_len,
            server_connected: matches!(cx.net.conn, ConnState::Connected),
            functions,
            loco,
            footer,
            next_hint: Line::new(),
            battery: cx.battery.map(|b| b.percent),
            battery_charging: cx.battery.is_some_and(|b| b.charging),
        })
    }

    /// Open the main menu (keep a typed address in the session).
    fn on_menu_key(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let _ = self.addr_kbd.ok();
        cx.session.addr.clear();
        let _ = cx.session.addr.push_str(self.addr_kbd.buffer.as_str());
        nav.go(ScreenId::Menu);
    }

    /// Back is unused on the drive HUD.
    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {}

    /// With loco: function list or direct commands. Without: acquire typed address.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if has_loco(cx) {
            if cx.session.hash_functions {
                nav.go(ScreenId::FunctionList);
            } else {
                nav.go(ScreenId::DirectCommands);
            }
            return;
        }
        let _ = self.addr_kbd.ok();
        cx.session.addr.clear();
        let _ = cx.session.addr.push_str(self.addr_kbd.buffer.as_str());
        self.addr_kbd.clear();
        if !cx.session.addr.is_empty() {
            nav.emit(Intent::AcquireAddr);
        }
    }

    /// With loco: toggle function 0–9. Without: type a DCC address digit.
    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if !c.is_ascii_digit() {
            return;
        }
        if has_loco(cx) {
            nav.emit(Intent::Function(c as u8 - b'0'));
            return;
        }
        let _ = self.addr_kbd.key_press(c as u8 - b'0', cx.now_ms);
    }

    /// Encoder steps the pending address digit when no loco is acquired.
    fn on_char_cycle(&mut self, d: i8, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        if !has_loco(cx) {
            let _ = self.addr_kbd.char_cycle(d, cx.now_ms);
        }
    }

    /// Move the address cursor when no loco is acquired.
    fn on_cursor_move(&mut self, d: i8, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        if has_loco(cx) {
            return;
        }
        if d < 0 {
            let _ = self.addr_kbd.nav_left();
        } else {
            let _ = self.addr_kbd.nav_right();
        }
    }

    /// With loco: map hardware Fn to DCC. Without: type into the address keyboard.
    fn on_fn_key(&mut self, k: u8, down: bool, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if !down {
            return;
        }
        if has_loco(cx) {
            if let Some(dcc) = cx.env.fn_to_dcc.get(usize::from(k)) {
                nav.emit(Intent::Function(*dcc));
            }
            return;
        }
        let _ = self.addr_kbd.fn_press(k, cx.now_ms);
    }

    /// Commit pending multitap on the address keyboard.
    fn on_tick(&mut self, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        if !has_loco(cx) {
            self.addr_kbd.tick(cx.now_ms);
        }
    }

    /// Menu Left / Right walk the effective catalogue in this slot.
    fn on_page(&mut self, d: PageDir, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        if cx.drive.effective_loco_source == LocoSource::AddressOnly {
            return;
        }
        nav.emit(Intent::SelectLoco(d));
    }
}
