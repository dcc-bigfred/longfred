//! Reusable full-width paged choice list (SSID / menu / servers / language).

use crate::view::{GridView, fill_list_page, has_next_page, page_item_count, page_start};

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

    pub fn draw(
        &self,
        g: &mut GridView,
        title: Option<&str>,
        items: &[&str],
        numbered: bool,
        height: u16,
    ) {
        if let Some(title) = title {
            g.set(0, title, false);
        }
        fill_list_page(g, items, self.page, self.cursor, numbered, height);
    }

    pub fn visible_count(&self, items: &[&str], numbered: bool, height: u16) -> usize {
        page_item_count(items, self.page, numbered, height)
    }

    pub fn global_index(&self, items: &[&str], numbered: bool, height: u16) -> usize {
        page_start(items, self.page, numbered, height) + self.cursor
    }

    pub fn list_prev(&mut self, items: &[&str], numbered: bool, height: u16) {
        let count = self.visible_count(items, numbered, height);
        if count == 0 {
            return;
        }
        if self.cursor == 0 {
            if self.page > 0 {
                self.page -= 1;
                self.cursor = self
                    .visible_count(items, numbered, height)
                    .saturating_sub(1);
            } else {
                self.cursor = count - 1;
            }
        } else {
            self.cursor -= 1;
        }
    }

    pub fn list_next(&mut self, items: &[&str], numbered: bool, height: u16) {
        let count = self.visible_count(items, numbered, height);
        if count == 0 {
            return;
        }
        if self.cursor + 1 >= count {
            if has_next_page(items, self.page, numbered, height) {
                self.page += 1;
                self.cursor = 0;
            } else {
                self.cursor = 0;
            }
        } else {
            self.cursor += 1;
        }
    }

    pub fn page_prev(&mut self, items: &[&str], _height: u16) {
        let _ = items;
        if self.page > 0 {
            self.page -= 1;
            self.cursor = 0;
        }
    }

    pub fn page_next(&mut self, items: &[&str], numbered: bool, height: u16) {
        if has_next_page(items, self.page, numbered, height) {
            self.page += 1;
            self.cursor = 0;
        }
    }

    /// Move cursor/page so `global` (0-based) is the highlighted item.
    pub fn jump_to_global(
        &mut self,
        items: &[&str],
        numbered: bool,
        height: u16,
        global: usize,
    ) -> bool {
        if global >= items.len() {
            return false;
        }
        let mut page = 0usize;
        loop {
            let start = page_start(items, page, numbered, height);
            let count = page_item_count(items, page, numbered, height);
            if count == 0 {
                return false;
            }
            if global >= start && global < start + count {
                self.page = page;
                self.cursor = global - start;
                return true;
            }
            if !has_next_page(items, page, numbered, height) {
                return false;
            }
            page = page.saturating_add(1);
            if page > items.len() {
                return false;
            }
        }
    }

    /// Digit matches the **displayed** number on this page (`1`–`9`, `0` = 10).
    pub fn select_digit(
        &mut self,
        n: u8,
        items: &[&str],
        numbered: bool,
        height: u16,
    ) -> Option<usize> {
        if !numbered {
            let idx = n as usize;
            if idx < self.visible_count(items, numbered, height) {
                self.cursor = idx;
                return Some(self.global_index(items, numbered, height));
            }
            return None;
        }
        let want = crate::view::digit_display_num(n);
        let start = page_start(items, self.page, numbered, height);
        let count = self.visible_count(items, numbered, height);
        for local in 0..count {
            let global = start + local;
            if crate::view::item_display_num(global) == want {
                self.cursor = local;
                return Some(global);
            }
        }
        None
    }
}
