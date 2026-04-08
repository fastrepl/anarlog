use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::Stream;
use pin_project::pin_project;

use crate::AudioChunk;

use super::{continuous::VadStreamItem, session::VadTransition};

const SAMPLE_RATE: usize = 16000;
const MIN_DETECTED_SPEECH_MS: u64 = 200;
const MAX_SHORT_CHUNK_MERGE_GAP_MS: usize = 250;

pub(crate) fn speech_chunks_from_transitions(
    transitions: Vec<VadTransition>,
) -> Vec<BufferedChunk> {
    transitions
        .into_iter()
        .filter_map(|transition| match transition {
            VadTransition::SpeechEnd {
                detected_speech_samples,
                sample_start,
                sample_end,
                samples,
            } => Some(BufferedChunk {
                chunk: AudioChunk {
                    samples,
                    sample_start,
                    sample_end,
                },
                detected_speech_samples,
            }),
            VadTransition::SpeechStart { sample_start } => {
                let _ = sample_start;
                None
            }
        })
        .collect()
}

pub(crate) fn normalize_speech_chunks(
    chunks: Vec<BufferedChunk>,
    redemption_time: Duration,
) -> Vec<AudioChunk> {
    let mut state = NormalizerState::new(redemption_time);
    let mut normalized = Vec::new();

    for chunk in chunks {
        state.push(chunk, &mut normalized);
    }

    if let Some(pending) = state.pending.take() {
        normalized.push(pending.chunk);
    }

    normalized
}

fn duration_to_samples(duration: Duration) -> usize {
    ((duration.as_millis() * SAMPLE_RATE as u128) / 1000) as usize
}

#[derive(Debug, Clone)]
pub(crate) struct BufferedChunk {
    chunk: AudioChunk,
    detected_speech_samples: usize,
}

impl BufferedChunk {
    fn is_short(&self, min_detected_speech_samples: usize) -> bool {
        self.detected_speech_samples < min_detected_speech_samples
    }

    fn gap_samples(&self, next: &Self) -> usize {
        next.chunk
            .sample_start
            .saturating_sub(self.chunk.sample_end)
    }

    fn merge(mut self, next: Self) -> Self {
        let gap_samples = self.gap_samples(&next);
        if gap_samples > 0 {
            self.chunk
                .samples
                .resize(self.chunk.samples.len() + gap_samples, 0.0);
        }

        self.chunk.samples.extend(next.chunk.samples);
        self.chunk.sample_end = next.chunk.sample_end;
        self.detected_speech_samples += next.detected_speech_samples;
        self
    }
}

struct NormalizerState {
    pending: Option<BufferedChunk>,
    min_detected_speech_samples: usize,
    merge_gap_samples: usize,
}

impl NormalizerState {
    fn new(redemption_time: Duration) -> Self {
        Self {
            pending: None,
            min_detected_speech_samples: duration_to_samples(Duration::from_millis(
                MIN_DETECTED_SPEECH_MS,
            )),
            merge_gap_samples: duration_to_samples(Duration::from_millis(
                (redemption_time.as_millis() as usize).clamp(100, MAX_SHORT_CHUNK_MERGE_GAP_MS)
                    as u64,
            )),
        }
    }

    fn push(&mut self, next: BufferedChunk, output: &mut Vec<AudioChunk>) {
        if let Some(pending) = self.pending.take() {
            if pending.gap_samples(&next) <= self.merge_gap_samples {
                let merged = pending.merge(next);
                if merged.is_short(self.min_detected_speech_samples) {
                    self.pending = Some(merged);
                    return;
                }

                output.push(merged.chunk);
                return;
            }

            self.pending = Some(next);
            output.push(pending.chunk);
            return;
        }

        if next.is_short(self.min_detected_speech_samples) {
            self.pending = Some(next);
            return;
        }

        output.push(next.chunk);
    }
}

#[pin_project]
pub(crate) struct SpeechChunkStream<S> {
    #[pin]
    inner: S,
    state: NormalizerState,
}

pub(crate) fn normalize_speech_chunk_stream<S>(
    inner: S,
    redemption_time: Duration,
) -> SpeechChunkStream<S> {
    SpeechChunkStream {
        inner,
        state: NormalizerState::new(redemption_time),
    }
}

