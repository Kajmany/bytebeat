use std::thread;

use color_eyre::Result;
use crossterm::event::{KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal,
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
};

use crate::{
    event::{Event, EventHandler},
    pipewire::StreamStatus,
};

pub fn main() -> Result<()> {
    let terminal = ratatui::init();
    let result = App::new().run(terminal);
    ratatui::restore();

    result
}

#[derive(Debug)]
pub struct App {
    pub running: bool,
    pub events: EventHandler,
    pub audio_state: StreamStatus,
}

impl Default for App {
    fn default() -> Self {
        let events = EventHandler::new();
        let event_tx = events.get_sender();
        // TODO: Do we need to watch this better?
        thread::spawn(move || crate::pipewire::main(event_tx));
        Self {
            running: true,
            events,
            audio_state: StreamStatus::Unconnected,
        }
    }
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn run(mut self, mut term: DefaultTerminal) -> Result<()> {
        while self.running {
            term.draw(|frame| frame.render_widget(&mut self, frame.area()))?;
            self.update()?;
        }
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        match self.events.next()? {
            Event::Crossterm(event) => match event {
                crossterm::event::Event::Key(event) if event.kind == KeyEventKind::Press => {
                    self.handle_key_event(event)
                }
                _ => {}
            },
            Event::Audio(crate::pipewire::AudioEvent::StateChange(event)) => {
                self.audio_state = event
            }
        }
        Ok(())
    }

    fn handle_key_event(&mut self, _event: KeyEvent) {
        self.quit();
    }

    /// Causes break and clean exit on next [`App::run`] loop
    pub fn quit(&mut self) {
        self.running = false;
    }
}

// TODO: break out to UI module when it gets too complicated
impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let main_block = Block::bordered()
            .title(" bytebeat   ")
            .title_alignment(Alignment::Left)
            .border_type(BorderType::Rounded);

        let main_interior = Layout::default()
            .direction(Direction::Vertical)
            // One big widget area, and a little bottom bar
            .constraints(vec![Constraint::Percentage(100), Constraint::Min(2)])
            .split(main_block.inner(area));

        let status_block = Block::new()
            .borders(Borders::TOP)
            .border_type(BorderType::Plain);

        let stream_status = match self.audio_state {
            StreamStatus::Error => "Audio: Error!",
            StreamStatus::Unconnected => "Audio: Unconnected",
            StreamStatus::Connecting => "Audio: Connecting",
            StreamStatus::Paused => "Audio: Paused",
            StreamStatus::Streaming => "Audio: Streaming",
        };

        main_block.render(area, buf);
        // Dummy text (for now)
        Paragraph::new("Test text, please ignore.")
            .centered()
            .render(main_interior[0], buf);
        // Status bar text must be rendered before status bar
        Paragraph::new(stream_status)
            .centered()
            .render(status_block.inner(main_interior[1]), buf);
        status_block.render(main_interior[1], buf);
    }
}
