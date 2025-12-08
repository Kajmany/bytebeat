use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::DefaultTerminal;

use crate::{
    audio::{AudioEvent, StreamStatus},
    event::{Event, EventHandler},
};

mod ui;

pub struct App {
    pub running: bool,
    pub events: EventHandler,
    // As we wish it.
    pub paused: bool,
    // May be streaming or paused, but also other things too
    pub audio_state: StreamStatus,
    /// Representing a valid interpretable bytebeat code
    // TODO: undo/redo system shouldn't be that hard. later.
    pub beat: String,
}

impl App {
    pub fn new(events: EventHandler) -> Self {
        Self {
            running: true,
            events,
            paused: true,
            audio_state: StreamStatus::Unconnected,
            // TODO: Not a pretty way to do defaults
            beat: "t*(42&t>>10)".to_owned(),
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

    /// Try-compile and play new are one operation from the user's perspective
    fn try_beat(&self) {
        // FIXME: Display error to user
        self.events.new_beat(&self.beat).unwrap();
    }

    /// Causes break and clean exit on next [`App::run`] loop
    fn quit(&mut self) {
        self.running = false;
    }
}