impl<S> Stream for SpeechChunkStream<S>
where
    S: Stream<Item = Result<VadStreamItem, crate::Error>>,
{
    type Item = Result<AudioChunk, crate::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        loop {
            match this.inner.as_mut().poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Some(Ok(VadStreamItem::SpeechEnd {
                    detected_speech_samples,
                    sample_start,
                    sample_end,
                    samples,
                }))) => {
                    let next = BufferedChunk {
                        chunk: AudioChunk {
                            samples,
                            sample_start,
                            sample_end,
                        },
                        detected_speech_samples,
                    };
                    let mut output = Vec::new();
                    this.state.push(next, &mut output);
                    if let Some(chunk) = output.into_iter().next() {
                        return Poll::Ready(Some(Ok(chunk)));
                    }
                }
                Poll::Ready(Some(Ok(VadStreamItem::SpeechStart { sample_start }))) => {
                    let _ = sample_start;
                    continue;
                }
                Poll::Ready(Some(Ok(VadStreamItem::AudioSamples(_)))) => continue,
                Poll::Ready(Some(Err(e))) => {
                    this.state.pending = None;
                    return Poll::Ready(Some(Err(e)));
                }
                Poll::Ready(None) => {
                    if let Some(pending) = this.state.pending.take() {
                        return Poll::Ready(Some(Ok(pending.chunk)));
                    }
                    return Poll::Ready(None);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use futures_util::{StreamExt, stream};

    use super::*;

    fn ms_to_samples(ms: usize) -> usize {
        ms * SAMPLE_RATE / 1000
    }

    #[tokio::test]
    async fn test_short_vad_chunks_are_merged_before_emit() {
        let chunks = normalize_speech_chunk_stream(
            stream::iter(vec![
                Ok(VadStreamItem::SpeechEnd {
                    detected_speech_samples: ms_to_samples(120),
                    sample_start: 0,
                    sample_end: ms_to_samples(120),
                    samples: vec![1.0; ms_to_samples(120)],
                }),
                Ok(VadStreamItem::SpeechEnd {
                    detected_speech_samples: ms_to_samples(120),
                    sample_start: ms_to_samples(160),
                    sample_end: ms_to_samples(280),
                    samples: vec![1.0; ms_to_samples(120)],
                }),
            ]),
            Duration::from_millis(80),
        )
        .collect::<Vec<_>>()
        .await;

        assert_eq!(chunks.len(), 1);

        let chunk = chunks.into_iter().next().unwrap().unwrap();
        assert_eq!(chunk.sample_start, 0);
        assert_eq!(chunk.sample_end, ms_to_samples(280));
        assert_eq!(chunk.samples.len(), ms_to_samples(280));
    }

    #[tokio::test]
    async fn test_isolated_short_vad_chunk_is_emitted_at_stream_end() {
        let chunks = normalize_speech_chunk_stream(
            stream::iter(vec![Ok(VadStreamItem::SpeechEnd {
                detected_speech_samples: ms_to_samples(120),
                sample_start: 0,
                sample_end: ms_to_samples(120),
                samples: vec![1.0; ms_to_samples(120)],
            })]),
            Duration::from_millis(80),
        )
        .collect::<Vec<_>>()
        .await;

        assert_eq!(chunks.len(), 1);
        let chunk = chunks.into_iter().next().unwrap().unwrap();
        assert_eq!(chunk.sample_start, 0);
        assert_eq!(chunk.sample_end, ms_to_samples(120));
    }

    #[tokio::test]
    async fn test_short_vad_chunks_emit_separately_across_large_gap() {
        let chunks = normalize_speech_chunk_stream(
            stream::iter(vec![
                Ok(VadStreamItem::SpeechEnd {
                    detected_speech_samples: ms_to_samples(120),
                    sample_start: 0,
                    sample_end: ms_to_samples(120),
                    samples: vec![1.0; ms_to_samples(120)],
                }),
                Ok(VadStreamItem::SpeechEnd {
                    detected_speech_samples: ms_to_samples(120),
                    sample_start: ms_to_samples(500),
                    sample_end: ms_to_samples(620),
                    samples: vec![1.0; ms_to_samples(120)],
                }),
            ]),
            Duration::from_millis(80),
        )
        .collect::<Vec<_>>()
        .await;

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].as_ref().unwrap().sample_start, 0);
        assert_eq!(chunks[1].as_ref().unwrap().sample_start, ms_to_samples(500));
    }

    #[test]
    fn test_short_vad_chunks_are_merged_in_batch_mode() {
        let chunks = normalize_speech_chunks(
            vec![
                BufferedChunk {
                    chunk: AudioChunk {
                        samples: vec![1.0; ms_to_samples(120)],
                        sample_start: 0,
                        sample_end: ms_to_samples(120),
                    },
                    detected_speech_samples: ms_to_samples(120),
                },
                BufferedChunk {
                    chunk: AudioChunk {
                        samples: vec![1.0; ms_to_samples(120)],
                        sample_start: ms_to_samples(160),
                        sample_end: ms_to_samples(280),
                    },
                    detected_speech_samples: ms_to_samples(120),
                },
            ],
            Duration::from_millis(80),
        );

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].sample_end, ms_to_samples(280));
        assert_eq!(chunks[0].samples.len(), ms_to_samples(280));
    }
}
