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

mod input;
mod scope;
mod ui;
mod volume;

pub struct App {
    running: bool,
    events: EventHandler,
    // As we wish it.
    paused: bool,
    // May be streaming or paused, but also other things too
    audio_state: StreamStatus,
    /// No boost, only decrease.
    audio_vol: Volume,
    // TODO: undo/redo system shouldn't be that hard. later.
    beat_input: BeatInput,

    scope: scope::Scope,
}

impl App {
    pub fn new(
        events: EventHandler,
        consumer: rtrb::Consumer<u8>,
        t_play: &'static AtomicI32,
    ) -> Self {
        Self {
            running: true,
            events,
            paused: true,
            audio_state: StreamStatus::Unconnected,
            audio_vol: Volume::default(),
            // TODO: Not a pretty way to do defaults
            beat_input: BeatInput::from_str("t*(42&t>>10)"),

            scope: scope::Scope::new(consumer, t_play),
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
        }
        Ok(())
    }

    /// Fires on recieving messages from the event thread
    fn tick(&mut self) {
        // Update the scope with any new samples
        self.scope.update();
    }

    fn handle_key_event(&mut self, event: KeyEvent) {
        match event.code {
            // We can always exit
            KeyCode::F(3) => self.quit(),
            KeyCode::F(4) => self.toggle_playback(),

            // For now, we assume we're always focused on the input field
            KeyCode::Backspace | KeyCode::Char(_) | KeyCode::Left | KeyCode::Right => {
                self.beat_input.handle_key_event(event);
            }

            // For beat input, delegate everything besides this so App has full control of events
            KeyCode::Enter => self.try_beat(),

            KeyCode::Up => {
                self.incr_volume();
            }
            KeyCode::Down => {
                self.decr_volume();
            }
            // This ain't emacs, pal.
            _ => {}
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
