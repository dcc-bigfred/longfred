//! Device name / numeric id summary.

use longfred_proto::persist::DEVICE_ID_MIN;

use super::helpers::write_u16_padded;
use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, ScreenId, Step};
use crate::screen::Screen;
use crate::view::{Line, UiView};

pub struct DeviceScreen {
    cursor: u8,
}

impl DeviceScreen {
    /// Three-row picker: name, id, regenerate.
    pub fn new() -> Self {
        Self { cursor: 0 }
    }
}

impl Default for DeviceScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for DeviceScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Device
    }

    /// Copy persist into the session draft and reset the cursor.
    fn on_enter(&mut self, cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        cx.session.device = cx.drive.persist.device.clone();
        self.cursor = 0;
    }

    /// Current name/id plus two action rows (edit vs new id).
    fn view(&self, cx: &ScreenCtx<'_>) -> UiView {
        let mut g = crate::view::GridView::new();
        g.set(0, cx.s.msg_device, false);
        let mut name_line = Line::new();
        let _ = name_line.push_str(cx.s.msg_device_name);
        let _ = name_line.push(' ');
        let _ = name_line.push_str(cx.session.device.name.as_str());
        g.set(1, name_line.as_str(), false);
        let mut id_line = Line::new();
        let _ = id_line.push_str(cx.s.msg_device_id);
        let _ = id_line.push(' ');
        let id = cx.session.device.id;
        if id >= DEVICE_ID_MIN {
            write_u16_padded(&mut id_line, id);
        } else {
            let _ = id_line.push_str("----");
        }
        g.set(2, id_line.as_str(), false);
        g.set(3, cx.s.device_name_id, self.cursor <= 1);
        g.set(4, cx.s.device_new_id, self.cursor == 2);
        g.set(5, cx.s.hint_device, false);
        UiView::Grid(g)
    }

    /// Cycle cursor 0=name, 1=id, 2=regenerate.
    fn on_list_step(&mut self, d: Step, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        self.cursor = match d {
            Step::Prev => {
                if self.cursor == 0 {
                    2
                } else {
                    self.cursor - 1
                }
            }
            Step::Next => {
                if self.cursor >= 2 {
                    0
                } else {
                    self.cursor + 1
                }
            }
        };
    }

    /// Replace with a name/id editor (Back → Extras) or emit a new random id.
    fn on_select(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        match self.cursor {
            0 => nav.replace(ScreenId::DeviceNameEdit),
            1 => nav.replace(ScreenId::DeviceIdEdit),
            2 => nav.emit(Intent::RegenerateDeviceId),
            _ => {}
        }
    }
}
