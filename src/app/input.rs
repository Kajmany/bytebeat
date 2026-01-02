//! Contains 'inputs' which hold potential beats as strings.
//!
//! The stdio-interactive input and file watcher ''input'' are both barbarically forced to implement
//! the same trait [`BeatInput`].
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph, Widget, WidgetRef},
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{app::ui, parser::ParseError};

/// Private trait for error storage, used to provide blanket implementations.
// TODO: More common error display functionality could be packed in here
trait ErrorStore {
    fn errors_mut(&mut self) -> &mut Vec<ParseError>;
}

#[expect(private_bounds)]
pub trait BeatInput: WidgetRef + ErrorStore {
    /// Only used for interactive input.
    fn handle_key_event(&mut self, event: KeyEvent);
    /// Only used for file watch input
    fn handle_watch_event(&mut self, event: notify::Event);
    fn get_buffer(&self) -> String;
    fn height_hint(&self) -> u16;
    /// Just used for visuals.
    fn tick(&mut self);

    fn clear_errors(&mut self) {
        self.errors_mut().clear();
    }

    fn set_errors(&mut self, errors: Vec<ParseError>) {
        *self.errors_mut() = errors;
    }
}

/// Extremely simple single-buffer utf-8 input widget for small texts.
///
/// Performs poorly with grapheme clusters (emoji, scripts, etc) but won't crash, or anything.
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

/// Renders a list of parse errors into the given area.
/// Displays up to `MAX_ERRORS_SHOWN` errors, with a summary line if there are more.
fn render_errors(
    errors: &[ParseError],
    area: ratatui::prelude::Rect,
    buf: &mut ratatui::prelude::Buffer,
) {
    if errors.is_empty() {
        return;
    }

    let mut error_text: Vec<Line> = errors
        .iter()
        .take(ui::MAX_ERRORS_SHOWN)
        .map(|e| {
            Line::from(vec![Span::styled(
                format!("Error: {}", e),
                Style::default().fg(Color::Red),
            )])
        })
        .collect();

    if errors.len() > ui::MAX_ERRORS_SHOWN {
        error_text.push(Line::from(vec![Span::styled(
            format!("...and {} more", errors.len() - ui::MAX_ERRORS_SHOWN),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )]));
    }

    Paragraph::new(error_text).render(area, buf);
}

/// Input widget for editing (and submitting) a bytebeat code and displaying errors.
/// Most functionality is that of [`LineInput`]
#[derive(Debug, Default)]
pub struct InteractiveInput {
    input: LineInput,
    errors: Vec<ParseError>,
}

impl ErrorStore for InteractiveInput {
    fn errors_mut(&mut self) -> &mut Vec<ParseError> {
        &mut self.errors
    }
}

impl BeatInput for InteractiveInput {
    fn handle_watch_event(&mut self, _event: notify::Event) {
        // No-op.
    }

    fn tick(&mut self) {
        // No-op.
        // TODO: flash cursor? that'd be neat.
    }

    fn handle_key_event(&mut self, event: KeyEvent) {
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

    fn get_buffer(&self) -> String {
        self.input.get_buffer()
    }

    fn height_hint(&self) -> u16 {
        // 2 for the block, 1 for the LineInput, up to n errors + 1 'n more...'
        (2 + 1 + self.errors.len().min(ui::MAX_ERRORS_SHOWN + 1)) as u16
    }
}

impl WidgetRef for InteractiveInput {
    fn render_ref(&self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
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

        render_errors(&self.errors, chunks[1], buf);
    }
}

/// Reads a file when given certain types of watch event. Does not visually display buffer, but does display errors.
///
/// Relies on a [`notify::Watcher`] it isn't exposed to directly.
//  TODO: Should the file I/O be hoisted up into App?
#[derive(Default, Debug)]
pub struct FileWatchInput {
    blinken: bool,
    blinken_timer: u16,
    buffer: String,
    errors: Vec<ParseError>,
}

impl ErrorStore for FileWatchInput {
    fn errors_mut(&mut self) -> &mut Vec<ParseError> {
        &mut self.errors
    }
}

impl BeatInput for FileWatchInput {
    fn handle_key_event(&mut self, _event: KeyEvent) {
        // No-op.
    }

    fn tick(&mut self) {
        // It's entirely coincidence that this blinkenlight isn't a total lie:
        // the ticks come from the same event loop that poll the watcher
        // but we're supposed to panic if something in the loop breaks, so this
        // probably still isn't technically useful. It looks cool though.
        self.blinken_timer += 1;
        if self.blinken_timer >= crate::event::TICK_FPS as u16 {
            self.blinken = !self.blinken;
            self.blinken_timer = 0;
        }
    }

    fn handle_watch_event(&mut self, event: notify::Event) {
        match event.kind {
            // TODO: FIXME: This should be more robust. Consider out of order events where we try to read a deleted file
            // or we try to read a folder, etc.
            notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {
                self.buffer = std::fs::read_to_string(event.paths.first().unwrap()).unwrap();
            }
            _ => {}
        }
    }

    fn get_buffer(&self) -> String {
        self.buffer.clone()
    }

    fn height_hint(&self) -> u16 {
        // 2 for the block, up to n errors + 1 'n more...' (no buffer displayed)
        (2 + self.errors.len().min(ui::MAX_ERRORS_SHOWN + 1)) as u16
    }
}

impl WidgetRef for FileWatchInput {
    fn render_ref(&self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        let indicator = if self.blinken { "◉" } else { "○" };
        let block = Block::bordered()
            .title(format!(" Watching {} ", indicator))
            .border_type(BorderType::Rounded);

        let inner_area = block.inner(area);
        block.render(area, buf);

        render_errors(&self.errors, inner_area, buf);
    }
}
