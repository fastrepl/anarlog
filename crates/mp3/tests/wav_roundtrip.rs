mod common;

use mp3::{decode_to_wav, encode_wav};
use tempfile::tempdir;

use crate::common::{
    Case, TestResult, assert_decoded_matches_case, assert_samples_valid, read_wav,
    write_fixture_wav, write_fixture_wav_int,
};

fn assert_float_wav_roundtrip(case: Case) -> TestResult {
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
    assert_decoded_matches_case(
        case,
        original_samples.len(),
        &decoded_spec,
        &decoded_samples,
    );
    Ok(())
}

fn assert_pcm_wav_roundtrip(case: Case, bits_per_sample: u16) -> TestResult {
    let tempdir = tempdir()?;
    let wav_path = tempdir.path().join("input.wav");
    let mp3_path = tempdir.path().join("encoded.mp3");
    let decoded_wav_path = tempdir.path().join("decoded.wav");

    write_fixture_wav_int(&wav_path, case, bits_per_sample)?;
    encode_wav(&wav_path, &mp3_path)?;
    decode_to_wav(&mp3_path, &decoded_wav_path)?;

    let (decoded_spec, decoded_samples) = read_wav(&decoded_wav_path)?;
    assert_eq!(
        decoded_spec.channels, case.channels,
        "channel count changed"
    );
    assert_eq!(
        decoded_spec.sample_rate, case.sample_rate,
        "sample rate changed"
    );
    assert!(!decoded_samples.is_empty(), "decoded output is empty");
    assert_samples_valid(&decoded_samples);
    Ok(())
}

#[test]
fn float_wav_roundtrips_across_chunk_boundaries() {
    let cases = [
        Case {
            channels: 1,
            frames: 0,
            sample_rate: 16_000,
        },
        Case {
            channels: 1,
            frames: 1,
            sample_rate: 16_000,
        },
        Case {
            channels: 1,
            frames: 4_096,
            sample_rate: 16_000,
        },
        Case {
            channels: 1,
            frames: 4_097,
            sample_rate: 16_000,
        },
        Case {
            channels: 1,
            frames: 12_345,
            sample_rate: 16_000,
        },
        Case {
            channels: 2,
            frames: 0,
            sample_rate: 48_000,
        },
        Case {
            channels: 2,
            frames: 1,
            sample_rate: 48_000,
        },
        Case {
            channels: 2,
            frames: 4_096,
            sample_rate: 48_000,
        },
        Case {
            channels: 2,
            frames: 4_097,
            sample_rate: 48_000,
        },
        Case {
            channels: 2,
            frames: 11_111,
            sample_rate: 48_000,
        },
    ];

    for case in cases {
        assert_float_wav_roundtrip(case).unwrap_or_else(|error| panic!("{case:?}: {error}"));
    }
}

#[test]
fn pcm_wav_roundtrips_each_bit_depth() {
    let cases = [
        (
            8,
            Case {
                channels: 1,
                frames: 4_096,
                sample_rate: 16_000,
            },
        ),
        (
            16,
            Case {
                channels: 2,
                frames: 8_192,
                sample_rate: 44_100,
            },
        ),
        (
            24,
            Case {
                channels: 1,
                frames: 8_192,
                sample_rate: 22_050,
            },
        ),
        (
            32,
            Case {
                channels: 2,
                frames: 6_321,
                sample_rate: 48_000,
            },
        ),
    ];

    for (bits_per_sample, case) in cases {
        assert_pcm_wav_roundtrip(case, bits_per_sample)
            .unwrap_or_else(|error| panic!("{bits_per_sample}-bit {case:?}: {error}"));
    }
}
