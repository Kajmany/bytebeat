use std::thread;

use color_eyre::Result;
use crossterm::event::{KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal,
    buffer::Buffer,
    layout::{Alignment, Rect},
    text::Line,
    widgets::{Block, BorderType, Paragraph, Widget},
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
        let stream_status = match self.audio_state {
            StreamStatus::Error => "Stream Status: Error!",
            StreamStatus::Unconnected => "Stream Status: Unconnected",
            StreamStatus::Connecting => "Stream Status: Connecting",
            StreamStatus::Paused => "Stream Status: Paused",
            StreamStatus::Streaming => "Stream Status: Streaming",
        };
        let status = Line::raw(stream_status);

        let block = Block::bordered()
            .title(" bytebeat   ")
            .title_alignment(Alignment::Left)
            .border_type(BorderType::Rounded)
            .title_bottom(status);

        let paragraph = Paragraph::new("Test text, please ignore.")
            .block(block)
            .centered();

        paragraph.render(area, buf);
    }
}
