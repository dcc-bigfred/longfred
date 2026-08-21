//! Field-by-field IPv4 editor (DHCP / IP / mask / GW / DNS).

use super::helpers::{commit_net_field, load_net_field_digits};
use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, ScreenId};
use crate::screen::{KeyBindings, Screen};
use crate::session::NetField;
use crate::view::{Line, UiView, push_oled};
use crate::widgets::{KeyboardMode, TextKeyboard, format_grouped_ip};

/// Field-by-field IPv4 editor (DHCP / IP / mask / GW / DNS).
pub struct IpEditScreen {
    kbd: TextKeyboard<12>,
}

impl IpEditScreen {
    /// Digit keyboard reused for DHCP/IP/mask/GW/DNS fields.
    #[must_use]
    pub fn new() -> Self {
        Self {
            kbd: TextKeyboard::new(KeyboardMode::Digits),
        }
    }

    /// Load the current field's digits and max length.
    fn reload(&mut self, cx: &mut ScreenCtx<'_>) {
        let field = cx.session.ip_field;
        self.kbd.set_max_len(field.max_digits());
        self.kbd
            .load(load_net_field_digits(&cx.session.net_cfg, field).as_str());
    }

    /// `"Mode 0 DHCP"` / `"IP aaa.bbb.ccc.ddd"` / mask / GW / DNS line for the current field.
    fn format_line(&self, cx: &ScreenCtx<'_>) -> Line {
        let mut s = Line::new();
        let _ = s.push_str(cx.session.ip_field.label());
        let _ = s.push(' ');
        match cx.session.ip_field {
            NetField::Dhcp => {
                push_oled(&mut s, self.kbd.preview().as_str());
                let d = self
                    .kbd
                    .pending()
                    .or_else(|| self.kbd.buffer.chars().next())
                    .unwrap_or(if cx.session.net_cfg.dhcp { '0' } else { '1' });
                let _ = s.push_str(if d == '0' { " DHCP" } else { " Static" });
                return s;
            }
            NetField::Prefix => {
                push_oled(&mut s, self.kbd.preview().as_str());
                return s;
            }
            NetField::Ip | NetField::Gateway | NetField::Dns => {}
        }
        let ip = format_grouped_ip(
            self.kbd.buffer.as_str(),
            self.kbd.cursor(),
            self.kbd.slot_char(),
            false,
        );
        push_oled(&mut s, ip.as_str());
        s
    }
}

impl Default for IpEditScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for IpEditScreen {
    fn id(&self) -> ScreenId {
        ScreenId::IpEdit
    }

    fn key_bindings(&self, _cx: &ScreenCtx<'_>) -> KeyBindings {
        KeyBindings::TEXT
    }

    /// Load digits for `session.ip_field`.
    fn on_enter(&mut self, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        self.reload(cx);
    }

    /// Current field label + grouped digits and a footer hint.
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        g.set(0, cx.s.msg_net_config, false);
        g.set(2, self.format_line(cx).as_str(), false);
        g.set(5, cx.s.hint_net_edit, false);
        UiView::Grid(g)
    }

    /// Encoder cycles the digit under the cursor.
    fn on_char_cycle(&mut self, d: i8, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        let _ = self.kbd.char_cycle(d, cx.now_ms);
    }

    /// Move the digit cursor.
    fn on_cursor_move(&mut self, d: i8, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        if d < 0 {
            let _ = self.kbd.nav_left();
        } else {
            let _ = self.kbd.nav_right();
        }
    }

    /// Type a digit into the current field.
    fn on_digit(&mut self, c: char, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        if c.is_ascii_digit() {
            let _ = self.kbd.key_press(c as u8 - b'0', cx.now_ms);
        }
    }

    /// Hardware Fn keys type digits.
    fn on_fn_key(&mut self, k: u8, down: bool, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        if down {
            let _ = self.kbd.fn_press(k, cx.now_ms);
        }
    }

    /// Commit pending multitap.
    fn on_tick(&mut self, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        self.kbd.tick(cx.now_ms);
    }

    /// Commit this field. DHCP or last field saves and returns to throttle; else advance.
    fn on_select(&mut self, cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        let _ = self.kbd.ok();
        let field = cx.session.ip_field;
        commit_net_field(
            &mut cx.session.net_cfg,
            field,
            self.kbd.buffer.as_str(),
            cx.env.default_prefix_len,
        );
        if field == NetField::Dhcp && cx.session.net_cfg.dhcp {
            nav.emit(Intent::SaveNetwork(cx.session.net_cfg));
            nav.root(ScreenId::Throttle);
            return;
        }
        if let Some(next) = field.next() {
            cx.session.ip_field = next;
            self.reload(cx);
        } else {
            nav.emit(Intent::SaveNetwork(cx.session.net_cfg));
            nav.root(ScreenId::Throttle);
        }
    }

    /// Discard remaining fields (draft is not saved) and pop to Extras.
    fn on_cancel(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        nav.back();
    }
}
