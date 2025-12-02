use color_eyre::eyre::WrapErr;
use crossterm::event::{self, Event as CrosstermEvent};
use std::thread;
use std::{sync::mpsc, time::Duration};

#[derive(Clone, Debug)]
pub enum Event {
    Crossterm(CrosstermEvent),
    App(AppEvent),
}

#[derive(Clone, Debug)]
pub enum AppEvent {
    Foo,
}

/// Terminal event handler.
#[derive(Debug)]
pub struct EventHandler {
    sender: mpsc::Sender<Event>,
    receiver: mpsc::Receiver<Event>,
}

impl Default for EventHandler {
    fn default() -> Self {
        EventHandler::new()
    }
}

impl EventHandler {
    /// Constructs a new instance of [`EventHandler`] and spawns a new thread to handle events.
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        let actor = EventThread::new(sender.clone());
        thread::spawn(|| actor.run());
        Self { sender, receiver }
    }

    /// Receives an event from the sender.
    ///
    /// This function blocks until an event is received.
    ///
    /// # Errors
    ///
    /// This function returns an error if the sender channel is disconnected. This can happen if an
    /// error occurs in the event thread. In practice, this should not happen unless there is a
    /// problem with the underlying terminal.
    pub fn next(&self) -> color_eyre::Result<Event> {
        Ok(self.receiver.recv()?)
    }

    /// Queue an app event to be sent to the event receiver.
    ///
    /// This is useful for sending events to the event handler which will be processed by the next
    /// iteration of the application's event loop.
    pub fn send(&mut self, app_event: AppEvent) {
        // Ignore the result as the reciever cannot be dropped while this struct still has a
        // reference to it
        let _ = self.sender.send(Event::App(app_event));
    }
}

/// A thread that handles reading crossterm events and emitting tick events on a regular schedule.
struct EventThread {
    /// Event sender channel.
    sender: mpsc::Sender<Event>,
}

impl EventThread {
    /// Constructs a new instance of [`EventThread`].
    fn new(sender: mpsc::Sender<Event>) -> Self {
        Self { sender }
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
        // Ignores the result because shutting down the app drops the receiver, which causes the send
        // operation to fail. This is expected behavior and should not panic.
        let _ = self.sender.send(event);
    }
}
