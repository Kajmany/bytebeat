use color_eyre::Result;

mod event;
mod parser;
mod pipewire;
mod tui;

fn main() -> Result<()> {
    color_eyre::install()?;

    // TODO: Cleanup doesn't matter, but what if we panic/res during the ratatui loop?
    tui::main()
}
