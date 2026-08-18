//! Reusable full-width paged choice list (SSID / menu / servers / language).
#![allow(missing_docs)]

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
    /// Reserve the last visible content row for a footer hint.
    pub footer: bool,
}

impl Default for PagedList {
    fn default() -> Self {
        Self::new(true)
    }
}

impl PagedList {
    #[must_use]
    pub const fn new(numbered: bool) -> Self {
        Self {
            page: 0,
            cursor: 0,
            numbered,
            footer: false,
        }
    }

    /// Reserve the last content row for a hint (same flag as paging).
    #[must_use]
    pub const fn with_footer(mut self, footer: bool) -> Self {
        self.footer = footer;
        self
    }

    pub fn reset(&mut self) {
        self.page = 0;
        self.cursor = 0;
    }

    /// First item index, visible count, and whether another page exists.
    #[must_use]
    pub fn layout(&self, items: &[&str], height: u16) -> PageLayout {
        let start = page_start(items, self.page, self.numbered, height, self.footer);
        let count = items_fitting(items, start, self.numbered, height, self.footer);
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

    #[must_use]
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

    /// Digit matches the **displayed** number (`1`–`9`, `0` = 10), on any page.
    pub fn select_digit(&mut self, n: u8, items: &[&str], height: u16) -> Option<usize> {
        if !self.numbered {
            let layout = self.layout(items, height);
            let idx = n as usize;
            if idx < layout.count {
                self.cursor = idx;
                return Some(layout.start + idx);
            }
            return None;
        }
        let want = crate::view::digit_display_num(n);
        for global in 0..items.len() {
            if crate::view::item_display_num(global) == want {
                self.focus_global(global, items, height);
                return Some(global);
            }
        }
        None
    }

    /// Jump to the row whose label starts with shortcut digit `n` (`"6 Jezyk"` + `6`).
    pub fn select_label_digit(&mut self, n: u8, items: &[&str], height: u16) -> Option<usize> {
        for (global, item) in items.iter().enumerate() {
            if crate::screens::helpers::label_shortcut_digit(item) == Some(n) {
                self.focus_global(global, items, height);
                return Some(global);
            }
        }
        None
    }

    /// Move page/cursor so `global` is highlighted.
    pub fn focus_global(&mut self, global: usize, items: &[&str], height: u16) {
        let mut idx = 0usize;
        let mut page = 0usize;
        while idx < items.len() {
            let count = items_fitting(items, idx, self.numbered, height, self.footer);
            if count == 0 {
                break;
            }
            if global < idx + count {
                self.page = page;
                self.cursor = global - idx;
                return;
            }
            idx += count;
            page += 1;
        }
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

    #[test]
    fn select_digit_zero_is_tenth_numbered_row() {
        let items = ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"];
        let mut list = PagedList::new(true);
        list.page_next(&items, H);
        assert_eq!(list.select_digit(0, &items, H), Some(9));
        assert_eq!(list.global_index(&items, H), 9);
    }

    #[test]
    fn select_label_digit_matches_leading_label_number() {
        let items = ["1 Net", "Manual", "6 Language", "5 Throttles -"];
        let mut list = PagedList::new(false);
        assert_eq!(list.select_label_digit(6, &items, H), Some(2));
        assert_eq!(list.global_index(&items, H), 2);
        assert_eq!(list.select_label_digit(5, &items, H), Some(3));
        assert_eq!(list.global_index(&items, H), 3);
    }

    #[test]
    fn long_item_uses_more_than_one_slot() {
        let items = ["abcdefghijklmnopqrstuvwxyz0123456789"];
        let list = PagedList::new(false);
        let layout = list.layout(&items, H);
        assert_eq!(layout.start, 0);
        assert_eq!(layout.count, 1);
        assert!(!layout.has_next);
    }

    #[test]
    fn no_footer_uses_full_page() {
        let six = ["a", "b", "c", "d", "e", "f"];
        let list = PagedList::new(false);
        let p64 = list.layout(&six, 64);
        assert_eq!(p64.count, 6);
        assert!(!p64.has_next);
        let three = ["a", "b", "c"];
        let p32 = list.layout(&three, 32);
        assert_eq!(p32.count, 3);
        assert!(!p32.has_next);
    }

    #[test]
    fn footer_pages_sixth_item_on_64() {
        let items = ["a", "b", "c", "d", "e", "f"];
        let list = PagedList::new(true).with_footer(true);
        let first = list.layout(&items, 64);
        assert_eq!(first.count, 5);
        assert!(first.has_next);
        let mut list = list;
        list.page_next(&items, 64);
        assert_eq!(list.layout(&items, 64).count, 1);
        assert_eq!(list.global_index(&items, 64), 5);
    }

    #[test]
    fn draw_with_footer_does_not_write_hint_row() {
        let items = ["a", "b", "c", "d", "e", "f"];
        let list = PagedList::new(true).with_footer(true);
        let mut g = GridView::new();
        list.draw(&mut g, Some("Title"), &items, 64);
        assert_eq!(g.lines[5].as_str(), "5:e");
        assert!(g.lines.get(6).is_none_or(|l| l.is_empty()));
        let mut g32 = GridView::new();
        list.draw(&mut g32, Some("Title"), &items, 32);
        assert_eq!(g32.lines[2].as_str(), "2:b");
        assert!(g32.lines.get(3).is_none_or(|l| l.is_empty()));
    }

    #[test]
    fn footer_pages_third_item_on_32() {
        let items = ["a", "b", "c"];
        let list = PagedList::new(true).with_footer(true);
        let first = list.layout(&items, 32);
        assert_eq!(first.count, 2);
        assert!(first.has_next);
        let mut list = list;
        list.page_next(&items, 32);
        assert_eq!(list.layout(&items, 32).count, 1);
        assert_eq!(list.global_index(&items, 32), 2);
    }

    #[test]
    fn numbered_digit_selects_item_off_current_page() {
        let items = ["a", "b", "c", "d", "e"];
        let mut list = PagedList::new(true).with_footer(true);
        assert_eq!(list.select_digit(5, &items, 32), Some(4));
        assert_eq!(list.global_index(&items, 32), 4);
        assert_eq!(list.page, 2);
    }
}
