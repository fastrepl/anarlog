use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::Stream;
use pin_project::pin_project;

use crate::{
    continuous::{AudioChunk, VadStreamItem},
    session::AdaptiveVadConfig,
};

const SAMPLE_RATE: usize = 16000;
const MIN_DETECTED_SPEECH_MS: u64 = 200;
const MAX_SHORT_CHUNK_MERGE_GAP_MS: usize = 250;
const MIN_SPEECH_CONFIRM_MS: u64 = 150;

pub(crate) fn speech_chunk_vad_config(redemption_time: Duration) -> AdaptiveVadConfig {
    AdaptiveVadConfig {
        redemption_time,
        pre_speech_pad: redemption_time,
        min_speech_time: Duration::from_millis(MIN_SPEECH_CONFIRM_MS),
        ..Default::default()
    }
}

pub(crate) fn normalize_speech_chunks<S>(
    inner: S,
    redemption_time: Duration,
) -> SpeechChunkStream<S> {
    SpeechChunkStream::new(inner, redemption_time)
}

fn duration_to_samples(duration: Duration) -> usize {
    ((duration.as_millis() * SAMPLE_RATE as u128) / 1000) as usize
}

#[derive(Debug, Clone)]
struct BufferedChunk {
    chunk: AudioChunk,
    detected_speech_samples: usize,
}

impl BufferedChunk {
    fn is_short(&self, min_detected_speech_samples: usize) -> bool {
        self.detected_speech_samples < min_detected_speech_samples
    }

    fn gap_ms(&self, next: &Self) -> usize {
        next.chunk
            .start_timestamp_ms
            .saturating_sub(self.chunk.end_timestamp_ms)
    }

    fn merge(mut self, next: Self) -> Self {
        let gap_samples = self.gap_ms(&next) * SAMPLE_RATE / 1000;
        if gap_samples > 0 {
            self.chunk
                .samples
                .resize(self.chunk.samples.len() + gap_samples, 0.0);
        }

        self.chunk.samples.extend(next.chunk.samples);
        self.chunk.end_timestamp_ms = next.chunk.end_timestamp_ms;
        self.detected_speech_samples += next.detected_speech_samples;
        self
    }
}

#[pin_project]
pub(crate) struct SpeechChunkStream<S> {
    #[pin]
    inner: S,
    pending: Option<BufferedChunk>,
    min_detected_speech_samples: usize,
    merge_gap_ms: usize,
}

