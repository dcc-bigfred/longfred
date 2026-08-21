//! Device name / numeric id summary.

use longfred_proto::persist::DEVICE_ID_MIN;

use super::helpers::write_u16_padded;
use crate::context::ScreenCtx;
use crate::intent::Intent;
use crate::nav::{Nav, ScreenId, Step};
use crate::screen::Screen;
use crate::view::{Line, UiView};

/// Device name / numeric id summary.
pub struct DeviceScreen {
    cursor: DeviceRow,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DeviceRow {
    Name,
    Id,
    NewId,
}

impl DeviceRow {
    fn step(self, d: Step) -> Self {
        match (self, d) {
            (Self::Name, Step::Prev) | (Self::Id, Step::Next) => Self::NewId,
            (Self::Name, Step::Next) | (Self::NewId, Step::Prev) => Self::Id,
            (Self::Id, Step::Prev) | (Self::NewId, Step::Next) => Self::Name,
        }
    }
}

impl DeviceScreen {
    /// Three-row picker: name, id, regenerate.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cursor: DeviceRow::Name,
        }
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
        self.cursor = DeviceRow::Name;
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
        g.set(
            3,
            cx.s.device_name_id,
            matches!(self.cursor, DeviceRow::Name | DeviceRow::Id),
        );
        g.set(4, cx.s.device_new_id, self.cursor == DeviceRow::NewId);
        g.set(5, cx.s.hint_device, false);
        UiView::Grid(g)
    }

    /// Cycle name / id / regenerate.
    fn on_list_step(&mut self, d: Step, _cx: &mut ScreenCtx<'_>, _nav: &mut Nav<'_>) {
        self.cursor = self.cursor.step(d);
    }

    /// Replace with a name/id editor (Back → Extras) or emit a new random id.
    fn on_select(&mut self, _cx: &mut ScreenCtx<'_>, nav: &mut Nav<'_>) {
        match self.cursor {
            DeviceRow::Name => nav.replace(ScreenId::DeviceNameEdit),
            DeviceRow::Id => nav.replace(ScreenId::DeviceIdEdit),
            DeviceRow::NewId => nav.emit(Intent::RegenerateDeviceId),
        }
    }
}
