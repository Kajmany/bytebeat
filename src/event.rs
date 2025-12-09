use crate::audio::{AudioCommand, AudioEvent};
use crate::parser::{self};
use color_eyre::eyre::WrapErr;
use crossterm::event::{self, Event as CrosstermEvent};
use std::thread;
use std::{sync::mpsc, time::Duration};
use tracing::{info, trace};

#[derive(Clone, Debug)]
pub enum Event {
    Crossterm(CrosstermEvent),
    Audio(AudioEvent),
}

/// Terminal event handler.
pub struct EventHandler {
    term_sender: mpsc::Sender<Event>,
    term_receiver: mpsc::Receiver<Event>,

    audio_sender: pipewire::channel::Sender<AudioCommand>,
}

impl EventHandler {
    /// Constructs a new instance of [`EventHandler`] and spawns a new thread to handle events.
    pub fn new(audio_sender: pipewire::channel::Sender<AudioCommand>) -> Self {
        let (term_sender, term_receiver) = mpsc::channel();
        let actor = EventThread::new(term_sender.clone());
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

    /// Attempt to compile a new beat. Return an error, or send it to the audio thread if successful.
    // TODO: This can be made async if we give this duty to `EventThread` and send a message back to App.
    //     Investigate lag!
    pub fn new_beat(&self, beat: &str) -> color_eyre::Result<(), parser::ParseError> {
        trace!("event handler recieved beat: {}", beat);
        let beat = parser::Beat::compile(beat)?;
        trace!("compilation complete; event handler sending new beat command");
        let _ = self.audio_sender.send(AudioCommand::NewBeat(beat));
        Ok(())
    }
}

/// A thread that handles reading crossterm events and emitting tick events on a regular schedule.
// TODO: This is maybe useless unless we want ticks later
struct EventThread {
    /// Event term_sender channel.
    term_sender: mpsc::Sender<Event>,
}

impl EventThread {
    /// Constructs a new instance of [`EventThread`].
    fn new(term_sender: mpsc::Sender<Event>) -> Self {
        Self { term_sender }
    }

    /// Runs the event thread.
    ///
    /// This function polls for crossterm events.
    fn run(self) -> color_eyre::Result<()> {
        info!("event thread loop starting");
        loop {
            // poll for crossterm events
            if event::poll(Duration::from_millis(100))
                .wrap_err("failed to poll for crossterm events")?
            {
                let event = event::read().wrap_err("failed to read crossterm event")?;
                trace!("event thread recieved crossterm event: {:?}", event);
                self.send(Event::Crossterm(event));
            }
        }
    }

    fn send(&self, event: Event) {
        // Ignores the result because shutting down the app drops the term_receiver, which causes the send
        // operation to fail. This is expected behavior and should not panic.
        let _ = self.term_sender.send(event);
    }
}
