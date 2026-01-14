//! Component which allows listening to hard-coded songs or replacing input buffer with them
//!
//! FIXME: Mediocre performance and readability because of reliant on slopped table submod
use crossterm::event::KeyCode;
use ratatui::{
    layout::Constraint,
    style::{Color, Modifier, Style},
    text::Text,
    widgets::{Block, BorderType, Row, StatefulWidget, Widget},
};

use crate::{
    app::{AppEvent, Component},
    library_data::{SONGS, Song},
};

pub mod dynatable;
use dynatable::{DynaTable, DynaTableState, key_char_for_index};

const CODE_TRUNCATE_LEN: usize = 40;

fn truncate_code(code: &str) -> String {
    let first_line = code.lines().next().unwrap_or("");
    if first_line.len() > CODE_TRUNCATE_LEN {
        format!("{}…", &first_line[..CODE_TRUNCATE_LEN])
    } else {
        first_line.to_string()
    }
}

#[derive(Debug, Default)]
pub struct Library {
    table_state: DynaTableState,
}

impl Library {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn selected_song(&self) -> Option<&'static Song> {
        self.table_state.selected_index().map(|idx| &SONGS[idx])
    }
}

impl Component for Library {
    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> Option<AppEvent> {
        match key.code {
            KeyCode::PageUp | KeyCode::Left => {
                self.table_state.prev_page();
            }
            KeyCode::PageDown | KeyCode::Right => {
                self.table_state.next_page();
            }
            KeyCode::Up => {
                self.table_state.select_prev();
            }
            KeyCode::Down => {
                self.table_state.select_next();
            }
            // Enter overwrites the input with the song
            KeyCode::Enter => {
                if let Some(song) = self.selected_song() {
                    return Some(AppEvent::BeatOverwrite(song.code.to_string()));
                }
            }
            // Selecting any song samples it by playing without touching buffer
            KeyCode::Char(c) => {
                self.table_state.select_by_key(c);
                if let Some(song) = self.selected_song() {
                    return Some(AppEvent::InputReady(song.code.to_string()));
                }
            }
            _ => {}
        }
        None
    }
}

impl Widget for &mut Library {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        let total_pages = SONGS
            .len()
            .div_ceil(self.table_state.items_per_page().max(1));
        let page_info = format!(
            " Library (Page {}/{}) ",
            self.table_state.current_page() + 1,
            total_pages.max(1)
        );

        let block = Block::bordered()
            .title(page_info)
            .border_type(BorderType::Rounded);

        let header = Row::new(vec!["Key", "Author", "Name", "Description", "Code"])
            .style(Style::default().add_modifier(Modifier::BOLD))
            .height(1);

        let widths = vec![
            Constraint::Length(3),      // Key
            Constraint::Percentage(15), // Author
            Constraint::Percentage(20), // Name
            Constraint::Percentage(25), // Description
            Constraint::Percentage(35), // Code (truncated)
        ];

        let table = DynaTable::new(SONGS.len(), widths, |idx, local_idx| {
            let song = &SONGS[idx];
            Row::new(vec![
                Text::from(key_char_for_index(local_idx).to_string()),
                Text::from(song.author),
                Text::from(song.name),
                Text::from(song.description),
                Text::from(truncate_code(song.code)),
            ])
            .height(1)
        })
        .header(header)
        .block(block)
        .row_height(1)
        .row_highlight_style(
            Style::default()
                .add_modifier(Modifier::REVERSED)
                .bg(Color::Cyan)
                .fg(Color::Black),
        );

        StatefulWidget::render(table, area, buf, &mut self.table_state);
    }
}
