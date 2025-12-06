use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal,
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders, Paragraph, Widget},
};

use crate::{
    audio::{AudioEvent, StreamStatus},
    event::{Event, EventHandler},
};

pub fn main(events: EventHandler) -> Result<()> {
    let terminal = ratatui::init();
    let result = App::new(events).run(terminal);
    ratatui::restore();

    result
}

pub struct App {
    pub running: bool,
    pub events: EventHandler,
    // As we wish it.
    pub paused: bool,
    // May be streaming or paused, but also other things too
    pub audio_state: StreamStatus,
}

impl App {
    pub fn new(events: EventHandler) -> Self {
        Self {
            running: true,
            events,
            paused: true,
            audio_state: StreamStatus::Unconnected,
        }
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
            Event::Audio(AudioEvent::StateChange(event)) => self.audio_state = event,
        }
        Ok(())
    }

    fn handle_key_event(&mut self, event: KeyEvent) {
        match event.code {
            // We can always exit
            KeyCode::F(3) => self.quit(),
            KeyCode::Char('p') | KeyCode::Char('P') => self.toggle_playback(),
            _ => {} //TODO: This
        }
    }

    // Sync with actual stream state absolutely not guaranteed.
    // TODO: if it matters, we can wait for a statechange event to change this
    // and even debounce while waiting
    fn toggle_playback(&mut self) {
        match self.paused {
            true => {
                self.events.stream_play();
                self.paused = false;
            }
            false => {
                self.events.stream_pause();
                self.paused = true;
            }
        };
    }

    /// Causes break and clean exit on next [`App::run`] loop
    fn quit(&mut self) {
        self.running = false;
    }
}

// TODO: break out to UI module when it gets too complicated
impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut control_str: String = " <F3>: Quit | <P>: ".to_owned();
        match self.paused {
            true => control_str.push_str("Play "),
            false => control_str.push_str("Pause "),
        };

        let main_block = Block::bordered()
            .title(" bytebeat   ")
            .title_alignment(Alignment::Left)
            .border_type(BorderType::Rounded)
            .title_bottom(control_str);

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
            .style(Style::default().add_modifier(Modifier::BOLD))
            .render(status_block.inner(main_interior[1]), buf);
        status_block.render(main_interior[1], buf);
    }
}
