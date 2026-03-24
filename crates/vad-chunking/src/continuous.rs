use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, StreamExt, future, stream};
use hypr_audio_interface::AsyncSource;
use hypr_vad::silero_onnx::CHUNK_SIZE_16KHZ;
use pin_project::pin_project;

use crate::session::{AdaptiveVadConfig, AdaptiveVadSession, VadTransition};

#[derive(Debug, Clone)]
pub(crate) enum VadStreamItem {
    #[allow(dead_code)]
    AudioSamples(Vec<f32>),
    #[allow(dead_code)]
    SpeechStart { timestamp_ms: usize },
    SpeechEnd {
        start_timestamp_ms: usize,
        end_timestamp_ms: usize,
        samples: Vec<f32>,
    },
}

#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub samples: Vec<f32>,
    pub start_timestamp_ms: usize,
    pub end_timestamp_ms: usize,
}

#[pin_project]
pub(crate) struct ContinuousVadStream<S: AsyncSource> {
    source: S,
    vad_session: AdaptiveVadSession,
    buffer: Vec<f32>,
    pending_items: VecDeque<VadStreamItem>,
    finalized: bool,
}

impl<S: AsyncSource> ContinuousVadStream<S> {
    pub(crate) fn new(source: S, config: AdaptiveVadConfig) -> Result<Self, crate::Error> {
        let sample_rate = source.sample_rate();
        if sample_rate != 16000 {
            return Err(crate::Error::UnsupportedSampleRate(sample_rate));
        }

        Ok(Self {
            source,
            vad_session: AdaptiveVadSession::new(config)?,
            buffer: Vec::with_capacity(CHUNK_SIZE_16KHZ),
            pending_items: VecDeque::new(),
            finalized: false,
        })
    }
}

fn push_transitions(pending: &mut VecDeque<VadStreamItem>, transitions: Vec<VadTransition>) {
    for transition in transitions {
        let item = match transition {
            VadTransition::SpeechStart { timestamp_ms } => {
                VadStreamItem::SpeechStart { timestamp_ms }
            }
            VadTransition::SpeechEnd {
                start_timestamp_ms,
                end_timestamp_ms,
                samples,
            } => VadStreamItem::SpeechEnd {
                start_timestamp_ms,
                end_timestamp_ms,
                samples,
            },
        };
        pending.push_back(item);
    }
}

impl<S: AsyncSource> Stream for ContinuousVadStream<S> {
    type Item = Result<VadStreamItem, crate::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(item) = this.pending_items.pop_front() {
            return Poll::Ready(Some(Ok(item)));
        }

        if this.finalized {
            return Poll::Ready(None);
        }

        let stream = this.source.as_stream();
        let mut stream = std::pin::pin!(stream);

        while this.buffer.len() < CHUNK_SIZE_16KHZ {
            match stream.as_mut().poll_next(cx) {
                Poll::Pending => {
                    return Poll::Pending;
                }
                Poll::Ready(Some(sample)) => {
                    this.buffer.push(sample);
                }
                Poll::Ready(None) => {
                    let trailing_audio = std::mem::take(&mut this.buffer);
                    match this.vad_session.finish(&trailing_audio) {
                        Ok(transitions) => {
                            if !trailing_audio.is_empty() {
                                this.pending_items
                                    .push_back(VadStreamItem::AudioSamples(trailing_audio));
                            }
                            push_transitions(&mut this.pending_items, transitions);
                            this.finalized = true;

                            if let Some(item) = this.pending_items.pop_front() {
                                return Poll::Ready(Some(Ok(item)));
                            }
                        }
                        Err(e) => {
                            this.finalized = true;
                            return Poll::Ready(Some(Err(e)));
                        }
                    }

                    return Poll::Ready(None);
                }
            }
        }

        let mut chunk = Vec::with_capacity(CHUNK_SIZE_16KHZ);
        chunk.extend(this.buffer.drain(..CHUNK_SIZE_16KHZ));

        match this.vad_session.process(&chunk) {
            Ok(transitions) => {
                this.pending_items
                    .push_back(VadStreamItem::AudioSamples(chunk));

                push_transitions(&mut this.pending_items, transitions);

                if let Some(item) = this.pending_items.pop_front() {
                    Poll::Ready(Some(Ok(item)))
                } else {
                    Poll::Pending
                }
            }
            Err(e) => Poll::Ready(Some(Err(e))),
        }
    }
}

