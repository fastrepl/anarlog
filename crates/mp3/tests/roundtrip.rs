use std::f32::consts::PI;
use std::path::Path;

use mp3::{decode_to_wav, encode_wav};
use tempfile::tempdir;

#[derive(Clone, Copy)]
struct Case {
    channels: u16,
    frames: usize,
    sample_rate: u32,
}

fn fixture_sample(frame_index: usize, channel_index: usize) -> f32 {
    let t = frame_index as f32 * 0.013 + channel_index as f32 * 0.17;
    let wave = (2.0 * PI * t).sin() * 0.6;
    let harmonic = (2.0 * PI * (t * 0.5)).cos() * 0.3;
    (wave + harmonic).clamp(-1.0, 1.0)
}

fn write_fixture_wav(path: &Path, case: Case) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let spec = hound::WavSpec {
        channels: case.channels,
        sample_rate: case.sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = hound::WavWriter::create(path, spec)?;
    let mut samples = Vec::with_capacity(case.frames * case.channels as usize);
    for frame in 0..case.frames {
        for channel in 0..case.channels as usize {
            let sample = fixture_sample(frame, channel);
            writer.write_sample(sample)?;
            samples.push(sample);
        }
    }
    writer.finalize()?;
    Ok(samples)
}

fn write_fixture_wav_int(
    path: &Path,
    case: Case,
    bits_per_sample: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let spec = hound::WavSpec {
        channels: case.channels,
        sample_rate: case.sample_rate,
        bits_per_sample,
        sample_format: hound::SampleFormat::Int,
    };

    let max_amplitude = match bits_per_sample {
        8 => i8::MAX as f32,
        16 => i16::MAX as f32,
        17..=31 => ((1i64 << (bits_per_sample - 1)) - 1) as f32,
        32 => i32::MAX as f32,
        bits => return Err(format!("unsupported bit depth: {bits}").into()),
    };

    let mut writer = hound::WavWriter::create(path, spec)?;
    for frame in 0..case.frames {
        for channel in 0..case.channels as usize {
            let sample = fixture_sample(frame, channel);
            writer.write_sample((sample * max_amplitude) as i32)?;
        }
    }
    writer.finalize()?;
    Ok(())
}

fn write_malformed_stereo_wav_with_odd_samples(
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let sample_rate = 44_100u32;
    let channels = 2u16;
    let bits_per_sample = 16u16;
    let block_align = channels * (bits_per_sample / 8);
    let byte_rate = sample_rate * u32::from(block_align);
    let samples = [0i16, i16::MAX, i16::MIN];
    let data_size = (samples.len() * std::mem::size_of::<i16>()) as u32;
    let riff_chunk_size = 4 + (8 + 16) + (8 + data_size);

    let mut bytes = Vec::with_capacity((riff_chunk_size + 8) as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&riff_chunk_size.to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&byte_rate.to_le_bytes());
    bytes.extend_from_slice(&block_align.to_le_bytes());
    bytes.extend_from_slice(&bits_per_sample.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_size.to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }

    std::fs::write(path, bytes)?;
    Ok(())
}

fn assert_samples_valid(samples: &[f32]) {
    for sample in samples {
        assert!(sample.is_finite(), "decoded sample is not finite");
        assert!(
            (-1.1..=1.1).contains(sample),
            "decoded sample out of expected range: {sample}"
        );
    }
}

fn read_wav(path: &Path) -> Result<(hound::WavSpec, Vec<f32>), Box<dyn std::error::Error>> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let samples = reader.samples::<f32>().collect::<Result<Vec<_>, _>>()?;
    Ok((spec, samples))
}

fn assert_roundtrip(case: Case) -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = tempdir()?;
    let wav_path = tempdir.path().join("input.wav");
    let mp3_path = tempdir.path().join("encoded.mp3");
    let decoded_wav_path = tempdir.path().join("decoded.wav");

    let original_samples = write_fixture_wav(&wav_path, case)?;
    encode_wav(&wav_path, &mp3_path)?;
    decode_to_wav(&mp3_path, &decoded_wav_path)?;

    assert!(mp3_path.exists(), "encoded mp3 was not created");
    let mp3_size = std::fs::metadata(&mp3_path)?.len();
    assert!(mp3_size > 0, "encoded mp3 is empty");

    let (decoded_spec, decoded_samples) = read_wav(&decoded_wav_path)?;
    assert_eq!(
        decoded_spec.channels, case.channels,
        "channel count changed"
    );
    assert_eq!(
        decoded_spec.sample_rate, case.sample_rate,
        "sample rate changed"
    );

    assert_samples_valid(&decoded_samples);

    if case.frames == 0 {
        let max_len = 4096 * case.channels as usize;
        let peak = decoded_samples
            .iter()
            .copied()
            .map(f32::abs)
            .fold(0.0_f32, f32::max);
        assert!(
            decoded_samples.len() <= max_len,
            "empty input decoded to unexpectedly large output: {} > {}",
            decoded_samples.len(),
            max_len
        );
        assert!(
            peak <= 0.01,
            "empty input decoded to non-silent output, peak amplitude: {peak}"
        );
    } else {
        assert!(
            !decoded_samples.is_empty(),
            "non-empty input decoded to empty output"
        );
        let expected_len = original_samples.len();
        let actual_len = decoded_samples.len();
        let delta = expected_len.abs_diff(actual_len);
        let tolerance = 4096 * case.channels as usize;
        assert!(
            delta <= tolerance,
            "decoded length drift too large: expected {expected_len}, got {actual_len}, delta {delta}, tolerance {tolerance}"
        );
    }

    Ok(())
}

