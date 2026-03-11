#![allow(dead_code)]

use std::f32::consts::PI;
use std::path::Path;

use mp3::decode_to_wav;
use tempfile::tempdir;

pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Copy)]
pub struct Case {
    pub channels: u16,
    pub frames: usize,
    pub sample_rate: u32,
}

pub fn fixture_sample(frame_index: usize, channel_index: usize) -> f32 {
    let t = frame_index as f32 * 0.013 + channel_index as f32 * 0.17;
    let wave = (2.0 * PI * t).sin() * 0.6;
    let harmonic = (2.0 * PI * (t * 0.5)).cos() * 0.3;
    (wave + harmonic).clamp(-1.0, 1.0)
}

pub fn fixture_channel_f32(frames: usize, channel_index: usize) -> Vec<f32> {
    (0..frames)
        .map(|frame| fixture_sample(frame, channel_index))
        .collect()
}

pub fn fixture_channel_i16(frames: usize, channel_index: usize) -> Vec<i16> {
    fixture_channel_f32(frames, channel_index)
        .into_iter()
        .map(|sample| (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
        .collect()
}

pub fn write_fixture_wav(path: &Path, case: Case) -> TestResult<Vec<f32>> {
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

pub fn write_fixture_wav_int(path: &Path, case: Case, bits_per_sample: u16) -> TestResult {
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

pub fn write_malformed_stereo_wav_with_odd_samples(path: &Path) -> TestResult {
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

pub fn read_wav(path: &Path) -> TestResult<(hound::WavSpec, Vec<f32>)> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let samples = reader.samples::<f32>().collect::<Result<Vec<_>, _>>()?;
    Ok((spec, samples))
}

pub fn decode_mp3_bytes(bytes: &[u8]) -> TestResult<(hound::WavSpec, Vec<f32>)> {
    let dir = tempdir()?;
    let mp3_path = dir.path().join("encoded.mp3");
    let wav_path = dir.path().join("decoded.wav");

    std::fs::write(&mp3_path, bytes)?;
    decode_to_wav(&mp3_path, &wav_path)?;
    read_wav(&wav_path)
}

pub fn assert_decoded_matches_case(
    case: Case,
    expected_len: usize,
    decoded_spec: &hound::WavSpec,
    decoded_samples: &[f32],
) {
    assert_eq!(
        decoded_spec.channels, case.channels,
        "channel count changed"
    );
    assert_eq!(
        decoded_spec.sample_rate, case.sample_rate,
        "sample rate changed"
    );

    assert_samples_valid(decoded_samples);

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
        return;
    }

    assert!(
        !decoded_samples.is_empty(),
        "non-empty input decoded to empty output"
    );
    let delta = expected_len.abs_diff(decoded_samples.len());
    let tolerance = 4096 * case.channels as usize;
    assert!(
        delta <= tolerance,
        "decoded length drift too large: expected {expected_len}, got {}, delta {delta}, tolerance {tolerance}",
        decoded_samples.len()
    );
}

pub fn assert_samples_valid(samples: &[f32]) {
    for sample in samples {
        assert!(sample.is_finite(), "decoded sample is not finite");
        assert!(
            (-1.1..=1.1).contains(sample),
            "decoded sample out of expected range: {sample}"
        );
    }
}
