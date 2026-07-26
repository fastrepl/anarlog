use std::convert::TryFrom;

use bytes::{BufMut, Bytes, BytesMut};
use futures_util::{Stream, StreamExt};
use hypr_audio_interface::AsyncSource;

mod error;
mod pcm;
mod vorbis;

pub use error::*;
pub use pcm::*;
pub use vorbis::*;

pub use rodio::Source;

const I16_SCALE: f32 = 32768.0;

pub fn mono_frames(
    mut source: impl Iterator<Item = f32>,
    channel_count: usize,
) -> impl Iterator<Item = f32> {
    std::iter::from_fn(move || {
        let first = source.next()?;
        let mut sum = first;
        let mut count = 1usize;
        while count < channel_count {
            let Some(sample) = source.next() else {
                break;
            };
            sum += sample;
            count += 1;
        }
        Some(sum / count as f32)
    })
}

#[derive(Debug, Clone, Copy)]
pub struct AudioMetadata {
    pub sample_rate: u32,
    pub channels: u8,
}

impl<T: AsyncSource> AudioFormatExt for T {}

pub trait AudioFormatExt: AsyncSource {
    fn to_i16_le_chunks(
        self,
        sample_rate: u32,
        chunk_size: usize,
    ) -> impl Stream<Item = Bytes> + Send + Unpin
    where
        Self: Sized + Send + Unpin + 'static,
    {
        hypr_resampler::ResamplerDynamicOld::new(self, sample_rate)
            .chunks(chunk_size)
            .map(|chunk| {
                let n = std::mem::size_of::<f32>() * chunk.len();

                let mut buf = BytesMut::with_capacity(n);
                for sample in chunk {
                    let scaled = (sample * I16_SCALE).clamp(-I16_SCALE, I16_SCALE);
                    buf.put_i16_le(scaled as i16);
                }
                buf.freeze()
            })
    }
}

pub fn i16_to_f32_samples(samples: &[i16]) -> Vec<f32> {
    samples
        .iter()
        .map(|&sample| sample as f32 / I16_SCALE)
        .collect()
}

pub fn f32_to_i16_samples(samples: &[f32]) -> Vec<i16> {
    samples
        .iter()
        .map(|&sample| {
            let scaled = (sample * I16_SCALE).clamp(-I16_SCALE, I16_SCALE);
            scaled as i16
        })
        .collect()
}

pub fn f32_to_i16_bytes<I>(samples: I) -> Bytes
where
    I: Iterator<Item = f32>,
{
    let mut buf = BytesMut::new();
    for sample in samples {
        let i16_sample = (sample * I16_SCALE).clamp(-I16_SCALE, I16_SCALE) as i16;
        buf.put_i16_le(i16_sample);
    }
    buf.freeze()
}

pub fn bytes_to_f32_samples(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(2)
        .map(|chunk| {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            sample as f32 / I16_SCALE
        })
        .collect()
}

pub fn deinterleave_stereo_bytes(data: &[u8]) -> (Vec<f32>, Vec<f32>) {
    let num_frames = data.len() / 4;
    let mut ch0 = Vec::with_capacity(num_frames);
    let mut ch1 = Vec::with_capacity(num_frames);
    for frame in data.chunks_exact(4) {
        ch0.push(i16::from_le_bytes([frame[0], frame[1]]) as f32 / I16_SCALE);
        ch1.push(i16::from_le_bytes([frame[2], frame[3]]) as f32 / I16_SCALE);
    }
    (ch0, ch1)
}

pub fn deinterleave(samples: &[f32], channels: usize) -> Vec<Vec<f32>> {
    if channels <= 1 {
        return vec![samples.to_vec()];
    }
    let mut output = vec![Vec::with_capacity(samples.len() / channels + 1); channels];
    for (index, sample) in samples.iter().enumerate() {
        output[index % channels].push(*sample);
    }
    output
}

pub fn interleave(channels: &[Vec<f32>]) -> Vec<f32> {
    if channels.is_empty() {
        return Vec::new();
    }
    let frames = channels.iter().map(|c| c.len()).max().unwrap_or(0);
    let mut output = Vec::with_capacity(frames * channels.len());
    for frame in 0..frames {
        for ch in channels {
            output.push(ch.get(frame).copied().unwrap_or(0.0));
        }
    }
    output
}

pub fn mix_sample_f32(mic: f32, speaker: f32) -> f32 {
    (mic + speaker).clamp(-1.0, 1.0)
}

