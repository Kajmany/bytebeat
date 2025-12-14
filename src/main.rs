use color_eyre::Result;
use std::{sync::atomic::AtomicI32, thread};
use tracing::info;
use tracing_subscriber::{EnvFilter, prelude::*};
use tui_logger::{LevelFilter, TuiLoggerFile};

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
    /// Log at 'trace'. 'info' otherwise. `RUST_LOG` env takes precedence.
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();

    let (level_str, level_enum) = if cli.verbose {
        ("trace", LevelFilter::Trace)
    } else {
        ("info", LevelFilter::Info)
    };
    // Environment variable takes precedence over -v flag usage
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| format!("{0}={1}", env!("CARGO_CRATE_NAME"), level_str).into());

    tracing_subscriber::registry()
        .with(filter)
        .with(tui_logger::TuiTracingSubscriberLayer)
        .init();

    tui_logger::init_logger(level_enum)?;
    if let Some(path) = cli.log_file {
        tui_logger::set_log_file(TuiLoggerFile::new(
            path.to_str()
                // This shouldn't happen.
                .expect("provided path valid but not convertible to &str"),
        ));
    }

    info!("app starting");
    // Somewhat ugly piping between threads done here

    // So commands to change stream can flow events -> audio
    let (command_tx, command_rx) = pipewire::channel::channel::<AudioCommand>();

    // For audio visualization widget. Audio thread produces, App consumes
    // 64000 samples @ 8kHz = 8 seconds of buffer (and 62.5KiB)
    // We'll probably only want to display 4 at once, maximum
    let (producer, consumer) = rtrb::RingBuffer::<u8>::new(64000);
    // Represents that the 't'th-ISH sample will play next
    // Audio thread gets a to set it, App gets a copy to read it
    // The scope widget needs this to look useful and track where-about we're at.
    static T_PLAY: AtomicI32 = AtomicI32::new(0);

    let events = EventHandler::new(command_tx);
    // TODO: maybe hoist channel creation for term here also
    let terminal_tx = events.get_term_sender();
    // Pipewire loop needs to tx states to App and rx commands from it (brokered by event handler)
    thread::spawn(move || crate::audio::main(terminal_tx, command_rx, producer, &T_PLAY));
    // App owns the event handler struct (but NOT the event thread!)
    let terminal = ratatui::init();
    let result = App::new(events, consumer, &T_PLAY).run(terminal);
    ratatui::restore();
    info!("app done: {:?}", result);
    result
}