impl<S> SpeechChunkStream<S> {
    fn new(inner: S, redemption_time: Duration) -> Self {
        Self {
            inner,
            pending: None,
            min_detected_speech_samples: duration_to_samples(Duration::from_millis(
                MIN_DETECTED_SPEECH_MS,
            )),
            merge_gap_ms: (redemption_time.as_millis() as usize)
                .clamp(100, MAX_SHORT_CHUNK_MERGE_GAP_MS),
        }
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
                    start_timestamp_ms,
                    end_timestamp_ms,
                    samples,
                }))) => {
                    let next = BufferedChunk {
                        chunk: AudioChunk {
                            samples,
                            start_timestamp_ms,
                            end_timestamp_ms,
                        },
                        detected_speech_samples,
                    };

                    if let Some(pending) = this.pending.take() {
                        if pending.gap_ms(&next) <= *this.merge_gap_ms {
                            let merged = pending.merge(next);
                            if merged.is_short(*this.min_detected_speech_samples) {
                                *this.pending = Some(merged);
                                continue;
                            }

                            return Poll::Ready(Some(Ok(merged.chunk)));
                        }

                        // Gap too large to merge. Stash next, emit pending.
                        *this.pending = Some(next);
                        return Poll::Ready(Some(Ok(pending.chunk)));
                    }

                    if next.is_short(*this.min_detected_speech_samples) {
                        *this.pending = Some(next);
                        continue;
                    }

                    return Poll::Ready(Some(Ok(next.chunk)));
                }
                Poll::Ready(Some(Ok(_))) => continue,
                Poll::Ready(Some(Err(e))) => {
                    *this.pending = None;
                    return Poll::Ready(Some(Err(e)));
                }
                Poll::Ready(None) => {
                    if let Some(pending) = this.pending.take() {
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
        let chunks = normalize_speech_chunks(
            stream::iter(vec![
                Ok(VadStreamItem::SpeechEnd {
                    detected_speech_samples: ms_to_samples(120),
                    start_timestamp_ms: 0,
                    end_timestamp_ms: 120,
                    samples: vec![1.0; ms_to_samples(120)],
                }),
                Ok(VadStreamItem::SpeechEnd {
                    detected_speech_samples: ms_to_samples(120),
                    start_timestamp_ms: 160,
                    end_timestamp_ms: 280,
                    samples: vec![1.0; ms_to_samples(120)],
                }),
            ]),
            Duration::from_millis(80),
        )
        .collect::<Vec<_>>()
        .await;

        assert_eq!(chunks.len(), 1);

        let chunk = chunks.into_iter().next().unwrap().unwrap();
        assert_eq!(chunk.start_timestamp_ms, 0);
        assert_eq!(chunk.end_timestamp_ms, 280);
        assert_eq!(chunk.samples.len(), ms_to_samples(280));
    }

    #[tokio::test]
    async fn test_isolated_short_vad_chunk_is_emitted_at_stream_end() {
        let chunks = normalize_speech_chunks(
            stream::iter(vec![Ok(VadStreamItem::SpeechEnd {
                detected_speech_samples: ms_to_samples(120),
                start_timestamp_ms: 0,
                end_timestamp_ms: 120,
                samples: vec![1.0; ms_to_samples(120)],
            })]),
            Duration::from_millis(80),
        )
        .collect::<Vec<_>>()
        .await;

        assert_eq!(chunks.len(), 1);
        let chunk = chunks.into_iter().next().unwrap().unwrap();
        assert_eq!(chunk.start_timestamp_ms, 0);
        assert_eq!(chunk.end_timestamp_ms, 120);
    }

    #[tokio::test]
    async fn test_short_vad_chunks_emit_separately_across_large_gap() {
        let chunks = normalize_speech_chunks(
            stream::iter(vec![
                Ok(VadStreamItem::SpeechEnd {
                    detected_speech_samples: ms_to_samples(120),
                    start_timestamp_ms: 0,
                    end_timestamp_ms: 120,
                    samples: vec![1.0; ms_to_samples(120)],
                }),
                Ok(VadStreamItem::SpeechEnd {
                    detected_speech_samples: ms_to_samples(120),
                    start_timestamp_ms: 500,
                    end_timestamp_ms: 620,
                    samples: vec![1.0; ms_to_samples(120)],
                }),
            ]),
            Duration::from_millis(80),
        )
        .collect::<Vec<_>>()
        .await;

        assert_eq!(chunks.len(), 2);
        let first = chunks[0].as_ref().unwrap();
        assert_eq!(first.start_timestamp_ms, 0);
        assert_eq!(first.end_timestamp_ms, 120);
        let second = chunks[1].as_ref().unwrap();
        assert_eq!(second.start_timestamp_ms, 500);
        assert_eq!(second.end_timestamp_ms, 620);
    }

    #[tokio::test]
    async fn test_long_enough_vad_chunk_emits_immediately() {
        let chunks = normalize_speech_chunks(
            stream::iter(vec![Ok(VadStreamItem::SpeechEnd {
                detected_speech_samples: ms_to_samples(240),
                start_timestamp_ms: 0,
                end_timestamp_ms: 240,
                samples: vec![1.0; ms_to_samples(240)],
            })]),
            Duration::from_millis(80),
        )
        .collect::<Vec<_>>()
        .await;

        assert_eq!(chunks.len(), 1);

        let chunk = chunks.into_iter().next().unwrap().unwrap();
        assert_eq!(chunk.start_timestamp_ms, 0);
        assert_eq!(chunk.end_timestamp_ms, 240);
        assert_eq!(chunk.samples.len(), ms_to_samples(240));
    }
}
