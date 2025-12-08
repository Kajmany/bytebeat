use color_eyre::Result;
use std::thread;

use crate::{app::App, audio::AudioCommand, event::EventHandler};

mod app;
mod audio;
mod event;
mod parser;

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
    // App owns the event handler struct (but NOT the event thread!)
    let terminal = ratatui::init();
    let result = App::new(events).run(terminal);
    ratatui::restore();
    result
}
