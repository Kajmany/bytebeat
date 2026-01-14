//! Wrapped [`ratatui::widgets::Table`] with pagination and selection by key (inspired by Cataclysm DDA)
//!
//! FIXME: This is the more powerful table I need, but it's very slopped
//! needs a pass/re-write to have a clear API and more features like filtering
use ratatui::{
    layout::Constraint,
    style::Style,
    widgets::{Block, Row, StatefulWidget, Table},
};

const MAX_KEYS: usize = 62; // 0-9 (10) + a-z (26) + A-Z (26)

fn index_to_key_char(index: usize) -> char {
    match index {
        0..=9 => (b'0' + index as u8) as char,
        10..=35 => (b'a' + (index - 10) as u8) as char,
        36..=61 => (b'A' + (index - 36) as u8) as char,
        _ => '?',
    }
}

fn key_char_to_index(c: char) -> Option<usize> {
    match c {
        '0'..='9' => Some((c as u8 - b'0') as usize),
        'a'..='z' => Some((c as u8 - b'a') as usize + 10),
        'A'..='Z' => Some((c as u8 - b'A') as usize + 36),
        _ => None,
    }
}

#[derive(Debug, Default, Clone)]
pub struct DynaTableState {
    // Which page has the user selected (0 inclusive)
    page: usize,
    // Which item has the user selected (0 inclusive)
    selection: Option<usize>,
    // Computed from last sync
    items_per_page: usize,
    total_items: usize,
}

impl DynaTableState {
    // Recalculate pagination based on available height and total items.
    // Call this at render time or when the item slice changes.
    // `usable_height` is the number of rows available for data (excluding header/borders).
    pub fn sync(&mut self, usable_height: u16, total_items: usize, row_height: u16) {
        self.total_items = total_items;

        // Calculate how many items fit, capped at MAX_KEYS
        let fits = if row_height == 0 {
            0
        } else {
            (usable_height / row_height) as usize
        };
        self.items_per_page = fits.clamp(1, MAX_KEYS);

        // Clamp page if it now exceeds total pages
        let total_pages = self.total_pages();
        if total_pages > 0 && self.page >= total_pages {
            self.page = total_pages - 1;
        }

        // Clamp selection if it exceeds current page size
        let page_len = self.page_len();
        if let Some(sel) = self.selection
            && sel >= page_len
        {
            self.selection = if page_len > 0 {
                Some(page_len - 1)
            } else {
                None
            };
        }
    }

    fn total_pages(&self) -> usize {
        if self.items_per_page == 0 {
            return 0;
        }
        self.total_items.div_ceil(self.items_per_page)
    }

    fn page_len(&self) -> usize {
        let start = self.page * self.items_per_page;
        let end = (start + self.items_per_page).min(self.total_items);
        end.saturating_sub(start)
    }

    pub fn page_range(&self) -> std::ops::Range<usize> {
        let start = self.page * self.items_per_page;
        let end = (start + self.items_per_page).min(self.total_items);
        start..end
    }

    pub fn current_page(&self) -> usize {
        self.page
    }

    pub fn items_per_page(&self) -> usize {
        self.items_per_page
    }

    // Returns the global index of the selected item, if any
    pub fn selected_index(&self) -> Option<usize> {
        self.selection
            .map(|local| self.page * self.items_per_page + local)
    }

    // Returns the local selection within the current page
    pub fn local_selection(&self) -> Option<usize> {
        self.selection
    }

    pub fn select_by_key(&mut self, c: char) -> bool {
        if let Some(local_index) = key_char_to_index(c)
            && local_index < self.page_len()
        {
            self.selection = Some(local_index);
            return true;
        }
        false
    }

    pub fn select_next(&mut self) {
        let page_len = self.page_len();
        if page_len == 0 {
            return;
        }
        self.selection = Some(match self.selection {
            Some(i) if i + 1 < page_len => i + 1,
            _ => 0,
        });
    }

    pub fn select_prev(&mut self) {
        let page_len = self.page_len();
        if page_len == 0 {
            return;
        }
        self.selection = Some(match self.selection {
            Some(i) if i > 0 => i - 1,
            _ => page_len.saturating_sub(1),
        });
    }

    pub fn next_page(&mut self) {
        if self.page + 1 < self.total_pages() {
            self.page += 1;
            self.selection = None;
        }
    }

    pub fn prev_page(&mut self) {
        if self.page > 0 {
            self.page -= 1;
            self.selection = None;
        }
    }

    // We'll TODO: this I swear dude
    #[allow(unused)]
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }
}

pub struct DynaTable<'a> {
    total_items: usize,
    row_constructor: Box<dyn Fn(usize, usize) -> Row<'a> + 'a>,
    header: Option<Row<'a>>,
    widths: Vec<Constraint>,
    block: Option<Block<'a>>,
    row_highlight_style: Style,
    row_height: u16,
}

impl<'a> DynaTable<'a> {
    // Create a new DynaTable with a constructor that takes (global_index, local_index) and returns a Row.
    pub fn new(
        total_items: usize,
        widths: Vec<Constraint>,
        constructor: impl Fn(usize, usize) -> Row<'a> + 'a,
    ) -> Self {
        Self {
            total_items,
            row_constructor: Box::new(constructor),
            header: None,
            widths,
            block: None,
            row_highlight_style: Style::default(),
            row_height: 1,
        }
    }

    pub fn header(mut self, header: Row<'a>) -> Self {
        self.header = Some(header);
        self
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    pub fn row_highlight_style(mut self, style: Style) -> Self {
        self.row_highlight_style = style;
        self
    }

    pub fn row_height(mut self, height: u16) -> Self {
        self.row_height = height;
        self
    }
}

impl StatefulWidget for DynaTable<'_> {
    type State = DynaTableState;

    fn render(
        self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
        state: &mut Self::State,
    ) {
        // Calculate usable height (subtract borders and header)
        let border_height: u16 = if self.block.is_some() { 2 } else { 0 };
        let header_height: u16 = if self.header.is_some() { 1 } else { 0 };
        let usable_height = area.height.saturating_sub(border_height + header_height);

        // Sync state with current dimensions
        state.sync(usable_height, self.total_items, self.row_height);

        // Generate rows for the current page
        let range = state.page_range();
        let page_rows: Vec<Row> = range
            .enumerate()
            .map(|(local_index, global_index)| (self.row_constructor)(global_index, local_index))
            .collect();

        // Build inner table
        let mut table =
            Table::new(page_rows, self.widths).row_highlight_style(self.row_highlight_style);

        if let Some(header) = self.header {
            table = table.header(header);
        }

        if let Some(block) = self.block {
            table = table.block(block);
        }

        // Create a ratatui TableState for the inner table
        let mut inner_state = ratatui::widgets::TableState::default();
        inner_state.select(state.local_selection());

        StatefulWidget::render(table, area, buf, &mut inner_state);
    }
}

// Re-export helper for key generation
pub fn key_char_for_index(index: usize) -> char {
    index_to_key_char(index)
}
