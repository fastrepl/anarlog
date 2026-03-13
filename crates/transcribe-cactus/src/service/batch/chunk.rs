use std::time::Duration;

use futures_util::StreamExt;
use hypr_vad_chunking::{AudioChunk, VadExt};
use rodio::buffer::SamplesBuffer;

pub(super) const TARGET_SAMPLE_RATE: u32 = 16000;
const VAD_REDEMPTION_TIME: Duration = Duration::from_millis(150);
const MAX_CHUNK_SAMPLES: usize = TARGET_SAMPLE_RATE as usize * 25;

pub(super) async fn chunk_mono_audio(mono: &[f32]) -> Result<Vec<AudioChunk>, crate::Error> {
    let source = SamplesBuffer::new(1, TARGET_SAMPLE_RATE, mono.to_vec());

    let vad_chunks = source
        .speech_chunks(VAD_REDEMPTION_TIME)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    let mut chunks = Vec::new();
    for chunk in vad_chunks {
        if chunk.samples.len() <= MAX_CHUNK_SAMPLES {
            chunks.push(chunk);
        } else {
            for (i, window) in chunk.samples.chunks(MAX_CHUNK_SAMPLES).enumerate() {
                let start_ms = chunk.start_timestamp_ms
                    + i * MAX_CHUNK_SAMPLES * 1000 / TARGET_SAMPLE_RATE as usize;
                let end_ms = start_ms + window.len() * 1000 / TARGET_SAMPLE_RATE as usize;
                chunks.push(AudioChunk {
                    samples: window.to_vec(),
                    start_timestamp_ms: start_ms,
                    end_timestamp_ms: end_ms,
                });
            }
        }
    }

    tracing::info!(
        chunk_count = chunks.len(),
        chunk_durations_ms = ?chunks.iter().map(|c| c.end_timestamp_ms - c.start_timestamp_ms).collect::<Vec<_>>(),
        "vad chunking complete"
    );

    Ok(chunks)
}
