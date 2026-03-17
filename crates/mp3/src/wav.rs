use std::fs::File;
use std::io::{BufWriter, Seek, Write};
use std::path::Path;

use hound::SampleFormat;

use crate::encoder::{f32_to_i16, int_to_i16};
use crate::{Error, MonoStreamEncoder, StereoStreamEncoder};

const CHUNK_FRAMES: usize = 4096;

pub fn concat_files(paths: &[&Path], output: &Path) -> Result<(), Error> {
    let file = File::create(output)?;
    let mut writer = BufWriter::new(file);

    for path in paths {
        let bytes = std::fs::read(path)?;
        writer.write_all(&bytes)?;
    }

    writer.flush()?;
    Ok(())
}

pub fn encode_wav(wav_path: &Path, mp3_path: &Path) -> Result<(), Error> {
    let mut reader = hound::WavReader::open(wav_path)?;
    let spec = reader.spec();
    let mut mp3_out = Vec::new();

    match spec.channels {
        1 => encode_mono_wav(&mut reader, spec, &mut mp3_out)?,
        2 => encode_stereo_wav(&mut reader, spec, &mut mp3_out)?,
        count => return Err(Error::UnsupportedChannelCount(count)),
    }

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

fn encode_mono_wav<R: std::io::Read + Seek>(
    reader: &mut hound::WavReader<R>,
    spec: hound::WavSpec,
    mp3_out: &mut Vec<u8>,
) -> Result<(), Error> {
    let mut encoder = MonoStreamEncoder::new(spec.sample_rate)?;

    match spec.sample_format {
        SampleFormat::Float => {
            if spec.bits_per_sample != 32 {
                return Err(Error::UnsupportedFloatBitDepth(spec.bits_per_sample));
            }

            encode_mono_samples(reader.samples::<f32>(), f32_to_i16, |chunk| {
                encoder.encode_i16(chunk, mp3_out)
            })?;
        }
        SampleFormat::Int => match spec.bits_per_sample {
            1..=8 => encode_mono_samples(
                reader.samples::<i8>(),
                |sample| int_to_i16(sample as i32, spec.bits_per_sample),
                |chunk| encoder.encode_i16(chunk, mp3_out),
            )?,
            9..=16 => encode_mono_samples(
                reader.samples::<i16>(),
                |sample| int_to_i16(sample as i32, spec.bits_per_sample),
                |chunk| encoder.encode_i16(chunk, mp3_out),
            )?,
            17..=32 => encode_mono_samples(
                reader.samples::<i32>(),
                |sample| int_to_i16(sample, spec.bits_per_sample),
                |chunk| encoder.encode_i16(chunk, mp3_out),
            )?,
            bits => return Err(Error::UnsupportedIntBitDepth(bits)),
        },
    }

    encoder.flush(mp3_out)?;
    Ok(())
}

fn encode_stereo_wav<R: std::io::Read + Seek>(
    reader: &mut hound::WavReader<R>,
    spec: hound::WavSpec,
    mp3_out: &mut Vec<u8>,
) -> Result<(), Error> {
    let mut encoder = StereoStreamEncoder::new(spec.sample_rate)?;

    match spec.sample_format {
        SampleFormat::Float => {
            if spec.bits_per_sample != 32 {
                return Err(Error::UnsupportedFloatBitDepth(spec.bits_per_sample));
            }

            encode_stereo_samples(reader.samples::<f32>(), f32_to_i16, |left, right| {
                encoder.encode_i16(left, right, mp3_out)
            })?;
        }
        SampleFormat::Int => match spec.bits_per_sample {
            1..=8 => encode_stereo_samples(
                reader.samples::<i8>(),
                |sample| int_to_i16(sample as i32, spec.bits_per_sample),
                |left, right| encoder.encode_i16(left, right, mp3_out),
            )?,
            9..=16 => encode_stereo_samples(
                reader.samples::<i16>(),
                |sample| int_to_i16(sample as i32, spec.bits_per_sample),
                |left, right| encoder.encode_i16(left, right, mp3_out),
            )?,
            17..=32 => encode_stereo_samples(
                reader.samples::<i32>(),
                |sample| int_to_i16(sample, spec.bits_per_sample),
                |left, right| encoder.encode_i16(left, right, mp3_out),
            )?,
            bits => return Err(Error::UnsupportedIntBitDepth(bits)),
        },
    }

    encoder.flush(mp3_out)?;
    Ok(())
}

fn encode_mono_samples<S, I, F, E>(
    samples: I,
    mut sample_to_i16: F,
    mut encode_chunk: E,
) -> Result<(), Error>
where
    I: Iterator<Item = Result<S, hound::Error>>,
    F: FnMut(S) -> i16,
    E: FnMut(&[i16]) -> Result<(), Error>,
{
    let mut pcm_i16 = Vec::with_capacity(CHUNK_FRAMES);
    for sample in samples {
        pcm_i16.push(sample_to_i16(sample?));
        if pcm_i16.len() < CHUNK_FRAMES {
            continue;
        }

        encode_chunk(&pcm_i16)?;
        pcm_i16.clear();
    }

    if !pcm_i16.is_empty() {
        encode_chunk(&pcm_i16)?;
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
    E: FnMut(&[i16], &[i16]) -> Result<(), Error>,
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

        encode_chunk(&left, &right)?;
        left.clear();
        right.clear();
    }

    if !left.is_empty() {
        encode_chunk(&left, &right)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

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
                Ok(())
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
                Ok(())
            },
        )?;

        assert_eq!(encoded.len(), 1);
        assert_eq!(encoded[0].0, vec![10, 30]);
        assert_eq!(encoded[0].1, vec![20, 0]);
        Ok(())
    }

    #[test]
    fn concat_files_joins_bytes_in_order() -> Result<(), Error> {
        let dir = tempdir()?;
        let first = dir.path().join("a.mp3");
        let second = dir.path().join("b.mp3");
        let output = dir.path().join("out.mp3");

        std::fs::write(&first, [1u8, 2, 3])?;
        std::fs::write(&second, [4u8, 5, 6])?;

        concat_files(&[&first, &second], &output)?;

        assert_eq!(std::fs::read(output)?, vec![1, 2, 3, 4, 5, 6]);
        Ok(())
    }
}
