use std::num::NonZero;
use std::time::Duration;

use futures_util::StreamExt;
use hypr_vad_chunking::{AudioChunk, VadExt};
use rodio::buffer::SamplesBuffer;
use rodio::nz;

pub const TARGET_SAMPLE_RATE: u32 = 16_000;

const VAD_REDEMPTION_TIME: Duration = Duration::from_millis(150);
const MAX_CHUNK_SAMPLES: usize = TARGET_SAMPLE_RATE as usize * 25;

pub async fn chunk_mono_audio<E>(mono: &[f32]) -> Result<Vec<AudioChunk>, E>
where
    E: From<hypr_vad_chunking::Error>,
{
    let source = SamplesBuffer::new(
        nz!(1u16),
        NonZero::new(TARGET_SAMPLE_RATE).unwrap(),
        mono.to_vec(),
    );

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
            continue;
        }

        for (index, window) in chunk.samples.chunks(MAX_CHUNK_SAMPLES).enumerate() {
            let start_ms = chunk.start_timestamp_ms
                + index * MAX_CHUNK_SAMPLES * 1000 / TARGET_SAMPLE_RATE as usize;
            let end_ms = start_ms + window.len() * 1000 / TARGET_SAMPLE_RATE as usize;
            chunks.push(AudioChunk {
                samples: window.to_vec(),
                start_timestamp_ms: start_ms,
                end_timestamp_ms: end_ms,
            });
        }
    }

    tracing::info!(
        chunk_count = chunks.len(),
        chunk_durations_ms = ?chunks.iter().map(|chunk| chunk.end_timestamp_ms - chunk.start_timestamp_ms).collect::<Vec<_>>(),
        "vad_chunking_complete"
    );

    Ok(chunks)
}

pub fn chunk_channel_audio<E>(samples: &[f32]) -> Result<Vec<AudioChunk>, E>
where
    E: From<std::io::Error> + From<hypr_vad_chunking::Error>,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.block_on(chunk_mono_audio(samples)),
        Err(_) => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(chunk_mono_audio(samples)),
    }
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
