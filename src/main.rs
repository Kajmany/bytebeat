use color_eyre::Result;
use notify::Watcher;
use std::{
    sync::{atomic::AtomicI32, mpsc},
    thread,
};
use tracing::info;
use tracing_subscriber::{EnvFilter, prelude::*};
use tui_logger::{LevelFilter, TuiLoggerFile};

use crate::{
    app::{
        App,
        input::{FileWatchInput, InteractiveInput},
    },
    event::EventHandler,
};

mod app;
mod audio;
mod event;
// Generated from CSV
mod library_data;
mod parser;

use clap::{Parser, builder::ArgPredicate};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Log to file. May provide filename, or default to "bytebeat.log"
    #[arg(short = 'l', long = "log-file", num_args = 0..=1, default_missing_value = "bytebeat.log")]
    log_file: Option<std::path::PathBuf>,
    /// Log at 'trace'. 'info' otherwise. `RUST_LOG` env takes precedence
    #[arg(short, long, default_value = "false")]
    verbose: bool,
    /// Watch this file for beat input. stdin is still used for controls
    #[arg(short = 'w', long = "watch-file", conflicts_with = "interactive", value_parser = readable_file)]
    watch_file: Option<std::path::PathBuf>,
    /// Interactive stdin Beat input with a simple line-editor. (Default)
    #[arg(
        short = 'i',
        long = "interactive",
        conflicts_with = "watch_file",
        default_value = "true",
        default_value_if("watch_file", ArgPredicate::IsPresent, "false")
    )]
    interactive: bool,
}

// TODO: This function has become a dumping ground, some of it should probably go in App. some should ???
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
    let (command_tx, command_rx) = audio::command_channel();

    // For audio visualization widget. Audio thread produces, App consumes
    // 64000 samples @ 8kHz = 8 seconds of buffer (and 62.5KiB)
    // We'll probably only want to display 4 at once, maximum
    let (producer, consumer) = rtrb::RingBuffer::<u8>::new(64000);
    // Represents that the 't'th-ISH sample will play next
    // Audio thread gets a to set it, App gets a copy to read it
    // The scope widget needs this to look useful and track where-about we're at.
    static T_PLAY: AtomicI32 = AtomicI32::new(0);

    // Set up file watching input if requested. The watcher must be kept alive.
    let (_watcher, file_watch_rx) = match cli.watch_file {
        Some(ref path) => {
            let (watcher, rx) = setup_watch(path)?;
            (Some(watcher), Some(rx))
        }
        None => (None, None),
    };

    let events = EventHandler::new(command_tx, file_watch_rx);
    // TODO: maybe hoist channel creation for term here also
    let terminal_tx = events.get_term_sender();
    // Pipewire loop needs to tx states to App and rx commands from it (brokered by event handler)
    thread::spawn(move || crate::audio::main(terminal_tx, command_rx, producer, &T_PLAY));
    // App owns the event handler struct (but NOT the event thread!)
    let terminal = ratatui::init();
    // We need to split here because App is generic over these possible input widgets TODO: Do this inside App?
    let result = if cli.interactive {
        App::new(events, consumer, &T_PLAY, InteractiveInput::default()).run(terminal)
    } else {
        App::new(events, consumer, &T_PLAY, FileWatchInput::default()).run(terminal)
    };
    ratatui::restore();
    info!("app done: {:?}", result);
    result
}

fn setup_watch(
    path: &std::path::Path,
) -> Result<
    (
        impl notify::Watcher,
        mpsc::Receiver<Result<notify::Event, notify::Error>>,
    ),
    notify::Error,
> {
    let (tx, rx) = mpsc::channel::<Result<notify::Event, notify::Error>>();
    let mut watcher = notify::recommended_watcher(tx)?;
    watcher.watch(path, notify::RecursiveMode::NonRecursive)?;
    Ok((watcher, rx))
}

/// Validates that a path exists, is not a directory, and is readable.
///
/// Used solely to check a path given for file watch functionality.
fn readable_file(s: &str) -> Result<std::path::PathBuf, String> {
    let path = std::path::PathBuf::from(s);

    if !path.exists() {
        return Err(format!("path does not exist: {}", path.display()));
    }
    if path.is_dir() {
        return Err(format!(
            "path is a directory, but must be a file: {}",
            path.display()
        ));
    }
    // Check readability by attempting to open
    std::fs::File::open(&path)
        .map_err(|e| format!("unable to read file {}: {}", path.display(), e))?;

    Ok(path)
}
