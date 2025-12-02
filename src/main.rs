use color_eyre::Result;
use std::thread;

mod event;
mod pipewire;
mod tui;

fn main() -> Result<()> {
    color_eyre::install()?;

    // TODO: Cleanup doesn't matter, but what if we panic/res during the ratatui loop?
    thread::spawn(pipewire::main);
    tui::main()
}
