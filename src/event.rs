use crate::audio::{self, AudioCommand, AudioEvent, Volume};
use crate::parser::{self};
use color_eyre::eyre::WrapErr;
use crossterm::event::{self, Event as CrosstermEvent};
use std::thread;
use std::time::Instant;
use std::{sync::mpsc, time::Duration};
use tracing::{error, info, trace};

/// The frequency at which tick events are emitted.
pub const TICK_FPS: f64 = 30.0;

#[derive(Clone, Debug)]
pub enum Event {
    /// Wraps [`CrosstermEvent`] sent from the terminal. Includes resizes, keystrokes, etc
    Crossterm(CrosstermEvent),
    /// Wraps [`AudioEvent`] sent from the audio thread.
    Audio(AudioEvent),
    /// Regularly scheduled from the [`EventThread`].
    Tick,
    /// Forwarded from the [`EventThread`]. Occurs only if user asked for file watch input at startup.
    FileWatch(notify::Event),
}

/// Terminal event handler. This is called commonly by the [`crate::app::App`] and 'lives' in the TUI thread.
pub struct EventHandler {
    term_sender: mpsc::Sender<Event>,
    term_receiver: mpsc::Receiver<Event>,

    audio_sender: audio::CommandSender,
    // File watch rx goes straight to the new thread. It'll forward those events back.
}

impl EventHandler {
    /// Constructs a new instance of [`EventHandler`] and spawns a new thread to handle events.
    pub fn new(
        audio_sender: audio::CommandSender,
        file_watch_receiver: Option<mpsc::Receiver<Result<notify::Event, notify::Error>>>,
    ) -> Self {
        let (term_sender, term_receiver) = mpsc::channel();
        let actor = EventThread::new(term_sender.clone(), file_watch_receiver);
        thread::spawn(|| actor.run());
        Self {
            term_sender,
            term_receiver,
            audio_sender,
        }
    }

    /// Receives an event from the term_sender.
    ///
    /// This function blocks until an event is received.
    ///
    /// # Errors
    ///
    /// This function returns an error if the term_sender channel is disconnected. This can happen if an
    /// error occurs in the event thread. In practice, this should not happen unless there is a
    /// problem with the underlying terminal.
    pub fn next(&self) -> color_eyre::Result<Event> {
        Ok(self.term_receiver.recv()?)
    }

    /// Get a term_sender handler, intended for other threads that wish to send events to the
    /// [`crate::app::App`]
    pub fn get_term_sender(&self) -> mpsc::Sender<Event> {
        self.term_sender.clone()
    }

    /// Enqueue play command for the audio thread to recieve. Should be fine if it's redundant.
    pub fn stream_play(&self) {
        trace!("event handler sending play command");
        let _ = self.audio_sender.send(AudioCommand::Play);
    }

    /// Enqueue pause command for the audio thread to recieve. Should be fine if it's redundant.
    pub fn stream_pause(&self) {
        trace!("event handler sending pause command");
        let _ = self.audio_sender.send(AudioCommand::Pause);
    }

    /// Enqueue new audio level command for the audio thread to recieve.
    pub fn set_volume(&self, vol: Volume) {
        trace!("event handler sending volume command");
        let _ = self.audio_sender.send(AudioCommand::SetVolume(vol));
    }

    /// Attempt to compile a new beat. Return an error, or send it to the audio thread if successful.
    // TODO: This can be made async if we give this duty to `EventThread` and send a message back to App.
    //     Investigate lag!
    pub fn new_beat(&self, beat: &str) -> color_eyre::Result<(), Vec<parser::ParseError>> {
        trace!("event handler recieved beat: {}", beat);
        let beat = parser::Beat::compile(beat)?;
        trace!("compilation complete; event handler sending new beat command");
        let _ = self.audio_sender.send(AudioCommand::NewBeat(beat));
        Ok(())
    }
}

/// A thread that forwards crossterm and file watch events to the main thread. Also emits ticks.
struct EventThread {
    /// Event term_sender channel.
    term_sender: mpsc::Sender<Event>,
    /// RX From a [`notify::Watcher`] IFF the user requested file watch beat input during startup.
    file_watch_receiver: Option<mpsc::Receiver<Result<notify::Event, notify::Error>>>,
}

impl EventThread {
    /// Constructs a new instance of [`EventThread`].
    fn new(
        term_sender: mpsc::Sender<Event>,
        file_watch_receiver: Option<mpsc::Receiver<Result<notify::Event, notify::Error>>>,
    ) -> Self {
        Self {
            term_sender,
            file_watch_receiver,
        }
    }

    /// Runs the event thread.
    ///
    /// This function polls for crossterm events.
    fn run(self) -> color_eyre::Result<()> {
        info!("event thread loop starting");
        let tick_interval = Duration::from_secs_f64(1.0 / TICK_FPS);
        let mut last_tick = Instant::now();
        loop {
            // emit tick events at a fixed rate
            let timeout = tick_interval.saturating_sub(last_tick.elapsed());
            if timeout == Duration::ZERO {
                last_tick = Instant::now();
                self.send(Event::Tick);
            }
            // poll for crossterm events
            if event::poll(timeout).wrap_err("failed to poll for crossterm events")? {
                let event = event::read().wrap_err("failed to read crossterm event")?;
                trace!("event thread recieved crossterm event: {:?}", event);
                self.send(Event::Crossterm(event));
            }
            // we'll have a file_watch only if the user wanted file-watching beat input
            if let Some(file_watch_rx) = &self.file_watch_receiver {
                match file_watch_rx.try_recv() {
                    Ok(Ok(event)) => {
                        trace!("event thread received file watch event: {:?}", event);
                        self.send(Event::FileWatch(event));
                    }
                    Ok(Err(e)) => error!("file watch error: {:?}", e),
                    Err(mpsc::TryRecvError::Empty) => {} // Cool
                    Err(mpsc::TryRecvError::Disconnected) => {
                        // Not cool TODO: Under what circumstances could this happen?
                        panic!("file watch channel disconnected")
                    }
                }
            }
        }
    }

    fn send(&self, event: Event) {
        // Ignores the result because shutting down the app drops the term_receiver, which causes the send
        // operation to fail. This is expected behavior and should not panic.
        let _ = self.term_sender.send(event);
    }
}
