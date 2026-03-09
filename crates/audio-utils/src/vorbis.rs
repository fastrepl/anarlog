use std::fs::File;
use std::io::BufReader;
use std::num::{NonZeroU8, NonZeroU32};
use std::path::Path;

use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use vorbis_rs::{VorbisBitrateManagementStrategy, VorbisDecoder, VorbisEncoderBuilder};

use crate::{Error, deinterleave};

pub const DEFAULT_VORBIS_QUALITY: f32 = 0.7;
pub const DEFAULT_VORBIS_BLOCK_SIZE: usize = 4096;

#[derive(Clone, Copy, Debug)]
pub struct VorbisEncodeSettings {
    pub quality: f32,
    pub block_size: usize,
}

impl Default for VorbisEncodeSettings {
    fn default() -> Self {
        Self {
            quality: DEFAULT_VORBIS_QUALITY,
            block_size: DEFAULT_VORBIS_BLOCK_SIZE,
        }
    }
}

pub fn encode_vorbis_from_channels(
    channels: &[&[f32]],
    sample_rate: NonZeroU32,
    settings: VorbisEncodeSettings,
) -> Result<Vec<u8>, Error> {
    let channel_count = channels.len();
    if channel_count == 0 {
        return Err(Error::EmptyChannelSet);
    }

    let channel_count_u8 = u8::try_from(channel_count).map_err(|_| Error::TooManyChannels {
        count: channel_count,
    })?;
    let channel_count = NonZeroU8::new(channel_count_u8).ok_or(Error::EmptyChannelSet)?;

    let reference_len = channels[0].len();
    for (index, channel) in channels.iter().enumerate() {
        if channel.len() != reference_len {
            return Err(Error::ChannelDataLengthMismatch { channel: index });
        }
    }

    let mut ogg_buffer = Vec::new();
    let mut encoder = VorbisEncoderBuilder::new(sample_rate, channel_count, &mut ogg_buffer)?
        .bitrate_management_strategy(VorbisBitrateManagementStrategy::QualityVbr {
            target_quality: settings.quality,
        })
        .build()?;

    let block_size = settings.block_size.max(1);
    let mut offsets = vec![0usize; channels.len()];

    loop {
        let mut slices: Vec<&[f32]> = Vec::with_capacity(channels.len());
        let mut has_samples = false;

        for (index, channel) in channels.iter().enumerate() {
            let start = offsets[index];
            if start >= channel.len() {
                slices.push(&[]);
                continue;
            }

            let end = (start + block_size).min(channel.len());
            if end > start {
                has_samples = true;
            }

            slices.push(&channel[start..end]);
            offsets[index] = end;
        }

        if !has_samples {
            break;
        }

        encoder.encode_audio_block(&slices)?;
    }

    encoder.finish()?;
    Ok(ogg_buffer)
}

pub fn encode_vorbis_from_interleaved(
    samples: &[f32],
    channel_count: NonZeroU8,
    sample_rate: NonZeroU32,
    settings: VorbisEncodeSettings,
) -> Result<Vec<u8>, Error> {
    let channels = deinterleave(samples, channel_count.get() as usize);
    let channel_refs: Vec<&[f32]> = channels.iter().map(Vec::as_slice).collect();
    encode_vorbis_from_channels(&channel_refs, sample_rate, settings)
}

pub fn encode_vorbis_mono(
    samples: &[f32],
    sample_rate: NonZeroU32,
    settings: VorbisEncodeSettings,
) -> Result<Vec<u8>, Error> {
    encode_vorbis_from_channels(&[samples], sample_rate, settings)
}

#[derive(Clone, Copy)]
enum DecodeMode {
    Source,
    Mono,
}

pub fn decode_vorbis_to_wav_file(
    ogg_path: impl AsRef<Path>,
    wav_path: impl AsRef<Path>,
) -> Result<(), Error> {
    decode_vorbis_to_wav_file_with_mode(ogg_path, wav_path, DecodeMode::Source)
}

pub fn decode_vorbis_to_mono_wav_file(
    ogg_path: impl AsRef<Path>,
    wav_path: impl AsRef<Path>,
) -> Result<(), Error> {
    decode_vorbis_to_wav_file_with_mode(ogg_path, wav_path, DecodeMode::Mono)
}

pub fn ogg_has_identical_channels(ogg_path: impl AsRef<Path>) -> Result<bool, Error> {
    const MAX_FRAMES_TO_CHECK: usize = 1000;
    const EPSILON: f32 = 1e-6;

    let ogg_reader = BufReader::new(File::open(ogg_path)?);
    let mut decoder = VorbisDecoder::new(ogg_reader)?;

    if decoder.channels().get() != 2 {
        return Ok(true);
    }

    let mut frames_checked = 0;
    while let Some(block) = decoder.decode_audio_block()? {
        let samples = block.samples();
        if samples.len() < 2 {
            continue;
        }

        let left = &samples[0];
        let right = &samples[1];
        let frame_count = left.len().min(right.len());

        for i in 0..frame_count {
            if (left[i] - right[i]).abs() > EPSILON {
                return Ok(false);
            }
            frames_checked += 1;
            if frames_checked >= MAX_FRAMES_TO_CHECK {
                return Ok(true);
            }
        }
    }

    Ok(true)
}

