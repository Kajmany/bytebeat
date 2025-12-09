use color_eyre::Result;
use std::thread;
use tracing::info;
use tracing_subscriber::{EnvFilter, prelude::*};

use crate::{app::App, audio::AudioCommand, event::EventHandler};

mod app;
mod audio;
mod event;
mod parser;

use clap::Parser;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Log to file. May provide filename, or default to "bytebeat.log"
    #[arg(short = 'f', long = "log-file", num_args = 0..=1, default_missing_value = "bytebeat.log")]
    log_file: Option<std::path::PathBuf>,
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();

    let (file_layer, _guard) = if let Some(path) = cli.log_file {
        let file = std::fs::File::options()
            .append(true)
            .create(true)
            .open(path)?;
        let (non_blocking, _guard) = tracing_appender::non_blocking(file);
        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false)
            .with_thread_names(true);
        (Some(file_layer), Some(_guard))
    } else {
        (None, None)
    };

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=trace", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(file_layer)
        .init();

    info!("app starting");
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
    info!("app done: {:?}", result);
    result
}
