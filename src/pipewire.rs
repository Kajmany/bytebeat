use pipewire::{
    self as pw, spa,
    spa::utils::Direction,
    stream::{Stream, StreamFlags},
};
use pw::properties::properties;

const CHANNELS: usize = 2;
const STRIDE: usize = size_of::<u8>() * CHANNELS;

pub fn main() -> Result<(), pw::Error> {
    pw::init();
    let main_loop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&main_loop, None)?;
    let core = context.connect_rc(None)?;

    let stream = pw::stream::StreamBox::new(
        &core,
        "audio-src",
        properties! {
            *pw::keys::MEDIA_TYPE => "Audio",
            *pw::keys::MEDIA_ROLE => "Music",
            *pw::keys::MEDIA_CATEGORY => "Playback",
            *pw::keys::AUDIO_CHANNELS => "2",
        },
    )?;

    let data: u64 = 0;
    let _listener = stream
        .add_local_listener_with_user_data(data)
        .process(on_process)
        .state_changed(|_, _, old, new| {
            //println!("State changed: {:?} -> {:?}", old, new);
        })
        .register()?;
    //println!("Created stream {:#?}", stream);

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

    main_loop.run();
    Ok(())
}

fn on_process(s: &Stream, data: &mut u64) {
    match s.dequeue_buffer() {
        None => println!("Got no buffer!"),
        Some(mut buffer) => {
            // We may get a valid buffer that is 0-sized(?)
            let n_frames = if let Some(slice) = buffer.datas_mut()[0].data() {
                let n_frames = slice.len() / STRIDE;
                //println!("Buffer with room for {}", n_frames);
                for i in 0..n_frames {
                    // First gen the frame
                    let value = (*data * (42 & *data >> 10)) as u8;
                    //let value = (5 * *data & *data >> 7 | 3 * *data & *data >> 10) as u8;
                    *data += 1;

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
