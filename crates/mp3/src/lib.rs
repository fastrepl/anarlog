mod error;

pub use error::Error;

use std::path::Path;

use hound::SampleFormat;
use mp3lame_encoder::{Builder as LameBuilder, DualPcm, FlushNoGap, MonoPcm};

const CHUNK_FRAMES: usize = 4096;

pub fn encode_wav(wav_path: &Path, mp3_path: &Path) -> Result<(), Error> {
    let mut reader = hound::WavReader::open(wav_path)?;
    let spec = reader.spec();

    let num_channels = match spec.channels {
        1 => 1u8,
        2 => 2u8,
        count => return Err(Error::UnsupportedChannelCount(count)),
    };
    let bitrate = if num_channels > 1 {
        mp3lame_encoder::Bitrate::Kbps128
    } else {
        mp3lame_encoder::Bitrate::Kbps64
    };

    let sample_rate = spec.sample_rate;

    let mut encoder = {
        let mut mp3_builder = LameBuilder::new().ok_or(Error::LameInit)?;
        mp3_builder
            .set_num_channels(num_channels)
            .map_err(|e| Error::LameConfig(format!("{:?}", e)))?;
        mp3_builder
            .set_sample_rate(sample_rate)
            .map_err(|e| Error::LameConfig(format!("{:?}", e)))?;

        mp3_builder
            .set_brate(bitrate)
            .map_err(|e| Error::LameConfig(format!("{:?}", e)))?;
        mp3_builder
            .set_quality(mp3lame_encoder::Quality::NearBest)
            .map_err(|e| Error::LameConfig(format!("{:?}", e)))?;

        mp3_builder
            .build()
            .map_err(|e| Error::LameBuild(format!("{:?}", e)))?
    };

    let mut mp3_out: Vec<u8> = Vec::new();
    let bits_per_sample = spec.bits_per_sample;

    match spec.sample_format {
        SampleFormat::Float => {
            if bits_per_sample != 32 {
                return Err(Error::UnsupportedFloatBitDepth(bits_per_sample));
            }

            if num_channels == 1 {
                encode_mono_samples(reader.samples::<f32>(), f32_to_i16, |chunk| {
                    mp3_out.reserve(mp3lame_encoder::max_required_buffer_size(chunk.len()));
                    let input = MonoPcm(chunk);
                    encoder
                        .encode_to_vec(input, &mut mp3_out)
                        .map_err(|e| Error::LameEncode(format!("{:?}", e)))
                })?;
            } else {
                encode_stereo_samples(reader.samples::<f32>(), f32_to_i16, |left, right| {
                    mp3_out.reserve(mp3lame_encoder::max_required_buffer_size(left.len()));
                    let input = DualPcm { left, right };
                    encoder
                        .encode_to_vec(input, &mut mp3_out)
                        .map_err(|e| Error::LameEncode(format!("{:?}", e)))
                })?;
            }
        }
        SampleFormat::Int => match bits_per_sample {
            1..=8 => {
                if num_channels == 1 {
                    encode_mono_samples(
                        reader.samples::<i8>(),
                        |sample| int_to_i16(sample as i32, bits_per_sample),
                        |chunk| {
                            mp3_out.reserve(mp3lame_encoder::max_required_buffer_size(chunk.len()));
                            let input = MonoPcm(chunk);
                            encoder
                                .encode_to_vec(input, &mut mp3_out)
                                .map_err(|e| Error::LameEncode(format!("{:?}", e)))
                        },
                    )?;
                } else {
                    encode_stereo_samples(
                        reader.samples::<i8>(),
                        |sample| int_to_i16(sample as i32, bits_per_sample),
                        |left, right| {
                            mp3_out.reserve(mp3lame_encoder::max_required_buffer_size(left.len()));
                            let input = DualPcm { left, right };
                            encoder
                                .encode_to_vec(input, &mut mp3_out)
                                .map_err(|e| Error::LameEncode(format!("{:?}", e)))
                        },
                    )?;
                }
            }
            9..=16 => {
                if num_channels == 1 {
                    encode_mono_samples(
                        reader.samples::<i16>(),
                        |sample| int_to_i16(sample as i32, bits_per_sample),
                        |chunk| {
                            mp3_out.reserve(mp3lame_encoder::max_required_buffer_size(chunk.len()));
                            let input = MonoPcm(chunk);
                            encoder
                                .encode_to_vec(input, &mut mp3_out)
                                .map_err(|e| Error::LameEncode(format!("{:?}", e)))
                        },
                    )?;
                } else {
                    encode_stereo_samples(
                        reader.samples::<i16>(),
                        |sample| int_to_i16(sample as i32, bits_per_sample),
                        |left, right| {
                            mp3_out.reserve(mp3lame_encoder::max_required_buffer_size(left.len()));
                            let input = DualPcm { left, right };
                            encoder
                                .encode_to_vec(input, &mut mp3_out)
                                .map_err(|e| Error::LameEncode(format!("{:?}", e)))
                        },
                    )?;
                }
            }
            17..=32 => {
                if num_channels == 1 {
                    encode_mono_samples(
                        reader.samples::<i32>(),
                        |sample| int_to_i16(sample, bits_per_sample),
                        |chunk| {
                            mp3_out.reserve(mp3lame_encoder::max_required_buffer_size(chunk.len()));
                            let input = MonoPcm(chunk);
                            encoder
                                .encode_to_vec(input, &mut mp3_out)
                                .map_err(|e| Error::LameEncode(format!("{:?}", e)))
                        },
                    )?;
                } else {
                    encode_stereo_samples(
                        reader.samples::<i32>(),
                        |sample| int_to_i16(sample, bits_per_sample),
                        |left, right| {
                            mp3_out.reserve(mp3lame_encoder::max_required_buffer_size(left.len()));
                            let input = DualPcm { left, right };
                            encoder
                                .encode_to_vec(input, &mut mp3_out)
                                .map_err(|e| Error::LameEncode(format!("{:?}", e)))
                        },
                    )?;
                }
            }
            bits => return Err(Error::UnsupportedIntBitDepth(bits)),
        },
    }

    mp3_out.reserve(mp3lame_encoder::max_required_buffer_size(0));
    encoder
        .flush_to_vec::<FlushNoGap>(&mut mp3_out)
        .map_err(|e| Error::LameFlush(format!("{:?}", e)))?;
    std::fs::write(mp3_path, &mp3_out)?;
    Ok(())
}