pub fn mix_audio_f32(mic: &[f32], speaker: &[f32]) -> Vec<f32> {
    let max_len = mic.len().max(speaker.len());
    (0..max_len)
        .map(|i| {
            let m = mic.get(i).copied().unwrap_or(0.0);
            let s = speaker.get(i).copied().unwrap_or(0.0);
            mix_sample_f32(m, s)
        })
        .collect()
}

pub fn mix_audio_pcm16le(mic: &[u8], speaker: &[u8]) -> Vec<u8> {
    let max_len = mic.len().max(speaker.len());
    let mut mixed = Vec::with_capacity(max_len);

    let mut index = 0;
    while index < max_len {
        let mic_sample = if index + 1 < mic.len() {
            i16::from_le_bytes([mic[index], mic[index + 1]])
        } else {
            0
        };

        let speaker_sample = if index + 1 < speaker.len() {
            i16::from_le_bytes([speaker[index], speaker[index + 1]])
        } else {
            0
        };

        let mixed_sample = ((mic_sample as i32 + speaker_sample as i32) / 2) as i16;

        mixed.extend_from_slice(&mixed_sample.to_le_bytes());
        index += 2;
    }

    mixed
}

pub use hypr_audio_mime::content_type_to_extension;

pub fn source_from_path(
    path: impl AsRef<std::path::Path>,
) -> Result<rodio::Decoder<std::io::BufReader<std::fs::File>>, crate::Error> {
    let file = std::fs::File::open(path.as_ref())?;
    let decoder = rodio::Decoder::try_from(file)?;
    Ok(decoder)
}

fn metadata_from_source<S>(source: &S) -> Result<AudioMetadata, crate::Error>
where
    S: Source,
{
    let sample_rate: u32 = source.sample_rate().into();
    let channels_u16: u16 = source.channels().into();

    let channels =
        u8::try_from(channels_u16).map_err(|_| crate::Error::UnsupportedChannelCount {
            count: channels_u16,
        })?;

    Ok(AudioMetadata {
        sample_rate,
        channels,
    })
}

pub fn audio_file_metadata(
    path: impl AsRef<std::path::Path>,
) -> Result<AudioMetadata, crate::Error> {
    let source = source_from_path(path)?;
    metadata_from_source(&source)
}

pub fn resample_audio<S>(source: S, to_rate: u32) -> Result<Vec<f32>, crate::Error>
where
    S: rodio::Source,
{
    use audioadapter_buffers::direct::SequentialSliceOfVecs;
    use rubato::{
        Async, FixedAsync, Resampler, SincInterpolationParameters, SincInterpolationType,
        WindowFunction,
    };

    let from_rate = u32::from(source.sample_rate()) as f64;
    let channels = u16::from(source.channels()) as usize;
    let to_rate_f64 = to_rate as f64;

    let samples: Vec<f32> = source.collect();

    if (from_rate - to_rate_f64).abs() < 1.0 {
        return Ok(samples);
    }

    let chunk_size = 1024;
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let mut resampler = Async::<f32>::new_sinc(
        to_rate_f64 / from_rate,
        2.0,
        &params,
        chunk_size,
        channels,
        FixedAsync::Input,
    )?;

    let frames_per_channel = samples.len() / channels;
    let mut input_channels: Vec<Vec<f32>> = vec![Vec::with_capacity(frames_per_channel); channels];
    for (i, &sample) in samples.iter().enumerate() {
        input_channels[i % channels].push(sample);
    }

    let input_adapter = SequentialSliceOfVecs::new(&input_channels, channels, frames_per_channel)
        .expect("input adapter");
    let output_len = resampler.process_all_needed_output_len(frames_per_channel);
    let mut output_channels: Vec<Vec<f32>> = vec![vec![0.0; output_len]; channels];
    let mut output_adapter =
        SequentialSliceOfVecs::new_mut(&mut output_channels, channels, output_len)
            .expect("output adapter");
    let (_, total_frames) = resampler.process_all_into_buffer(
        &input_adapter,
        &mut output_adapter,
        frames_per_channel,
        None,
    )?;

    let mut output = Vec::with_capacity(total_frames * channels);
    for frame in 0..total_frames {
        for ch in output_channels.iter() {
            output.push(ch[frame]);
        }
    }

    Ok(output)
}

#[derive(Debug)]
pub struct ChunkedAudio {
    pub chunks: Vec<Bytes>,
    pub sample_count: usize,
    pub frame_count: usize,
    pub metadata: AudioMetadata,
}

