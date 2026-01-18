//! Pipewire backend for Linux. Uses the stream API. Mostly uses safe bindings.
//!
//! TODO: May need to handle device {un,re}plugging like in WASAPI?
use std::{
    mem,
    sync::{
        Arc, LazyLock,
        atomic::{AtomicI32, Ordering},
        mpsc,
    },
};

use arc_swap::ArcSwap;
use derive_new::new;
use pipewire::{
    self as pw,
    context::ContextRc,
    main_loop::MainLoopRc,
    spa::{self, utils::Direction},
    stream::{Stream, StreamFlags, StreamRc, StreamState},
};
use pw::properties::properties;
use tracing::{error, info, trace, warn};

use super::{AudioCommand, AudioEvent, BITRATE, CHANNELS, STRIDE, StreamStatus, Volume};
use crate::{event::Event, parser};

// None of these structs are necessary. They're hopefully optimized out
// They're used to make it clearer what state each callback relies upon

/// Used in the [`pipewire::stream::ListenerLocalBuilder::state_changed`] callback
#[derive(new)]
struct StateChangeState {
    // i'm so semantically satiated right now
    /// Used to communicate with the [`crate::event::EventHandler`]
    event_tx: mpsc::Sender<Event>,
}

/// Used in the mpsc reading callback (which takes commands)
#[derive(new)]
struct CommandState {
    stream: StreamRc,
    beat: &'static ArcSwap<parser::Beat>,
}

/// Used in the attached timer which updates the 'play head'
/// for the benefit of the TUI
#[derive(new)]
struct TimerState {
    t_write: &'static AtomicI32,
    stream: StreamRc,
    t_play: &'static AtomicI32,
}

/// Passed solely to the [`on_process`] callback
#[derive(new)]
struct ProcessState {
    /// Used internally to decide what sample to calculate next
    t_write: &'static AtomicI32,
    beat: &'static ArcSwap<parser::Beat>,
    /// (Ideally) loaded with contiguous sample frames. Scope widget uses this to visualize
    producer: rtrb::Producer<u8>,
}

pub fn main(
    event_tx: mpsc::Sender<Event>,
    command_rx: pipewire::channel::Receiver<AudioCommand>,
    producer: rtrb::Producer<u8>,
    t_play: &'static AtomicI32,
) -> Result<(), pw::Error> {
    info!("pipewire thread starting");
    pw::init();
    let main_loop: &'static mut MainLoopRc = Box::leak(Box::new(MainLoopRc::new(None)?));
    let context: &'static mut ContextRc =
        Box::leak(Box::new(pw::context::ContextRc::new(main_loop, None)?));
    let core = context.connect_rc(None)?;

    let stream = pw::stream::StreamRc::new(
        core,
        "audio-src",
        properties! {
            *pw::keys::MEDIA_TYPE => "Audio",
            *pw::keys::MEDIA_ROLE => "Music",
            *pw::keys::MEDIA_CATEGORY => "Playback",
            *pw::keys::AUDIO_CHANNELS => "2",
        },
    )?;

    // Used in a few callbacks
    static T_WRITE: AtomicI32 = AtomicI32::new(0);
    static BEAT: LazyLock<ArcSwap<parser::Beat>> =
        // 'Silent' beat by default
        LazyLock::new(|| ArcSwap::new(Arc::new(parser::Beat::default())));
    // See struct declarations
    let sts = StateChangeState::new(event_tx);
    let ts = TimerState::new(&T_WRITE, stream.clone(), t_play);
    let ps = ProcessState::new(&T_WRITE, &BEAT, producer);
    let cs = CommandState::new(stream.clone(), &BEAT);

    // Attach a command callback to the mpsc rx so event handler can bark at us
    let _recv = command_rx.attach(main_loop.loop_(), move |msg| {
        trace!("pipewire thread received command: {:?}", msg);
        match msg {
            AudioCommand::Play => cs.stream.set_active(true).unwrap(),
            AudioCommand::Pause => cs.stream.set_active(false).unwrap(),
            AudioCommand::NewBeat(beat) => {
                cs.beat.store(Arc::new(beat));
            }
            AudioCommand::SetVolume(vol) => {
                set_volume(&cs.stream, vol);
            }
        }
    });

    // Attach a timer so we can regularly send the current 't' being played to the scope widget
    let t_sync_timer = main_loop.loop_().add_timer(move |_| {
        let head = estimate_play_head(&ts.stream, ts.t_write.load(Ordering::Relaxed));
        ts.t_play.store(head, Ordering::Relaxed);
    });
    t_sync_timer.update_timer(Some(super::T_SYNC_INTERVAL), Some(super::T_SYNC_INTERVAL));

    let _listener = stream
        .add_local_listener_with_user_data(ps)
        .process(on_process)
        .state_changed(move |_, _, _, new| {
            let new_state = match new {
                StreamState::Error(e) => {
                    error!("pipewire thread reports stream error: {:?}", e);
                    StreamStatus::Error
                }
                StreamState::Unconnected => StreamStatus::Unconnected,
                StreamState::Connecting => StreamStatus::Connecting,
                StreamState::Paused => StreamStatus::Paused,
                StreamState::Streaming => StreamStatus::Streaming,
            };

            trace!("pipewire thread sending state change: {:?}", new_state);
            let _ = sts
                .event_tx
                .send(Event::Audio(AudioEvent::StateChange(new_state)));
        })
        .register()?;

    // Twiddle our audio settings
    use spa::param::audio;
    let mut audio_info = audio::AudioInfoRaw::new();
    audio_info.set_format(audio::AudioFormat::U8);
    audio_info.set_rate(BITRATE as u32);
    audio_info.set_channels(CHANNELS as u32);
    let mut position = [0; audio::MAX_CHANNELS];
    position[0] = libspa_sys::SPA_AUDIO_CHANNEL_FL;
    position[1] = libspa_sys::SPA_AUDIO_CHANNEL_FR;
    audio_info.set_position(position);

    // Serialize it into a native POD for pipewire
    let values: Vec<u8> = pw::spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &pw::spa::pod::Value::Object(pw::spa::pod::Object {
            type_: libspa_sys::SPA_TYPE_OBJECT_Format,
            id: libspa_sys::SPA_PARAM_EnumFormat,
            properties: audio_info.into(),
        }),
    )
    .unwrap()
    .0
    .into_inner();

    let mut params = [spa::pod::Pod::from_bytes(&values).unwrap()];

    stream.connect(
        Direction::Output,
        None,
        StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS,
        &mut params,
    )?;
    // TODO: Starting at Max is uncomfortable for my system, but is it just me?
    set_volume(&stream, Volume::default());
    stream.set_active(false)?;

    info!("pipewire thread startup complete, starting main loop");
    main_loop.run();
    info!("pipewire thread exiting");
    Ok(())
}

