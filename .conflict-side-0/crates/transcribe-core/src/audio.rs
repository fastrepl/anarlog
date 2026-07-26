use std::time::Duration;

use hypr_audio_chunking::{AudioChunk, Chunker, SpeechChunker, SpeechChunkingConfig};

pub const TARGET_SAMPLE_RATE: u32 = 16_000;

const DEFAULT_SPEECH_REDEMPTION_TIME: Duration = Duration::from_millis(150);
const MAX_CHUNK_SAMPLES: usize = TARGET_SAMPLE_RATE as usize * 25;

pub fn chunk_channel_audio<E>(samples: &[f32]) -> Result<Vec<AudioChunk>, E>
where
    E: From<hypr_audio_chunking::Error>,
{
    let mut chunker =
        SpeechChunker::new(SpeechChunkingConfig::speech(DEFAULT_SPEECH_REDEMPTION_TIME))?;
    Ok(chunk_channel_audio_with(samples, &mut chunker)?)
}

fn chunk_channel_audio_with<C>(
    samples: &[f32],
    chunker: &mut C,
) -> Result<Vec<AudioChunk>, C::Error>
where
    C: Chunker,
{
    let chunks = chunker.chunk(samples, TARGET_SAMPLE_RATE)?;
    let mut normalized = Vec::new();

    for chunk in chunks {
        if chunk.samples.len() <= MAX_CHUNK_SAMPLES {
            normalized.push(chunk);
            continue;
        }

        for (index, window) in chunk.samples.chunks(MAX_CHUNK_SAMPLES).enumerate() {
            let sample_start = chunk.sample_start + index * MAX_CHUNK_SAMPLES;
            let sample_end = sample_start + window.len();
            normalized.push(AudioChunk {
                samples: window.to_vec(),
                sample_start,
                sample_end,
            });
        }
    }

    tracing::info!(
        chunk_count = normalized.len(),
        chunk_durations_ms = ?normalized
            .iter()
            .map(|chunk| (chunk.sample_end - chunk.sample_start) * 1000 / TARGET_SAMPLE_RATE as usize)
            .collect::<Vec<_>>(),
        "audio_chunking_complete"
    );

    Ok(normalized)
}

pub fn split_resampled_channels(samples: &[f32], channel_count: usize) -> Vec<Vec<f32>> {
    if channel_count <= 1 {
        return vec![samples.to_vec()];
    }

    hypr_audio_utils::deinterleave(samples, channel_count)
}

pub fn channel_duration_sec(samples: &[f32]) -> f64 {
    samples.len() as f64 / TARGET_SAMPLE_RATE as f64
}

#[cfg(test)]
mod tests {
    use hypr_audio_chunking::{AudioChunk, Chunker};

    use super::*;
    use crate::{initial_resolved_until, next_resolved_until};

    struct FakeChunker {
        chunks: Vec<AudioChunk>,
    }

    impl Chunker for FakeChunker {
        type Error = std::convert::Infallible;

        fn chunk(
            &mut self,
            _samples: &[f32],
            _sample_rate: u32,
        ) -> Result<Vec<AudioChunk>, Self::Error> {
            Ok(self.chunks.clone())
        }
    }

    #[test]
    fn empty_audio_marks_channel_complete() {
        let chunks = chunk_channel_audio::<hypr_audio_chunking::Error>(&[]).unwrap();

        assert!(chunks.is_empty());
        assert_eq!(initial_resolved_until(&chunks, 40.0), 40.0);
    }

    #[test]
    fn empty_chunk_lists_mark_channel_complete() {
        let mut chunker = FakeChunker { chunks: Vec::new() };
        let chunks = chunk_channel_audio_with(&[], &mut chunker).unwrap();

        assert!(chunks.is_empty());
        assert_eq!(initial_resolved_until(&chunks, 40.0), 40.0);
    }

    #[test]
    fn leading_silence_uses_sample_offsets() {
        let mut chunker = FakeChunker {
            chunks: vec![AudioChunk {
                samples: vec![0.0; TARGET_SAMPLE_RATE as usize * 3],
                sample_start: TARGET_SAMPLE_RATE as usize * 12,
                sample_end: TARGET_SAMPLE_RATE as usize * 15,
            }],
        };
        let chunks =
            chunk_channel_audio_with(&vec![0.0; TARGET_SAMPLE_RATE as usize * 15], &mut chunker)
                .unwrap();

        assert_eq!(initial_resolved_until(&chunks, 40.0), 12.0);
    }

    #[test]
    fn oversized_chunks_are_split_at_generic_limit() {
        let oversized = MAX_CHUNK_SAMPLES + TARGET_SAMPLE_RATE as usize;
        let mut chunker = FakeChunker {
            chunks: vec![AudioChunk {
                samples: vec![1.0; oversized],
                sample_start: TARGET_SAMPLE_RATE as usize * 2,
                sample_end: TARGET_SAMPLE_RATE as usize * 2 + oversized,
            }],
        };

        let chunks = chunk_channel_audio_with(&vec![0.0; oversized], &mut chunker).unwrap();

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].sample_start, TARGET_SAMPLE_RATE as usize * 2);
        assert_eq!(
            chunks[0].sample_end,
            TARGET_SAMPLE_RATE as usize * 2 + MAX_CHUNK_SAMPLES
        );
        assert_eq!(chunks[1].sample_start, chunks[0].sample_end);
        assert_eq!(chunks[1].samples.len(), TARGET_SAMPLE_RATE as usize);
    }

    #[test]
    fn resolved_progress_uses_sample_offsets() {
        let mut chunker = FakeChunker {
            chunks: vec![
                AudioChunk {
                    samples: vec![0.0; TARGET_SAMPLE_RATE as usize * 2],
                    sample_start: TARGET_SAMPLE_RATE as usize * 4,
                    sample_end: TARGET_SAMPLE_RATE as usize * 6,
                },
                AudioChunk {
                    samples: vec![0.0; TARGET_SAMPLE_RATE as usize * 3],
                    sample_start: TARGET_SAMPLE_RATE as usize * 8,
                    sample_end: TARGET_SAMPLE_RATE as usize * 11,
                },
            ],
        };

        let chunks =
            chunk_channel_audio_with(&vec![0.0; TARGET_SAMPLE_RATE as usize * 11], &mut chunker)
                .unwrap();

        assert_eq!(initial_resolved_until(&chunks, 20.0), 4.0);
        assert_eq!(next_resolved_until(&chunks, 0, 20.0), 8.0);
        assert_eq!(next_resolved_until(&chunks, 1, 20.0), 20.0);
    }
}
