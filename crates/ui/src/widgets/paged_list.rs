//! Reusable full-width paged choice list (SSID / menu / servers / language).

use crate::view::{GridView, fill_list_page, items_fitting, page_start};

/// Visible slice of a wrap-aware list page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageLayout {
    pub start: usize,
    pub count: usize,
    pub has_next: bool,
}

/// Cursor + page for a wrap-aware choice list.
#[derive(Clone, Copy, Debug)]
pub struct PagedList {
    pub page: usize,
    /// Index of the highlighted item **on the current page**.
    pub cursor: usize,
    /// Prefix visible rows with global 1-based numbers (`1:`… continuing across pages).
    pub numbered: bool,
}

impl Default for PagedList {
    fn default() -> Self {
        Self::new(true)
    }
}

impl PagedList {
    pub const fn new(numbered: bool) -> Self {
        Self {
            page: 0,
            cursor: 0,
            numbered,
        }
    }

    pub fn reset(&mut self) {
        self.page = 0;
        self.cursor = 0;
    }

    /// First item index, visible count, and whether another page exists.
    pub fn layout(&self, items: &[&str], height: u16) -> PageLayout {
        let start = page_start(items, self.page, self.numbered, height);
        let count = items_fitting(items, start, self.numbered, height);
        PageLayout {
            start,
            count,
            has_next: start + count < items.len(),
        }
    }

    pub fn draw(&self, g: &mut GridView, title: Option<&str>, items: &[&str], height: u16) {
        if let Some(title) = title {
            g.set(0, title, false);
        }
        fill_list_page(g, items, self, height);
    }

    pub fn global_index(&self, items: &[&str], height: u16) -> usize {
        self.layout(items, height).start + self.cursor
    }

    pub fn list_prev(&mut self, items: &[&str], height: u16) {
        let layout = self.layout(items, height);
        if layout.count == 0 {
            return;
        }
        if self.cursor == 0 {
            if self.page > 0 {
                self.page -= 1;
                self.cursor = self.layout(items, height).count.saturating_sub(1);
            } else {
                self.cursor = layout.count - 1;
            }
        } else {
            self.cursor -= 1;
        }
    }

    pub fn list_next(&mut self, items: &[&str], height: u16) {
        let layout = self.layout(items, height);
        if layout.count == 0 {
            return;
        }
        if self.cursor + 1 >= layout.count {
            if layout.has_next {
                self.page += 1;
                self.cursor = 0;
            } else {
                self.cursor = 0;
            }
        } else {
            self.cursor += 1;
        }
    }

    pub fn page_prev(&mut self, _items: &[&str], _height: u16) {
        if self.page > 0 {
            self.page -= 1;
            self.cursor = 0;
        }
    }

    pub fn page_next(&mut self, items: &[&str], height: u16) {
        if self.layout(items, height).has_next {
            self.page += 1;
            self.cursor = 0;
        }
    }

    /// Digit matches the **displayed** number on this page (`1`–`9`, `0` = 10).
    pub fn select_digit(&mut self, n: u8, items: &[&str], height: u16) -> Option<usize> {
        let layout = self.layout(items, height);
        if !self.numbered {
            let idx = n as usize;
            if idx < layout.count {
                self.cursor = idx;
                return Some(layout.start + idx);
            }
            return None;
        }
        let want = crate::view::digit_display_num(n);
        for local in 0..layout.count {
            let global = layout.start + local;
            if crate::view::item_display_num(global) == want {
                self.cursor = local;
                return Some(global);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: u16 = 64;

    #[test]
    fn numbered_wraps_next_and_prev_on_one_page() {
        let items = ["a", "b", "c"];
        let mut list = PagedList::new(true);
        list.list_next(&items, H);
        list.list_next(&items, H);
        assert_eq!(list.global_index(&items, H), 2);
        list.list_next(&items, H);
        assert_eq!(list.global_index(&items, H), 0);
        list.list_prev(&items, H);
        assert_eq!(list.global_index(&items, H), 2);
    }

    #[test]
    fn page_next_advances_then_stops() {
        let items = ["one", "two", "three", "four", "five", "six", "seven"];
        let mut list = PagedList::new(true);
        let first = list.layout(&items, H);
        assert!(first.has_next, "64px list of 7 numbered rows should page");
        list.page_next(&items, H);
        assert_eq!(list.page, 1);
        assert_eq!(list.cursor, 0);
        list.page_next(&items, H);
        assert_eq!(
            list.page,
            if list.layout(&items, H).has_next {
                2
            } else {
                1
            }
        );
    }
}
