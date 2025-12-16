//! Input widget for editing (and submitting) a bytebeat code
//!
//! Probably doesn't handled grapheme clusters prettily, but theoretically
//! unicode-respecting if 'add' is used carefully.
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph, Widget},
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{app::ui, parser::ParseError};
/// Extremely simple single-buffer utf-8 input widget for small texts.
#[derive(Default, Debug)]
pub struct LineInput {
    // 0-Indexed. cursor == len represents append
    cursor: usize,
    buf: Vec<char>,
}

impl LineInput {
    /// Convenience method that clones the input and sets the cursor to the end.
    pub fn from_str(s: &str) -> Self {
        LineInput {
            cursor: s.len(),
            buf: s.chars().collect(),
        }
    }

    /// Insert a character at the cursor.
    pub fn add(&mut self, c: char) {
        self.buf.insert(self.cursor, c);
        self.cursor += 1;
    }

    /// Remove the character before the cursor.
    pub fn remove(&mut self) {
        if self.buf.is_empty() || self.cursor == 0 {
            return;
        }

        self.buf.remove(self.cursor - 1);
        self.cursor -= 1;
    }

    /// Move the cursor count left, or remain at the start.
    pub fn shift_left(&mut self, count: usize) {
        self.cursor = self.cursor.saturating_sub(count);
    }

    /// Move the cursor left until it is ahead of the nearest whitespace, or go to the start.
    pub fn jump_left(&mut self) {
        // We try not move to not get stuck on current whitespace
        let mut i = self.cursor.saturating_sub(1).min(self.buf.len());
        while i > 0 && !self.buf[i - 1].is_whitespace() {
            i -= 1;
        }
        self.cursor = i;
    }

    /// Move the cursor right until it is ahead of the nearest whitespace, or go to the end.
    pub fn jump_right(&mut self) {
        let mut i = (self.cursor + 1).min(self.buf.len());
        while i < self.buf.len() && !self.buf[i - 1].is_whitespace() {
            i += 1;
        }
        self.cursor = i;
    }

    /// Move the cursor count right, or remain at the end.
    pub fn shift_right(&mut self, count: usize) {
        self.cursor = (self.cursor + count).min(self.buf.len());
    }

    /// Are we at the end and would be appending?
    pub fn at_end(&self) -> bool {
        self.cursor == self.buf.len()
    }

    /// Are we at the start?
    pub fn at_start(&self) -> bool {
        self.cursor == 0
    }

    /// O(n) + an alloc, probably.
    pub fn get_buffer(&self) -> String {
        String::from_iter(&self.buf)
    }
}

impl Widget for &LineInput {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let (left, cursor_char, right) = if self.at_end() {
            // Draw the underscore at the far right as our cursor
            (String::from_iter(&self.buf), "_".to_owned(), None)
        } else {
            // Split before and after cursor so we can style the one char
            (
                String::from_iter(&self.buf[..self.cursor]),
                self.buf[self.cursor].to_string(),
                Some(String::from_iter(&self.buf[self.cursor + 1..])),
            )
        };

        let cursor_styled =
            Span::styled(cursor_char, Style::new().add_modifier(Modifier::REVERSED));

        let mut renderable = Line::default();
        renderable.push_span(left);
        renderable.push_span(cursor_styled);
        if let Some(right) = right {
            renderable.push_span(right);
        }
        renderable.render(area, buf);
    }
}

/// Wraps `[LineInput]` for top-level display. Responsible for displaying errors, too.
#[derive(Debug, Default)]
pub struct BeatInput {
    input: LineInput,
    errors: Vec<ParseError>,
}

impl BeatInput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_str(s: &str) -> Self {
        Self {
            input: LineInput::from_str(s),
            errors: Vec::new(),
        }
    }

    pub fn handle_key_event(&mut self, event: KeyEvent) {
        match event.code {
            KeyCode::Backspace => {
                self.input.remove();
            }
            KeyCode::Char(c) => {
                if !c.is_control() {
                    self.input.add(c);
                }
            }
            KeyCode::Left => {
                if event.modifiers.contains(KeyModifiers::CONTROL) {
                    self.input.jump_left();
                } else {
                    self.input.shift_left(1);
                }
            }
            KeyCode::Right => {
                if event.modifiers.contains(KeyModifiers::CONTROL) {
                    self.input.jump_right();
                } else {
                    self.input.shift_right(1);
                }
            }
            _ => {}
        }
    }

    pub fn set_errors(&mut self, errors: Vec<ParseError>) {
        self.errors = errors;
    }

    pub fn clear_errors(&mut self) {
        self.errors.clear();
    }

    pub fn get_buffer(&self) -> String {
        self.input.get_buffer()
    }

    pub fn height_hint(&self) -> u16 {
        // 2 for the block, 1 for the LineInput, up to n errors + 1 'n more...'
        (2 + 1 + self.errors.len().min(ui::MAX_ERRORS_SHOWN + 1)) as u16
    }
}

impl Widget for &BeatInput {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let block = Block::bordered()
            .title(" Input ")
            .border_type(BorderType::Rounded);

        let inner_area = block.inner(area);
        block.render(area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(inner_area);

        self.input.render(chunks[0], buf);

        if !self.errors.is_empty() {
            let mut error_text: Vec<Line> = self
                .errors
                .iter()
                .take(ui::MAX_ERRORS_SHOWN)
                .map(|e| {
                    Line::from(vec![Span::styled(
                        format!("Error: {}", e),
                        Style::default().fg(Color::Red),
                    )])
                })
                .collect();

            if self.errors.len() > ui::MAX_ERRORS_SHOWN {
                error_text.push(Line::from(vec![Span::styled(
                    format!("...and {} more", self.errors.len() - ui::MAX_ERRORS_SHOWN),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )]));
            }

            Paragraph::new(error_text).render(chunks[1], buf);
        }
    }
}
