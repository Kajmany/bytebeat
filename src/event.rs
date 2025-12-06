use crate::audio::{AudioCommand, AudioEvent};
use color_eyre::eyre::WrapErr;
use crossterm::event::{self, Event as CrosstermEvent};
use std::thread;
use std::{sync::mpsc, time::Duration};

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

    // Queue an app event to be sent to the event term_receiver.
    //
    // This is useful for sending events to the event handler which will be processed by the next
    // iteration of the application's event loop.
    //pub fn send(&mut self, app_event: Event) {
    //    // Ignore the result as the reciever cannot be dropped while this struct still has a
    //    // reference to it
    //    let _ = self.term_sender.send(Event::App(app_event));
    //}

    /// Get a term_sender handler, intended for other threads that wish to send events to the
    /// [`crate::tui::App`]
    // TODO: Should we encapsulate this at a higher level?
    pub fn get_term_sender(&self) -> mpsc::Sender<Event> {
        self.term_sender.clone()
    }

    pub fn toggle_playback(&self) {
        let _ = self.audio_sender.send(AudioCommand::StreamToggle);
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
        loop {
            // poll for crossterm events
            if event::poll(Duration::from_millis(100))
                .wrap_err("failed to poll for crossterm events")?
            {
                let event = event::read().wrap_err("failed to read crossterm event")?;
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
