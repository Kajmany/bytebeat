use std::sync::mpsc;

use pipewire::{
    self as pw,
    context::ContextRc,
    main_loop::MainLoopRc,
    spa::{self, utils::Direction},
    stream::{Stream, StreamFlags, StreamState},
};
use pw::properties::properties;

use crate::event::Event;

const CHANNELS: usize = 2;
const STRIDE: usize = size_of::<u8>() * CHANNELS;

#[derive(Clone, Debug)]
pub enum AudioEvent {
    StateChange(StreamStatus),
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Remapping of [`pipewire::stream::StreamState`] that can be cloned.
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

#[derive(Clone, Debug)]
pub enum AudioCommand {
    Play,
    Pause,
}

struct AudioState {
    pub t: u64,
    pub event_tx: mpsc::Sender<Event>,
}

impl AudioState {
    pub fn new(event_tx: mpsc::Sender<Event>) -> AudioState {
        AudioState { t: 0, event_tx }
    }
}

pub fn main(
    event_tx: mpsc::Sender<Event>,
    command_rx: pipewire::channel::Receiver<AudioCommand>,
) -> Result<(), pw::Error> {
    let state = AudioState::new(event_tx);
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

    // FIXME: Uhh, this is not robust
    //   but it holds up to spamming toggle so it works for now...?
    let stream2 = stream.clone();
    let _recv = command_rx.attach(main_loop.loop_(), move |msg| match msg {
        AudioCommand::Play => stream2.set_active(true).unwrap(),
        AudioCommand::Pause => stream2.set_active(false).unwrap(),
    });

    let _listener = stream
        .add_local_listener_with_user_data(state)
        .process(on_process)
        .state_changed(|_, state, _, new| {
            // TODO: Have a sense of shame. Do better.
            let new_state = match new {
                StreamState::Error(_) => StreamStatus::Error,
                StreamState::Unconnected => StreamStatus::Unconnected,
                StreamState::Connecting => StreamStatus::Connecting,
                StreamState::Paused => StreamStatus::Paused,
                StreamState::Streaming => StreamStatus::Streaming,
            };
            // TODO: probably okay but why
            let _ = state
                .event_tx
                .send(Event::Audio(AudioEvent::StateChange(new_state)));
        })
        .register()?;

    // Twiddle our audio settings
    use spa::param::audio;
    let mut audio_info = audio::AudioInfoRaw::new();
    audio_info.set_format(audio::AudioFormat::U8);
    audio_info.set_rate(8000);
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
    stream.set_active(false)?;

    main_loop.run();
    Ok(())
}

fn on_process(s: &Stream, state: &mut AudioState) {
    let t = &mut state.t;
    match s.dequeue_buffer() {
        None => println!("Got no buffer!"),
        Some(mut buffer) => {
            // We may get a valid buffer that is 0-sized(?)
            let n_frames = if let Some(slice) = buffer.datas_mut()[0].data() {
                let n_frames = slice.len() / STRIDE;
                for i in 0..n_frames {
                    // First gen the frame
                    //let value = (*t * (42 & *t >> 10)) as u8;
                    let value = ((5 * *t) & (*t >> 7) | (3 * *t) & (*t >> 10)) as u8;
                    *t += 1;

                    // Copy it across strides
                    for c in 0..CHANNELS {
                        let start = i * STRIDE + (c * size_of::<u8>());
                        let end = start + size_of::<u8>();
                        let chan = &mut slice[start..end];
                        chan.copy_from_slice(&u8::to_le_bytes(value));
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
