//! WASAPI backend for Windows - Vista and later. Very unsafe theoretically and practically because we're `?`-ing our way through Microslop's Win32 API.
//!
//! TODO: Handle errors better - invalidations are kind of expected already but not consistently handled.
use std::{
    sync::{
        Arc, LazyLock,
        atomic::{AtomicI32, Ordering},
        mpsc::{self, TryRecvError},
    },
    time::{Duration, Instant},
};

use arc_swap::ArcSwap;
use color_eyre::Result;
use tracing::{error, info, trace};
use windows::Win32::{
    Foundation::HANDLE,
    Media::Audio::{
        AUDCLNT_E_DEVICE_INVALIDATED, AUDCLNT_SESSIONFLAGS_EXPIREWHENUNOWNED,
        AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
        AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, IAudioClient,
        IAudioRenderClient, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator,
        WAVE_FORMAT_PCM, WAVEFORMATEX, eConsole, eRender,
    },
    System::{
        Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx},
        Threading::{CreateEventW, WaitForSingleObject},
    },
};

use windows::core::Error as WindowsError;

use super::{AudioCommand, AudioEvent, BITRATE, CHANNELS, STRIDE, StreamStatus};
use crate::{event::Event, parser};

/// Yeah, duh. But we'll const it.
const BITS_PER_SAMPLE: u16 = 8;

/// Timeout for WaitForSingleObject in milliseconds.
/// Short enough to respond to commands promptly.
const WAIT_TIMEOUT_MS: u32 = 10;

/// Tracks the current stream state and sends notifications when it changes.
struct StreamStateTracker {
    current: StreamStatus,
    event_tx: mpsc::Sender<Event>,
}

impl StreamStateTracker {
    fn new(event_tx: mpsc::Sender<Event>) -> Self {
        Self {
            current: StreamStatus::Unconnected,
            event_tx,
        }
    }

    fn set(&mut self, new_status: StreamStatus) {
        if self.current != new_status {
            trace!(
                "WASAPI stream state change: {:?} -> {:?}",
                self.current, new_status
            );
            let _ = self
                .event_tx
                .send(Event::Audio(AudioEvent::StateChange(new_status.clone())));
            self.current = new_status;
        }
    }

    fn is_active(&self) -> bool {
        self.current == StreamStatus::Streaming
    }
}

/// 'Kinda' Wraps the WASAPI IAudioClient and associated objects we'll use from it.
struct Device {
    pub audio: IAudioClient,
    pub render: IAudioRenderClient,
    pub volume: ISimpleAudioVolume,
}

impl Device {
    /// Get an immediately-usable set of WASAPI IAudio objects. We'll have to re-use this
    /// after init if the device is invalidated.
    ///
    /// # Errors
    ///
    /// Upon usual AUDCLNT_E_SERVICE_NOT_RUNNING and etc
    ///
    /// But also if resource or device are invalidated *during* this function.
    /// TODO: Not sure if this is *practically* possible.
    unsafe fn init(eventw: HANDLE) -> Result<Self> {
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

            let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;

            let audio_client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;

            // 8-bit Stereo PCM @ 8kHz, naturally
            let format = WAVEFORMATEX {
                wFormatTag: WAVE_FORMAT_PCM as u16, // why am I casting their const lol
                nChannels: CHANNELS as u16,
                nSamplesPerSec: BITRATE as u32,
                nAvgBytesPerSec: (BITRATE * STRIDE) as u32,
                nBlockAlign: STRIDE as u16,
                wBitsPerSample: BITS_PER_SAMPLE,
                cbSize: 0,
            };

            let buffer_duration = 1_000_000;
            audio_client.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_EVENTCALLBACK // Event driven processing
                | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM // Let WASAPI upsample
                | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY // Ask WASAPI to upsample well
                | AUDCLNT_SESSIONFLAGS_EXPIREWHENUNOWNED, // Prevents leak?? since we're too lazy to shutdown TODO
                buffer_duration,
                0,
                &format,
                None,
            )?;

            audio_client.SetEventHandle(eventw)?;
            let render_client: IAudioRenderClient = audio_client.GetService()?;
            let volume_client: ISimpleAudioVolume = audio_client.GetService()?;

            Ok(Self {
                audio: audio_client,
                render: render_client,
                volume: volume_client,
            })
        }
    }

    /// Estimates which sample is currently being played, accounting for buffered samples.
    fn estimate_play_head(&self, t_write: i32) -> i32 {
        unsafe {
            let padding = self.audio.GetCurrentPadding().unwrap_or(0);
            t_write - padding as i32
        }
    }

    /// I'm not actually 100% sure this is immutable after init so we'll just keep calling it
    /// return value is in frames
    fn bufsize(&self) -> u32 {
        unsafe { self.audio.GetBufferSize().unwrap_or(0) }
    }
}