pub fn decode_to_wav(mp3_path: &Path, wav_path: &Path) -> Result<(), Error> {
    use hypr_audio_utils::Source;

    let source = hypr_audio_utils::source_from_path(mp3_path)?;
    let channels = source.channels();
    let sample_rate = source.sample_rate();

    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = hound::WavWriter::create(wav_path, spec)?;
    for sample in source {
        writer.write_sample(sample)?;
    }
    writer.finalize()?;
    Ok(())
}

fn f32_to_i16(sample: f32) -> i16 {
    let clamped = sample.clamp(-1.0, 1.0);
    (clamped * i16::MAX as f32) as i16
}

fn int_to_i16(sample: i32, bits_per_sample: u16) -> i16 {
    let max_amplitude = match bits_per_sample {
        0 | 1 => return 0,
        32.. => i32::MAX as f32,
        bits => ((1i64 << (bits - 1)) - 1) as f32,
    };
    f32_to_i16(sample as f32 / max_amplitude)
}

fn encode_mono_samples<S, I, F, E>(
    samples: I,
    mut sample_to_i16: F,
    mut encode_chunk: E,
) -> Result<(), Error>
where
    I: Iterator<Item = Result<S, hound::Error>>,
    F: FnMut(S) -> i16,
    E: FnMut(&[i16]) -> Result<usize, Error>,
{
    let mut pcm_i16 = Vec::with_capacity(CHUNK_FRAMES);
    for sample in samples {
        pcm_i16.push(sample_to_i16(sample?));
        if pcm_i16.len() < CHUNK_FRAMES {
            continue;
        }

        let _ = encode_chunk(&pcm_i16)?;
        pcm_i16.clear();
    }

    if !pcm_i16.is_empty() {
        let _ = encode_chunk(&pcm_i16)?;
    }

    Ok(())
}

fn encode_stereo_samples<S, I, F, E>(
    mut samples: I,
    mut sample_to_i16: F,
    mut encode_chunk: E,
) -> Result<(), Error>
where
    I: Iterator<Item = Result<S, hound::Error>>,
    F: FnMut(S) -> i16,
    E: FnMut(&[i16], &[i16]) -> Result<usize, Error>,
{
    let mut left = Vec::with_capacity(CHUNK_FRAMES);
    let mut right = Vec::with_capacity(CHUNK_FRAMES);

    loop {
        let Some(left_sample) = samples.next() else {
            break;
        };
        left.push(sample_to_i16(left_sample?));

        match samples.next() {
            Some(right_sample) => right.push(sample_to_i16(right_sample?)),
            None => right.push(0i16),
        }

        if left.len() < CHUNK_FRAMES {
            continue;
        }

        let _ = encode_chunk(&left, &right)?;
        left.clear();
        right.clear();
    }

    if !left.is_empty() {
        let _ = encode_chunk(&left, &right)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f32_to_i16_clamps_out_of_range_values() {
        assert_eq!(f32_to_i16(-2.0), -i16::MAX);
        assert_eq!(f32_to_i16(2.0), i16::MAX);
    }

    #[test]
    fn int_to_i16_scales_32_bit_extremes() {
        assert_eq!(int_to_i16(i32::MAX, 32), i16::MAX);
        assert_eq!(int_to_i16(i32::MIN, 32), -i16::MAX);
    }

    #[test]
    fn int_to_i16_handles_single_bit_depth() {
        assert_eq!(int_to_i16(1, 1), 0);
    }

    #[test]
    fn encode_mono_samples_flushes_partial_tail() -> Result<(), Error> {
        let samples = (0..(CHUNK_FRAMES + 1))
            .map(|n| Ok(n as i16))
            .collect::<Vec<_>>()
            .into_iter();
        let mut chunk_sizes = Vec::new();

        encode_mono_samples(
            samples,
            |sample| sample,
            |chunk| {
                chunk_sizes.push(chunk.len());
                Ok(0)
            },
        )?;

        assert_eq!(chunk_sizes, vec![CHUNK_FRAMES, 1]);
        Ok(())
    }

    #[test]
    fn encode_stereo_samples_pads_missing_right_sample() -> Result<(), Error> {
        let samples = vec![Ok(10i16), Ok(20i16), Ok(30i16)].into_iter();
        let mut encoded = Vec::new();

        encode_stereo_samples(
            samples,
            |sample| sample,
            |left, right| {
                encoded.push((left.to_vec(), right.to_vec()));
                Ok(0)
            },
        )?;

        assert_eq!(encoded.len(), 1);
        assert_eq!(encoded[0].0, vec![10, 30]);
        assert_eq!(encoded[0].1, vec![20, 0]);
        Ok(())
    }
}
