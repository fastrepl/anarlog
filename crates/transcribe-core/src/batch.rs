use hypr_vad_chunking::AudioChunk;
use owhisper_interface::batch_sse::BatchSseMessage;
use owhisper_interface::progress::{InferencePhase, InferenceProgress};
use tokio::sync::mpsc;

pub fn initial_resolved_until(chunks: &[AudioChunk], channel_duration: f64) -> f64 {
    chunks
        .first()
        .map(|chunk| chunk.start_timestamp_ms as f64 / 1000.0)
        .unwrap_or(channel_duration)
}

pub fn next_resolved_until(chunks: &[AudioChunk], chunk_idx: usize, channel_duration: f64) -> f64 {
    chunks
        .get(chunk_idx + 1)
        .map(|chunk| chunk.start_timestamp_ms as f64 / 1000.0)
        .unwrap_or(channel_duration)
}

pub fn overall_resolved_audio(resolved_until: &[f64]) -> f64 {
    let count = resolved_until.len() as f64;
    if count == 0.0 {
        return 0.0;
    }

    resolved_until.iter().copied().sum::<f64>() / count
}

pub fn overall_resolved_with_channel(
    resolved_until: &[f64],
    channel_idx: usize,
    resolved: f64,
) -> f64 {
    let count = resolved_until.len() as f64;
    if count == 0.0 {
        return resolved;
    }

    resolved_until
        .iter()
        .enumerate()
        .map(|(idx, value)| if idx == channel_idx { resolved } else { *value })
        .sum::<f64>()
        / count
}

pub fn record_progress(resolved_audio: f64, total_duration: f64, last_progress: &mut f64) -> f64 {
    let raw = if total_duration > 0.0 {
        (resolved_audio / total_duration).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let progress = raw.max(*last_progress).min(0.99);
    *last_progress = progress;
    progress
}

pub struct ProgressTracker {
    resolved_until: Vec<f64>,
    total_duration: f64,
    last_progress: f64,
    event_tx: Option<mpsc::UnboundedSender<BatchSseMessage>>,
}

impl ProgressTracker {
    pub fn new(
        resolved_until: Vec<f64>,
        total_duration: f64,
        event_tx: Option<mpsc::UnboundedSender<BatchSseMessage>>,
    ) -> Self {
        Self {
            resolved_until,
            total_duration,
            last_progress: 0.0,
            event_tx,
        }
    }

    pub fn update_channel(&mut self, channel_idx: usize, resolved: f64) {
        self.resolved_until[channel_idx] = resolved;
    }

    pub fn emit(&mut self, partial_text: Option<String>) {
        let Some(ref tx) = self.event_tx else { return };
        let resolved_audio = overall_resolved_audio(&self.resolved_until);
        self.emit_inner(tx.clone(), resolved_audio, partial_text);
    }

    pub fn emit_for_channel(
        &mut self,
        channel_idx: usize,
        resolved: f64,
        partial_text: Option<String>,
    ) {
        let Some(ref tx) = self.event_tx else { return };
        let overall = overall_resolved_with_channel(&self.resolved_until, channel_idx, resolved);
        self.emit_inner(tx.clone(), overall, partial_text);
    }

    pub fn has_tx(&self) -> bool {
        self.event_tx.is_some()
    }

    pub fn event_tx(&self) -> Option<&mpsc::UnboundedSender<BatchSseMessage>> {
        self.event_tx.as_ref()
    }

    fn emit_inner(
        &mut self,
        tx: mpsc::UnboundedSender<BatchSseMessage>,
        resolved_audio: f64,
        partial_text: Option<String>,
    ) {
        let previous = self.last_progress;
        let percentage =
            record_progress(resolved_audio, self.total_duration, &mut self.last_progress);
        if percentage <= previous {
            return;
        }

        let _ = tx.send(BatchSseMessage::Progress {
            progress: InferenceProgress {
                percentage,
                partial_text,
                phase: InferencePhase::Decoding,
            },
        });
    }
}
