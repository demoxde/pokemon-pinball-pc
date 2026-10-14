//! Sound output: the emulator pushes stereo samples into a ring buffer that the sound card drains.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, Stream, StreamConfig};
use ringbuf::traits::Consumer;
use rustboyadvance_core::prelude::{DynAudioInterface, SimpleAudioInterface};
use rustboyadvance_utils::audio::SampleConsumer;

/// Ring buffer size in samples (both channels): about 85ms at 48kHz
const BUFFER_SAMPLES: usize = 8192;

pub fn start() -> Result<(DynAudioInterface, Stream), String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("no sound output device")?;
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    let config: StreamConfig = supported.config();
    let (interface, consumer) =
        SimpleAudioInterface::create_channel(config.sample_rate as i32, Some(BUFFER_SAMPLES));

    let stream = match supported.sample_format() {
        SampleFormat::F32 => build::<f32>(&device, config, consumer),
        SampleFormat::I16 => build::<i16>(&device, config, consumer),
        SampleFormat::U16 => build::<u16>(&device, config, consumer),
        SampleFormat::I32 => build::<i32>(&device, config, consumer),
        other => return Err(format!("unsupported sample format {other:?}")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((interface, stream))
}

fn build<T>(
    device: &cpal::Device,
    config: StreamConfig,
    mut samples: SampleConsumer,
) -> Result<Stream, String>
where
    T: SizedSample + FromSample<i16>,
{
    let channels = config.channels as usize;
    device
        .build_output_stream(
            config,
            move |out: &mut [T], _| {
                for frame in out.chunks_mut(channels) {
                    // On underrun play silence rather than stale data
                    let left = samples.try_pop().unwrap_or(0);
                    let right = samples.try_pop().unwrap_or(left);
                    for (i, s) in frame.iter_mut().enumerate() {
                        let v = match i {
                            0 => left,
                            1 => right,
                            _ => ((left as i32 + right as i32) / 2) as i16,
                        };
                        *s = T::from_sample(v);
                    }
                }
            },
            |e| log::warn!("audio stream error: {e}"),
            None,
        )
        .map_err(|e| e.to_string())
}
