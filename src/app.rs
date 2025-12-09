use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use tracing::{info, trace};

use crate::{
    app::input::LineInput,
    audio::{AudioEvent, StreamStatus},
    event::{Event, EventHandler},
};

mod input;
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
    pub beat_input: LineInput,
}

impl App {
    pub fn new(events: EventHandler) -> Self {
        Self {
            running: true,
            events,
            paused: true,
            audio_state: StreamStatus::Unconnected,
            // TODO: Not a pretty way to do defaults
            beat_input: LineInput::from_str("t*(42&t>>10)"),
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
                    trace!("app handling crossterm event: {:?}", event);
                    self.handle_key_event(event)
                }
                _ => {}
            },
            Event::Audio(AudioEvent::StateChange(event)) => {
                info!("app recieved audio state change: {:?}", event);
                self.audio_state = event;
            }
        }
        Ok(())
    }

    fn handle_key_event(&mut self, event: KeyEvent) {
        match event.code {
            // We can always exit
            KeyCode::F(3) => self.quit(),
            KeyCode::F(4) => self.toggle_playback(),
            // For now, we assume we're always focused on the input field
            KeyCode::Backspace => {
                self.beat_input.remove();
            }
            KeyCode::Char(c) => {
                // This could be any unicode so this doesn't mean we're safe
                // TODO: I won't be more strict yet b/c I don't want to ruin unicode fun
                if !c.is_control() {
                    self.beat_input.add(c);
                }
            }
            KeyCode::Enter => self.try_beat(),
            KeyCode::Left => {
                if event.modifiers.contains(KeyModifiers::CONTROL) {
                    self.beat_input.jump_left();
                } else {
                    self.beat_input.shift_left(1);
                }
            }
            KeyCode::Right => {
                if event.modifiers.contains(KeyModifiers::CONTROL) {
                    self.beat_input.jump_right();
                } else {
                    self.beat_input.shift_right(1);
                }
            }
            // This ain't emacs, pal.
            _ => {}
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
        self.events.new_beat(&self.beat_input.get_buffer()).unwrap();
    }

    /// Causes break and clean exit on next [`App::run`] loop
    fn quit(&mut self) {
        trace!("app quit requested");
        self.running = false;
    }
}
