use std::sync::atomic::AtomicI32;

use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::DefaultTerminal;
use tracing::{error, info, trace};

use crate::{
    app::input::BeatInput,
    audio::{AudioEvent, StreamStatus, Volume},
    event::{Event, EventHandler},
};

pub mod input; // TODO: Not pretty, has to be pub so we can make it in main :(
mod scope;
mod ui;

/// Every widget owned by [`App`] implements this to handle delegated events
///
/// They should also implement a 'renderable' ratatui trait, but I won't use supertrait here
/// because there's several possibilities.
pub trait Component {
    fn handle_event(&mut self, event: Event) -> Option<AppEvent> {
        match event {
            // These two probably aren't for us, but a component could care.
            Event::App(_) => None,
            Event::Audio(_) => None,
            // Usually only care about Keydown
            Event::Crossterm(crossterm::event::Event::Key(event))
                if event.kind == KeyEventKind::Press =>
            {
                self.handle_key_event(event)
            }
            Event::Crossterm(_) => None,
            Event::Tick => self.handle_tick(),
            Event::FileWatch(event) => self.handle_filewatch(event),
        }
    }

    #[allow(unused)]
    fn handle_key_event(&mut self, event: KeyEvent) -> Option<AppEvent> {
        None
    }

    #[allow(unused)]
    fn handle_tick(&mut self) -> Option<AppEvent> {
        None
    }

    #[allow(unused)]
    fn handle_filewatch(&mut self, event: notify::Event) -> Option<AppEvent> {
        None
    }
}

#[derive(Debug, Clone)]
/// Returned from component-specific update methods or methods of [`App`]
/// only these events mutate state directly.
pub enum AppEvent {
    /// Input OR Library wants you to play this sick beat
    InputReady(String),
    /// Library wants you to play this AND over-write the Input
    BeatOverwrite(String),
    // All these were formerly immediate & hardcoded in handle_key_event
    VolumeUp,
    VolumeDown,
    Quit,
    TogglePlay,
    /// Changes to this specific view
    ChangeView(View),
    /// Esc action, will close help or return to main view
    ViewBack,
    ToggleHelp,
}

/// Used to decide where to route events and what to render
///
/// Help modal is elsewhere because it's not mutually exclusive
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

    /// Blocks until the next event. Upper update bound is time to tick
    fn update(&mut self) -> Result<()> {
        // Some events (currently just tick) delegate to multiple components
        let mut responses: Vec<Option<AppEvent>> = Vec::new();
        match self.events.next()? {
            Event::App(event) => {
                trace!("app recieved app event: {:?}", event);
                match event {
                    AppEvent::InputReady(code) => {
                        self.try_beat(&code);
                    }
                    AppEvent::BeatOverwrite(code) => {
                        let _ = self.beat_input.set_buffer(code.clone());
                        let _ = self.events.new_beat(&code).map_err(|e| {
                        error!(
                            "library sent a hardcoded beat that had an error (embarrassing): {e:?}"
                        )
                    });
                    }
                    AppEvent::VolumeUp => {
                        self.incr_volume();
                    }
                    AppEvent::VolumeDown => {
                        self.decr_volume();
                    }
                    AppEvent::Quit => {
                        self.quit();
                    }
                    AppEvent::TogglePlay => {
                        self.toggle_playback();
                    }
                    AppEvent::ChangeView(view) => {
                        self.view = view;
                    }
                    AppEvent::ToggleHelp => {
                        self.show_help = !self.show_help;
                    }
                    AppEvent::ViewBack => {
                        if self.show_help {
                            self.show_help = false;
                        } else {
                            self.view = View::Main;
                        }
                    }
                };
            }
            Event::Crossterm(event) => {
                trace!("app handling crossterm event: {:?}", event);
                responses.push(self.handle_crossterm_event(event));
            }
            Event::Audio(AudioEvent::StateChange(event)) => {
                info!("app recieved audio state change: {:?}", event);
                self.audio_state = event;
            }
            Event::Tick => {
                // Must run even when not shown to keep emptying rtrb
                responses.push(self.scope.handle_event(Event::Tick));
                // For visuals in component
                responses.push(self.beat_input.handle_event(Event::Tick));
            }
            Event::FileWatch(event) => {
                // One actual action is often many of these
                trace!("app recieved file watch event: {:?}", event);
                responses.push(self.beat_input.handle_event(Event::FileWatch(event)))
            }
        };

        responses
            .into_iter()
            .flatten()
            .for_each(|e| self.events.enqueue_app_event(e));
        Ok(())
    }

    fn handle_crossterm_event(&mut self, event: crossterm::event::Event) -> Option<AppEvent> {
        // Handle global keys now
        if let crossterm::event::Event::Key(key) = event {
            if key.kind == KeyEventKind::Press {
                if let Some(resp) = match key.code {
                    KeyCode::F(1) => Some(AppEvent::ToggleHelp),
                    KeyCode::F(2) => Some(AppEvent::ChangeView(View::BigLog)),
                    KeyCode::F(3) => Some(AppEvent::Quit),
                    KeyCode::F(4) => Some(AppEvent::TogglePlay),
                    KeyCode::F(5) => Some(AppEvent::ChangeView(View::Library)),
                    KeyCode::Esc => Some(AppEvent::ViewBack),
                    KeyCode::Up => Some(AppEvent::VolumeUp),
                    KeyCode::Down => Some(AppEvent::VolumeDown),
                    _ => None,
                } {
                    return Some(resp);
                }
            }
        }

        // Or delegate to relevant component
        match self.view {
            View::Main => self.beat_input.handle_event(Event::Crossterm(event)),
            View::Library => self.library.handle_event(Event::Crossterm(event)),
            View::BigLog => None,
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
    fn try_beat(&mut self, code: &str) {
        match self.events.new_beat(code) {
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
