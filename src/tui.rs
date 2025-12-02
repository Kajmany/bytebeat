use color_eyre::Result;
use crossterm::event::{KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal,
    buffer::Buffer,
    layout::{Alignment, Rect},
    widgets::{Block, BorderType, Paragraph, Widget},
};

use crate::event::{Event, EventHandler};

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
}

impl Default for App {
    fn default() -> Self {
        Self {
            running: true,
            events: EventHandler::new(),
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
            _ => {}
        }
        Ok(())
    }

    fn handle_key_event(&mut self, _event: KeyEvent) {
        self.quit();
    }

    // Causes break and clean exit on next `run` loop
    pub fn quit(&mut self) {
        self.running = false;
    }
}

// TODO: break out to UI module when it gets too complicated
impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .title("{{project-name}}")
            .title_alignment(Alignment::Left)
            .border_type(BorderType::Rounded);

        let paragraph = Paragraph::new("Test text, please ignore.")
            .block(block)
            .centered();

        paragraph.render(area, buf);
    }
}
