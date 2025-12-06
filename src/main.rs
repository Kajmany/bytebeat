use color_eyre::Result;
use std::thread;

use crate::{audio::AudioCommand, event::EventHandler};

mod audio;
mod event;
mod parser;
mod tui;

fn main() -> Result<()> {
    color_eyre::install()?;

    // Somewhat ugly piping between threads done here
    // So commands to change stream can flow events -> audio
    let (command_tx, command_rx) = pipewire::channel::channel::<AudioCommand>();
    let events = EventHandler::new(command_tx);
    // TODO: maybe hoist channel creation for term here also
    let terminal_tx = events.get_term_sender();
    // Pipewire loop needs to tx states to App and rx commands from it (brokered by event handler)
    thread::spawn(move || crate::audio::main(terminal_tx, command_rx));
    // App owns the event handler struct (but NOT the thread!)
    tui::main(events)
}