fn on_process(s: &Stream, state: &mut ProcessState) {
    match s.dequeue_buffer() {
        None => warn!("no buffer available for pipewire process thread"),
        Some(mut buffer) => {
            // We may get a valid buffer that is 0-sized(?)
            let n_frames = if let Some(slice) = buffer.datas_mut()[0].data() {
                let n_frames = slice.len() / STRIDE;
                for i in 0..n_frames {
                    // I thought walking an AST like this in a RT audio loop would cause like a million xruns,
                    // but pw-top stats are about the same as when it was hardcoded. Crazy!
                    let val = state
                        .beat
                        .load()
                        .eval(state.t_write.load(Ordering::Relaxed));
                    state
                        .t_write
                        .store(state.t_write.load(Ordering::Relaxed) + 1, Ordering::Relaxed);

                    // Copy it across strides
                    for c in 0..CHANNELS {
                        let start = i * STRIDE + (c * size_of::<u8>());
                        let end = start + size_of::<u8>();
                        let chan = &mut slice[start..end];

                        chan.copy_from_slice(&u8::to_le_bytes(val));
                    }

                    // Push to visualization buffer (best effort)
                    // We only need one channel for visualization
                    if !state.producer.is_full() {
                        let _ = state.producer.push(val);
                    }
                }
                n_frames
            } else {
                0
            };
            // Pipewire must be told which region of this data is valid
            let chunk = &mut buffer.datas_mut()[0].chunk_mut();
            *chunk.offset_mut() = 0;
            *chunk.stride_mut() = STRIDE as _;
            *chunk.size_mut() = (STRIDE * n_frames) as _;
        }
    }
}

fn set_volume(stream: &Stream, volume: Volume) {
    const _: () = assert!(CHANNELS == 2, "The way we set this only works on stereo!");
    // We modify the stream properties rather than doing it ourselves.
    // Trust pipewire can do it better than f32 * u8 -> u8
    let vol_val = volume.val();
    let _ = stream
        .set_control(libspa_sys::SPA_PROP_volume, &[vol_val, vol_val])
        .inspect_err(|e| error!("audio thread reported problem changing volume: {}", e));
}

/// RT Safe. Shouldn't mutate stream at all. Estimates which 't' we're playing next
///
/// We want to know which 't' sample is playing now
/// We know how many t's we've produced
/// We're about to know how many t's are queued, and how many are buffered
fn estimate_play_head(stream: &Stream, t_write: i32) -> i32 {
    unsafe {
        // It's all numbers inside so zeroed is fine
        let mut time: pipewire_sys::pw_time = mem::zeroed();
        pipewire_sys::pw_stream_get_time_n(
            stream.as_raw_ptr(),
            &mut time,
            mem::size_of::<pipewire_sys::pw_time>(),
        );
        t_write - (time.queued as i32 + time.buffered as i32)
    }
}
