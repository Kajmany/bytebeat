use std::sync::atomic::AtomicI32;

use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::DefaultTerminal;
use tracing::{info, trace};

use crate::{
    app::input::BeatInput,
    audio::{AudioEvent, StreamStatus, Volume},
    event::{Event, EventHandler},
};

pub mod input; // TODO: Not pretty, has to be pub so we can make it in main :(
mod scope;
mod ui;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum View {
    #[default]
    Main,
    BigLog,
    Library,
}

pub struct App<I: BeatInput> {
    running: bool,
    events: EventHandler,
    /// As we wish it.
    paused: bool,
    /// May be streaming or paused, but also other things too
    audio_state: StreamStatus,
    /// No boost, only decrease.
    audio_vol: Volume,
    // TODO: undo/redo system shouldn't be that hard. later.
    beat_input: I,
    scope: scope::Scope,
    view: View,
    /// We can draw the help modal with (over) any view
    show_help: bool,
}

impl<I: BeatInput> App<I> {
    pub fn new(
        events: EventHandler,
        consumer: rtrb::Consumer<u8>,
        t_play: &'static AtomicI32,
        beat_input: I,
    ) -> Self {
        Self {
            running: true,
            events,
            paused: true,
            audio_state: StreamStatus::Unconnected,
            audio_vol: Volume::default(),
            beat_input,
            scope: scope::Scope::new(consumer, t_play),
            view: View::Main,
            show_help: false,
        }
    }

    pub fn run(mut self, mut term: DefaultTerminal) -> Result<()> {
        while self.running {
            term.draw(|frame| frame.render_widget(&mut self, frame.area()))?;
            self.update()?;
        }
        Ok(())
    }

    fn update(&mut self) -> Result<()> {
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
            Event::Tick => self.tick(),
            Event::FileWatch(event) => self.beat_input.handle_watch_event(event),
        }
        Ok(())
    }

    /// Fires on recieving messages from the event thread
    fn tick(&mut self) {
        // Update the scope with any new samples
        self.scope.update();
        // Just does visuals
        self.beat_input.tick();
    }

    fn handle_key_event(&mut self, event: KeyEvent) {
        // Global keys
        match event.code {
            KeyCode::F(1) => self.show_help = !self.show_help,
            KeyCode::F(2) => self.toggle_view(View::BigLog),
            KeyCode::F(3) => self.quit(),
            KeyCode::F(4) => self.toggle_playback(),
            KeyCode::F(5) => self.toggle_view(View::Library),
            KeyCode::Esc => {
                if self.show_help {
                    self.show_help = false;
                } else {
                    self.view = View::Main;
                }
            }
            KeyCode::Up => self.incr_volume(),
            KeyCode::Down => self.decr_volume(),

            // View-specific keys
            // FIXME: pulling the buffer doesn't play well with file reading, we need async 'request' with messages
            _ => match self.view {
                View::Main => match event.code {
                    KeyCode::Enter => self.try_beat(),
                    KeyCode::Backspace | KeyCode::Char(_) | KeyCode::Left | KeyCode::Right => {
                        self.beat_input.handle_key_event(event);
                    }
                    _ => {}
                },
                _ => {
                    // Swallow other keys in non-main views for now
                }
            },
        }
    }

    /// Go to the target view from main, Or back to main if we're there
    fn toggle_view(&mut self, target: View) {
        if self.view == target {
            self.view = View::Main;
        } else {
            self.view = target;
        }
    }

    /// Sync with actual stream state absolutely not guaranteed.
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

    fn incr_volume(&mut self) {
        let new = self.audio_vol.set(self.audio_vol.val() + 0.1);
        self.set_volume(new);
    }

    fn decr_volume(&mut self) {
        let new = self.audio_vol.set(self.audio_vol.val() - 0.1);
        self.set_volume(new);
    }

    /// Sync with stream state also not guaranteed.
    // Ditto: could wait to set our state until update
    fn set_volume(&mut self, vol: Volume) {
        self.events.set_volume(vol);
        self.audio_vol = vol;
    }

    /// Try-compile and play new are one operation from the user's perspective
    fn try_beat(&mut self) {
        match self.events.new_beat(&self.beat_input.get_buffer()) {
            Ok(_) => self.beat_input.clear_errors(),
            Err(errs) => self.beat_input.set_errors(errs),
        }
    }

    /// Causes break and clean exit on next [`App::run`] loop
    fn quit(&mut self) {
        trace!("app quit requested");
        self.running = false;
    }
}