pub fn main(
    event_tx: mpsc::Sender<Event>,
    command_rx: mpsc::Receiver<AudioCommand>,
    mut producer: rtrb::Producer<u8>,
    t_play: &'static AtomicI32,
) -> Result<()> {
    unsafe {
        info!("WASAPI thread starting");
        static BEAT: LazyLock<ArcSwap<parser::Beat>> =
            LazyLock::new(|| ArcSwap::new(Arc::new(parser::Beat::default())));
        static T_WRITE: AtomicI32 = AtomicI32::new(0);

        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

        // We can re-use this if we have to re-init
        let buffer_ready = CreateEventW(None, false, false, None)?;

        let mut state_tracker = StreamStateTracker::new(event_tx);
        let mut last_t_sync = Instant::now();

        loop {
            state_tracker.set(StreamStatus::Connecting);

            let device = match Device::init(buffer_ready) {
                Ok(res) => res,
                Err(e) => {
                    error!("Failed to initialize WASAPI: {}", e);
                    state_tracker.set(StreamStatus::Error);
                    std::thread::sleep(Duration::from_secs(1));
                    continue;
                }
            };

            // Start paused - matches pipewire behavior
            state_tracker.set(StreamStatus::Paused);

            loop {
                // Process all pending commands
                loop {
                    match command_rx.try_recv() {
                        Ok(cmd) => {
                            trace!("WASAPI thread received command: {:?}", cmd);
                            match cmd {
                                AudioCommand::Play => {
                                    if !state_tracker.is_active() {
                                        let _ = device.audio.Start();
                                        state_tracker.set(StreamStatus::Streaming);
                                    }
                                }
                                AudioCommand::Pause => {
                                    if state_tracker.is_active() {
                                        let _ = device.audio.Stop();
                                        state_tracker.set(StreamStatus::Paused);
                                    }
                                }
                                AudioCommand::NewBeat(beat) => {
                                    BEAT.store(Arc::new(beat));
                                }
                                AudioCommand::SetVolume(vol) => {
                                    // Just assume it is as we've set
                                    // TODO: We *could* make an event callback & send what it actually is
                                    // to the UI as an event
                                    let _ =
                                        device.volume.SetMasterVolume(vol.val(), std::ptr::null());
                                }
                            }
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            info!("WASAPI command channel disconnected, exiting");
                            return Ok(());
                        }
                    }
                }

                // Update T_PLAY periodically for the Scope widget
                if last_t_sync.elapsed() >= super::T_SYNC_INTERVAL {
                    let head = device.estimate_play_head(T_WRITE.load(Ordering::Relaxed));
                    t_play.store(head, Ordering::Relaxed);
                    last_t_sync = Instant::now();
                }

                // Wait for buffer event with timeout so we can process commands
                WaitForSingleObject(buffer_ready, WAIT_TIMEOUT_MS);

                if !state_tracker.is_active() {
                    continue;
                }

                let res = (|| -> Result<()> {
                    let padding = device.audio.GetCurrentPadding()?;
                    let frames_available = device.bufsize().saturating_sub(padding);

                    if frames_available == 0 {
                        return Ok(());
                    }

                    let buffer = device.render.GetBuffer(frames_available)?;
                    let samples = std::slice::from_raw_parts_mut(
                        buffer as *mut u8,
                        (frames_available * CHANNELS as u32) as usize,
                    );

                    for frame in 0..frames_available {
                        let sample = BEAT.load().eval(T_WRITE.fetch_add(1, Ordering::Relaxed));

                        // Write to both channels (stereo)
                        let idx = (frame * CHANNELS as u32) as usize;
                        samples[idx] = sample;
                        samples[idx + 1] = sample;

                        // Push to visualization buffer (best effort)
                        if !producer.is_full() {
                            let _ = producer.push(sample);
                        }
                    }

                    device.render.ReleaseBuffer(frames_available, 0)?;
                    Ok(())
                })();

                if let Err(e) = res {
                    let is_invalidated = e
                        .downcast_ref::<WindowsError>()
                        .map(|w| w.code().0 == AUDCLNT_E_DEVICE_INVALIDATED.0)
                        .unwrap_or(false);

                    if is_invalidated {
                        info!("WASAPI device invalidated, re-initializing");
                        let _ = device.audio.Stop();
                        state_tracker.set(StreamStatus::Connecting);
                        break;
                    } else {
                        state_tracker.set(StreamStatus::Error);
                        return Err(e);
                    }
                }
            }
        }
    }
}
