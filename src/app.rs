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

    fn update(&mut self) -> Result<()> {
        let response_event: Option<AppEvent> = match self.events.next()? {
            Event::App(event) => match event {
                AppEvent::InputReady(code) => {
                    self.try_beat(&code);
                    None
                }
                AppEvent::BeatOverwrite(code) => {
                    let _ = self.beat_input.set_buffer(code.clone());
                    let _ = self.events.new_beat(&code).map_err(|e| {
                        error!(
                            "library sent a hardcoded beat that had an error (embarrassing): {e:?}"
                        )
                    });
                    None
                }
                AppEvent::VolumeUp => {
                    self.incr_volume();
                    None
                }
                AppEvent::VolumeDown => {
                    self.decr_volume();
                    None
                }
                AppEvent::Quit => {
                    self.quit();
                    None
                }
                AppEvent::TogglePlay => {
                    self.toggle_playback();
                    None
                }
                AppEvent::ChangeView(view) => {
                    self.view = view;
                    None
                }
                AppEvent::ToggleHelp => {
                    self.show_help = !self.show_help;
                    None
                }
                AppEvent::ViewBack => {
                    if self.show_help {
                        self.show_help = false;
                    } else {
                        self.view = View::Main;
                    }
                    None
                }
            },
            Event::Crossterm(event) => match event {
                crossterm::event::Event::Key(event) if event.kind == KeyEventKind::Press => {
                    trace!("app handling crossterm event: {:?}", event);
                    self.handle_key_event(event)
                }
                _ => None,
            },
            Event::Audio(AudioEvent::StateChange(event)) => {
                info!("app recieved audio state change: {:?}", event);
                self.audio_state = event;
                None
            }
            Event::Tick => {
                // Must run even when not shown to keep emptying rtrb
                self.scope.update();
                // For visuals in component
                self.beat_input.handle_event(&Event::Tick)
            }
            Event::FileWatch(event) => self.beat_input.handle_event(&Event::FileWatch(event)),
        };

        if let Some(event) = response_event {
            self.events.enqueue_app_event(event);
        }
        Ok(())
    }

    fn handle_key_event(&mut self, event: KeyEvent) -> Option<AppEvent> {
        // Global keys
        match event.code {
            KeyCode::F(1) => Some(AppEvent::ToggleHelp),
            KeyCode::F(2) => Some(AppEvent::ChangeView(View::BigLog)),
            KeyCode::F(3) => Some(AppEvent::Quit),
            KeyCode::F(4) => Some(AppEvent::TogglePlay),
            KeyCode::F(5) => Some(AppEvent::ChangeView(View::Library)),
            KeyCode::Esc => Some(AppEvent::ViewBack),
            KeyCode::Up => Some(AppEvent::VolumeUp),
            KeyCode::Down => Some(AppEvent::VolumeDown),

            // Pass-through to components depending on view
            _ => match self.view {
                View::Main => self
                    .beat_input
                    // Oh wow this is ugly
                    .handle_event(&Event::Crossterm(crossterm::event::Event::Key(event))),
                View::Library => self
                    .library
                    .handle_event(&Event::Crossterm(crossterm::event::Event::Key(event))),
                View::BigLog => None,
            },
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