macro_rules! roundtrip_cases {
    ($($name:ident => { channels: $channels:expr, frames: $frames:expr, sample_rate: $sample_rate:expr }),+ $(,)?) => {
        $(
            #[test]
            fn $name() -> Result<(), Box<dyn std::error::Error>> {
                assert_roundtrip(Case {
                    channels: $channels,
                    frames: $frames,
                    sample_rate: $sample_rate,
                })
            }
        )+
    };
}

roundtrip_cases! {
    mono_empty => { channels: 1, frames: 0, sample_rate: 16_000 },
    mono_single_frame => { channels: 1, frames: 1, sample_rate: 16_000 },
    mono_chunk_edge => { channels: 1, frames: 4_096, sample_rate: 16_000 },
    mono_chunk_plus_one => { channels: 1, frames: 4_097, sample_rate: 16_000 },
    mono_long => { channels: 1, frames: 12_345, sample_rate: 16_000 },
    stereo_empty => { channels: 2, frames: 0, sample_rate: 48_000 },
    stereo_single_frame => { channels: 2, frames: 1, sample_rate: 48_000 },
    stereo_chunk_edge => { channels: 2, frames: 4_096, sample_rate: 48_000 },
    stereo_chunk_plus_one => { channels: 2, frames: 4_097, sample_rate: 48_000 },
    stereo_long => { channels: 2, frames: 11_111, sample_rate: 48_000 },
}

#[test]
fn rejects_more_than_two_channels() -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = tempdir()?;
    let wav_path = tempdir.path().join("input_3ch.wav");
    let mp3_path = tempdir.path().join("encoded.mp3");

    write_fixture_wav(
        &wav_path,
        Case {
            channels: 3,
            frames: 128,
            sample_rate: 48_000,
        },
    )?;

    let err = encode_wav(&wav_path, &mp3_path).expect_err("3-channel input should be rejected");
    let message = err.to_string();
    assert!(
        message.contains("unsupported channel count"),
        "unexpected error message: {message}"
    );

    Ok(())
}

macro_rules! pcm_roundtrip_cases {
    ($($name:ident => { bits: $bits:expr, channels: $channels:expr, frames: $frames:expr, sample_rate: $sample_rate:expr }),+ $(,)?) => {
        $(
            #[test]
            fn $name() -> Result<(), Box<dyn std::error::Error>> {
                let tempdir = tempdir()?;
                let case = Case { channels: $channels, frames: $frames, sample_rate: $sample_rate };
                let wav_path = tempdir.path().join("input.wav");
                let mp3_path = tempdir.path().join("encoded.mp3");
                let decoded_wav_path = tempdir.path().join("decoded.wav");

                write_fixture_wav_int(&wav_path, case, $bits)?;
                encode_wav(&wav_path, &mp3_path)?;
                decode_to_wav(&mp3_path, &decoded_wav_path)?;

                let (decoded_spec, decoded_samples) = read_wav(&decoded_wav_path)?;
                assert_eq!(decoded_spec.channels, case.channels, "channel count changed");
                assert_eq!(decoded_spec.sample_rate, case.sample_rate, "sample rate changed");
                assert!(!decoded_samples.is_empty(), "decoded output is empty");
                assert_samples_valid(&decoded_samples);

                Ok(())
            }
        )+
    };
}

pcm_roundtrip_cases! {
    roundtrip_pcm8_mono    => { bits: 8,  channels: 1, frames: 4_096, sample_rate: 16_000 },
    roundtrip_pcm16_stereo => { bits: 16, channels: 2, frames: 8_192, sample_rate: 44_100 },
    roundtrip_pcm24_mono   => { bits: 24, channels: 1, frames: 8_192, sample_rate: 22_050 },
    roundtrip_pcm32_stereo => { bits: 32, channels: 2, frames: 6_321, sample_rate: 48_000 },
}

#[test]
fn encode_rejects_malformed_stereo_data() -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = tempdir()?;
    let wav_path = tempdir.path().join("odd_stereo.wav");
    let mp3_path = tempdir.path().join("encoded.mp3");

    write_malformed_stereo_wav_with_odd_samples(&wav_path)?;
    let err = encode_wav(&wav_path, &mp3_path)
        .expect_err("malformed stereo wav should be rejected before encoding");
    let message = err.to_string();
    assert!(
        message.contains("invalid data chunk length"),
        "unexpected malformed wav error: {message}"
    );

    Ok(())
}

#[test]
fn decode_rejects_invalid_mp3() -> Result<(), Box<dyn std::error::Error>> {
    let tempdir = tempdir()?;
    let invalid_mp3_path = tempdir.path().join("invalid.mp3");
    let wav_path = tempdir.path().join("decoded.wav");
    std::fs::write(&invalid_mp3_path, b"not an mp3")?;

    let result = decode_to_wav(&invalid_mp3_path, &wav_path);
    assert!(result.is_err(), "invalid mp3 should return an error");

    Ok(())
}