pub fn chunk_audio_file(
    path: impl AsRef<std::path::Path>,
    chunk_ms: u64,
) -> Result<ChunkedAudio, crate::Error> {
    let source = source_from_path(path)?;
    let metadata = metadata_from_source(&source)?;
    let samples = resample_audio(source, metadata.sample_rate)?;

    if samples.is_empty() {
        return Ok(ChunkedAudio {
            chunks: Vec::new(),
            sample_count: 0,
            frame_count: 0,
            metadata,
        });
    }

    let channels = metadata.channels.max(1) as usize;
    let frames_per_chunk = {
        let frames = (chunk_ms as u128)
            .saturating_mul(metadata.sample_rate as u128)
            .div_ceil(1000);
        frames.max(1).min(usize::MAX as u128) as usize
    };
    let samples_per_chunk = frames_per_chunk
        .saturating_mul(channels)
        .clamp(1, usize::MAX);

    let sample_count = samples.len();
    let frame_count = sample_count / channels;
    let chunks = samples
        .chunks(samples_per_chunk)
        .map(|chunk| f32_to_i16_bytes(chunk.iter().copied()))
        .collect();

    Ok(ChunkedAudio {
        chunks,
        sample_count,
        frame_count,
        metadata,
    })
}

pub fn chunk_size_for_stt(sample_rate: u32) -> usize {
    // https://github.com/orgs/deepgram/discussions/224#discussioncomment-6234166
    const CHUNK_MS: u32 = 120;

    let samples = ((sample_rate as u64) * (CHUNK_MS as u64)) / 1000;
    samples.clamp(1024, 7168) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! test_audio_file_metadata {
        ($($name:ident: $path:expr),* $(,)?) => {
            $(
                #[test]
                fn $name() {
                    let metadata = audio_file_metadata($path).unwrap();
                    assert!(metadata.sample_rate > 0);
                    assert!(metadata.channels > 0);
                }
            )*
        };
    }

    test_audio_file_metadata! {
        test_audio_file_metadata_wav: hypr_data::english_1::AUDIO_PATH,
        test_audio_file_metadata_mp3: hypr_data::english_1::AUDIO_MP3_PATH,
        test_audio_file_metadata_mp4: hypr_data::english_1::AUDIO_MP4_PATH,
        test_audio_file_metadata_m4a: hypr_data::english_1::AUDIO_M4A_PATH,
        test_audio_file_metadata_ogg: hypr_data::english_1::AUDIO_OGG_PATH,
        test_audio_file_metadata_flac: hypr_data::english_1::AUDIO_FLAC_PATH,
        test_audio_file_metadata_aac: hypr_data::english_1::AUDIO_AAC_PATH,
        test_audio_file_metadata_aiff: hypr_data::english_1::AUDIO_AIFF_PATH,
    }

    #[test]
    fn resample_preserves_duration() {
        let from_rate = 44100;
        let to_rate = 16000;
        let duration_secs = 5;
        let channels = 2;
        let total_samples = from_rate * duration_secs * channels;

        let source = rodio::buffer::SamplesBuffer::new(
            std::num::NonZeroU16::new(channels as u16).unwrap(),
            std::num::NonZeroU32::new(from_rate as u32).unwrap(),
            vec![0.5f32; total_samples],
        );
        let output = resample_audio(source, to_rate as u32).unwrap();

        let expected_frames = duration_secs * to_rate;
        let actual_frames = output.len() / channels;
        let ratio = actual_frames as f64 / expected_frames as f64;
        assert!(
            (ratio - 1.0).abs() < 0.02,
            "expected ~{expected_frames} frames, got {actual_frames} (ratio {ratio:.4})"
        );
    }

    #[test]
    fn resample_short_partial_chunk_preserves_duration() {
        let from_rate = 44_100;
        let to_rate = 16_000;
        let channels = 2;
        let input_frames = 200;

        let source = rodio::buffer::SamplesBuffer::new(
            std::num::NonZeroU16::new(channels as u16).unwrap(),
            std::num::NonZeroU32::new(from_rate as u32).unwrap(),
            vec![0.5f32; input_frames * channels],
        );
        let output = resample_audio(source, to_rate as u32).unwrap();

        let actual_frames = output.len() / channels;
        let expected_frames = (input_frames as f64 * to_rate as f64 / from_rate as f64).round();
        assert!(
            (actual_frames as f64 - expected_frames).abs() <= 2.0,
            "expected ~{expected_frames} frames, got {actual_frames}"
        );
    }
}
