#[cfg(target_os = "windows")]
mod wasapi;

#[cfg(target_os = "linux")]
mod pipewire;

use crate::parser;

pub const CHANNELS: usize = 2;
pub const STRIDE: usize = size_of::<u8>() * CHANNELS;

#[derive(Debug, Clone, Copy, PartialEq)]
/// Wrapped float that can represent no volume [`Volume::MUTE`] or
/// normal (not amplified) volume [`Volume::MAX`].
/// Same range as [`libspa_sys::SPA_PROP_volume`]
pub struct Volume(f32);

impl Default for Volume {
    fn default() -> Self {
        Self::new(0.5)
    }
}

impl Volume {
    pub const MUTE: Self = Self(0.0);
    pub const MAX: Self = Self(1.0);

    pub fn new(value: f32) -> Self {
        Self(value.clamp(Self::MUTE.val(), Self::MAX.val()))
    }

    pub fn set(&self, val: f32) -> Self {
        Self(val.clamp(Self::MUTE.val(), Self::MAX.val()))
    }

    pub fn val(&self) -> f32 {
        self.0
    }
}

impl std::fmt::Display for Volume {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.0}%", self.0 * 100.0)
    }
}

#[derive(Clone, Debug)]
pub enum AudioEvent {
    StateChange(StreamStatus),
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Remapping of [`::pipewire::stream::StreamState`] that can be cloned.
pub enum StreamStatus {
    /// the stream is in error
    Error,
    /// unconnected
    Unconnected,
    /// connection is in progress
    Connecting,
    /// paused
    Paused,
    /// streaming
    Streaming,
}

#[derive(Debug)]
pub enum AudioCommand {
    Play,
    Pause,
    SetVolume(Volume),
    NewBeat(parser::Beat),
}

// Re-export the platform-specific main function
#[cfg(target_os = "linux")]
pub use pipewire::main;

#[cfg(target_os = "windows")]
pub use wasapi::main;

// Platform-agnostic channel types for sending commands to the audio thread.
// On Linux, we use pipewire::channel which integrates with the pipewire main loop.
// On Windows (and other platforms), we use std::sync::mpsc.

#[cfg(target_os = "linux")]
pub type CommandSender = ::pipewire::channel::Sender<AudioCommand>;
#[cfg(target_os = "linux")]
pub type CommandReceiver = ::pipewire::channel::Receiver<AudioCommand>;

#[cfg(not(target_os = "linux"))]
pub type CommandSender = std::sync::mpsc::Sender<AudioCommand>;
#[cfg(not(target_os = "linux"))]
pub type CommandReceiver = std::sync::mpsc::Receiver<AudioCommand>;

/// Create a new command channel for sending commands to the audio thread.
#[cfg(target_os = "linux")]
pub fn command_channel() -> (CommandSender, CommandReceiver) {
    ::pipewire::channel::channel::<AudioCommand>()
}

#[cfg(not(target_os = "linux"))]
pub fn command_channel() -> (CommandSender, CommandReceiver) {
    std::sync::mpsc::channel::<AudioCommand>()
}