fn decode_vorbis_to_wav_file_with_mode(
    ogg_path: impl AsRef<Path>,
    wav_path: impl AsRef<Path>,
    mode: DecodeMode,
) -> Result<(), Error> {
    let ogg_reader = BufReader::new(File::open(ogg_path)?);
    let mut decoder = VorbisDecoder::new(ogg_reader)?;

    let wav_spec = WavSpec {
        channels: match mode {
            DecodeMode::Source => decoder.channels().get() as u16,
            DecodeMode::Mono => 1,
        },
        sample_rate: decoder.sampling_frequency().get(),
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };

    let mut writer = WavWriter::create(wav_path, wav_spec)?;

    while let Some(block) = decoder.decode_audio_block()? {
        let samples = block.samples();
        if samples.is_empty() {
            continue;
        }

        let frame_count = samples[0].len();
        for (index, channel) in samples.iter().enumerate() {
            if channel.len() != frame_count {
                return Err(Error::ChannelDataLengthMismatch { channel: index });
            }
        }

        match mode {
            DecodeMode::Source => {
                for frame in 0..frame_count {
                    for channel in samples.iter() {
                        writer.write_sample(channel[frame])?;
                    }
                }
            }
            DecodeMode::Mono => {
                let channel_count = samples.len() as f32;
                for frame in 0..frame_count {
                    let sum: f32 = samples.iter().map(|channel| channel[frame]).sum();
                    writer.write_sample(sum / channel_count)?;
                }
            }
        }
    }

    writer.flush()?;
    writer.finalize()?;
    Ok(())
}

#[derive(Clone, Copy)]
enum EncodeWavMode {
    SourceChannels,
    MonoAsStereo,
}

pub fn encode_wav_to_vorbis_file(
    wav_path: impl AsRef<Path>,
    ogg_path: impl AsRef<Path>,
    settings: VorbisEncodeSettings,
) -> Result<(), Error> {
    encode_wav_to_vorbis_file_with_mode(wav_path, ogg_path, settings, EncodeWavMode::SourceChannels)
}

pub fn encode_wav_to_vorbis_file_mono_as_stereo(
    wav_path: impl AsRef<Path>,
    ogg_path: impl AsRef<Path>,
    settings: VorbisEncodeSettings,
) -> Result<(), Error> {
    encode_wav_to_vorbis_file_with_mode(wav_path, ogg_path, settings, EncodeWavMode::MonoAsStereo)
}

fn encode_wav_to_vorbis_file_with_mode(
    wav_path: impl AsRef<Path>,
    ogg_path: impl AsRef<Path>,
    settings: VorbisEncodeSettings,
    mode: EncodeWavMode,
) -> Result<(), Error> {
    let mut reader = WavReader::open(wav_path)?;
    let spec = reader.spec();

    let sample_rate = non_zero_sample_rate(spec.sample_rate)?;
    let samples: Vec<f32> = reader.samples::<f32>().collect::<Result<_, _>>()?;

    let (channel_count, samples) = match mode {
        EncodeWavMode::SourceChannels => (non_zero_channel_count(spec.channels)?, samples),
        EncodeWavMode::MonoAsStereo => {
            if spec.channels == 1 {
                let mut stereo_samples = Vec::with_capacity(samples.len().saturating_mul(2));
                for sample in samples {
                    stereo_samples.push(sample);
                    stereo_samples.push(sample);
                }
                (NonZeroU8::new(2).unwrap(), stereo_samples)
            } else {
                (non_zero_channel_count(spec.channels)?, samples)
            }
        }
    };

    let encoded = encode_vorbis_from_interleaved(&samples, channel_count, sample_rate, settings)?;

    std::fs::write(ogg_path, encoded)?;

    Ok(())
}

fn non_zero_sample_rate(sample_rate: u32) -> Result<NonZeroU32, Error> {
    NonZeroU32::new(sample_rate).ok_or(Error::InvalidSampleRate(sample_rate))
}

fn non_zero_channel_count(channels: u16) -> Result<NonZeroU8, Error> {
    let channel_count_u8 =
        u8::try_from(channels).map_err(|_| Error::UnsupportedChannelCount { count: channels })?;
    NonZeroU8::new(channel_count_u8).ok_or(Error::UnsupportedChannelCount { count: channels })
}

pub fn mix_down_to_mono(samples: &[f32], channels: NonZeroU8) -> Vec<f32> {
    let channel_count = channels.get() as usize;
    if channel_count <= 1 {
        return samples.to_vec();
    }

    let mut mono = Vec::with_capacity(samples.len() / channel_count);
    for frame in samples.chunks(channel_count) {
        let sum: f32 = frame.iter().copied().sum();
        mono.push(sum / frame.len() as f32);
    }
    mono
}
