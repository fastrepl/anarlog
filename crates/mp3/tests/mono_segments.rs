mod common;

use std::time::Duration;

use common::{Case, TestResult, decode_mp3_bytes, write_fixture_wav};
use mp3::encode_mono_segments;
use tempfile::tempdir;

#[test]
fn splits_stereo_audio_into_mono_segments() -> TestResult {
    let dir = tempdir()?;
    let source = dir.path().join("source.wav");
    let case = Case {
        channels: 2,
        frames: 16_000 * 5,
        sample_rate: 16_000,
    };
    write_fixture_wav(&source, case)?;

    let segments = encode_mono_segments(&source, dir.path(), Duration::from_secs(2))?;

    assert_eq!(segments.len(), 3, "5s of audio in 2s segments");
    for segment in &segments {
        let (spec, samples) = decode_mp3_bytes(&std::fs::read(segment)?)?;
        assert_eq!(spec.channels, 1, "segments are downmixed to mono");
        assert_eq!(spec.sample_rate, case.sample_rate);
        assert!(!samples.is_empty(), "segment decoded to no audio");
    }

    Ok(())
}

#[test]
fn keeps_short_audio_in_a_single_segment() -> TestResult {
    let dir = tempdir()?;
    let source = dir.path().join("source.wav");
    write_fixture_wav(
        &source,
        Case {
            channels: 1,
            frames: 16_000,
            sample_rate: 16_000,
        },
    )?;

    let segments = encode_mono_segments(&source, dir.path(), Duration::from_secs(600))?;

    assert_eq!(segments.len(), 1);
    Ok(())
}