pub trait VadExt: AsyncSource + Sized {
    fn speech_chunks(
        self,
        redemption_time: Duration,
    ) -> impl Stream<Item = Result<AudioChunk, crate::Error>>
    where
        Self: 'static,
    {
        let config = AdaptiveVadConfig {
            redemption_time,
            pre_speech_pad: redemption_time,
            min_speech_time: Duration::from_millis(50),
            ..Default::default()
        };

        match ContinuousVadStream::new(self, config) {
            Ok(stream) => stream
                .filter_map(|item| {
                    future::ready(match item {
                        Ok(VadStreamItem::SpeechEnd {
                            samples,
                            start_timestamp_ms,
                            end_timestamp_ms,
                        }) => Some(Ok(AudioChunk {
                            samples,
                            start_timestamp_ms,
                            end_timestamp_ms,
                        })),
                        Ok(_) => None,
                        Err(e) => Some(Err(e)),
                    })
                })
                .left_stream(),
            Err(e) => stream::once(future::ready(Err(e))).right_stream(),
        }
    }
}

impl<T: AsyncSource> VadExt for T {}

#[cfg(test)]
mod tests {
    use std::num::NonZero;

    use futures_util::StreamExt;
    use rodio::nz;

    use super::*;

    fn sample_source(sample_rate: u32, samples: Vec<f32>) -> rodio::buffer::SamplesBuffer {
        rodio::buffer::SamplesBuffer::new(nz!(1u16), NonZero::new(sample_rate).unwrap(), samples)
    }

    #[tokio::test]
    async fn test_no_audio_drops_for_continuous_vad() {
        let all_audio = rodio::Decoder::try_from(
            std::fs::File::open(hypr_data::english_1::AUDIO_PATH).unwrap(),
        )
        .unwrap()
        .collect::<Vec<_>>();

        let vad = ContinuousVadStream::new(
            rodio::Decoder::new(std::io::BufReader::new(
                std::fs::File::open(hypr_data::english_1::AUDIO_PATH).unwrap(),
            ))
            .unwrap(),
            AdaptiveVadConfig::default(),
        )
        .unwrap();

        let all_audio_from_vad = vad
            .filter_map(|item| async move {
                match item {
                    Ok(VadStreamItem::AudioSamples(samples)) => Some(samples),
                    _ => None,
                }
            })
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<f32>>();

        assert_eq!(all_audio, all_audio_from_vad);
    }

    #[tokio::test]
    async fn test_no_speech_drops_for_vad_chunks() {
        let vad = rodio::Decoder::new(std::io::BufReader::new(
            std::fs::File::open(hypr_data::english_1::AUDIO_PATH).unwrap(),
        ))
        .unwrap()
        .speech_chunks(std::time::Duration::from_millis(50));

        let all_audio_from_vad = vad
            .filter_map(|item| async move {
                match item {
                    Ok(AudioChunk { samples, .. }) => Some(samples),
                    _ => None,
                }
            })
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<f32>>();

        let how_many_sec = (all_audio_from_vad.len() as f64 / 16.0) / 1000.0;
        assert!(how_many_sec > 100.0);
    }

    #[tokio::test]
    async fn test_invalid_sample_rate_returns_stream_error() {
        let mut stream = sample_source(8_000, vec![0.0; CHUNK_SIZE_16KHZ])
            .speech_chunks(std::time::Duration::from_millis(50));

        let first = stream.next().await;
        assert!(matches!(
            first,
            Some(Err(crate::Error::UnsupportedSampleRate(8_000)))
        ));
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn test_vad_chunks_are_monotonic_and_non_overlapping() {
        let chunks = rodio::Decoder::new(std::io::BufReader::new(
            std::fs::File::open(hypr_data::english_1::AUDIO_PATH).unwrap(),
        ))
        .unwrap()
        .speech_chunks(std::time::Duration::from_millis(50))
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

        let mut previous_end = 0usize;
        for chunk in chunks {
            assert!(chunk.start_timestamp_ms < chunk.end_timestamp_ms);
            assert!(previous_end <= chunk.start_timestamp_ms);
            previous_end = chunk.end_timestamp_ms;
        }
    }
}
