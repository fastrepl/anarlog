use anlg_transcribe_core::TARGET_SAMPLE_RATE;

use super::{BatchParams, BatchProvider};
use crate::BatchRuntime;

pub(super) struct NeverCancelled;

impl BatchRuntime for NeverCancelled {
    fn emit(&self, _event: crate::BatchEvent) {}

    fn is_cancelled(&self) -> bool {
        false
    }
}

pub(super) fn params(provider: BatchProvider, base_url: &str, file_path: &str) -> BatchParams {
    BatchParams {
        session_id: "s".to_string(),
        provider,
        file_path: file_path.to_string(),
        model: None,
        base_url: base_url.to_string(),
        api_key: String::new(),
        languages: vec![],
        keywords: vec![],
        num_speakers: None,
        min_speakers: None,
        max_speakers: None,
        known_speakers: vec![],
    }
}

#[derive(serde::Deserialize)]
pub(super) struct Turn {
    pub(super) start: u64,
    pub(super) end: u64,
    pub(super) speaker: String,
}

pub(super) fn english_1_turns() -> Vec<Turn> {
    serde_json::from_str(anlg_data::english_1::DIARIZATION_JSON).unwrap()
}

/// The fixture stores 500 ms slices; join adjacent slices of one speaker into
/// whole turns.
pub(super) fn english_1_coalesced_turns() -> Vec<Turn> {
    let mut turns: Vec<Turn> = Vec::new();
    for slice in english_1_turns() {
        match turns.last_mut() {
            Some(last) if last.speaker == slice.speaker && last.end >= slice.start => {
                last.end = last.end.max(slice.end);
            }
            _ => turns.push(slice),
        }
    }
    turns
}

/// Splits the two-speaker `english_1` fixture into a stereo file with one
/// speaker per channel, the shape of a mic + system-audio capture.
pub(super) fn stereo_fixture() -> tempfile::NamedTempFile {
    let mono: Vec<f32> = anlg_data::english_1::AUDIO
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0)
        .collect();
    let rate = TARGET_SAMPLE_RATE as usize;
    let mut left = vec![0.0f32; mono.len()];
    let mut right = vec![0.0f32; mono.len()];
    for turn in english_1_turns() {
        let start = (turn.start as usize * rate / 1000).min(mono.len());
        let end = (turn.end as usize * rate / 1000).min(mono.len());
        let target = if turn.speaker == "speaker0" {
            &mut left
        } else {
            &mut right
        };
        target[start..end].copy_from_slice(&mono[start..end]);
    }

    let file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    let mut writer = hound::WavWriter::create(
        file.path(),
        hound::WavSpec {
            channels: 2,
            sample_rate: TARGET_SAMPLE_RATE,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .unwrap();
    for (l, r) in left.iter().zip(&right) {
        writer.write_sample(*l).unwrap();
        writer.write_sample(*r).unwrap();
    }
    writer.finalize().unwrap();
    file
}
