use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::Stream;
use hypr_audio_interface::AsyncSource;
use hypr_vad::silero_onnx::CHUNK_SIZE_16KHZ;
use pin_project::pin_project;

use crate::AudioChunk;

use super::{
    chunk_policy::normalize_speech_chunk_stream,
    session::{VadChunkerConfig, VadSession, VadTransition},
};

#[derive(Debug, Clone)]
pub(crate) enum VadStreamItem {
    #[allow(dead_code)]
    AudioSamples(Vec<f32>),
    #[allow(dead_code)]
    SpeechStart { sample_start: usize },
    SpeechEnd {
        detected_speech_samples: usize,
        sample_start: usize,
        sample_end: usize,
        samples: Vec<f32>,
    },
}

#[pin_project]
pub(crate) struct ContinuousVadStream<S: AsyncSource> {
    source: S,
    vad_session: VadSession,
    buffer: Vec<f32>,
    pending_items: VecDeque<VadStreamItem>,
    finalized: bool,
}

impl<S: AsyncSource> ContinuousVadStream<S> {
    pub(crate) fn new(source: S, config: VadChunkerConfig) -> Result<Self, crate::Error> {
        let sample_rate = source.sample_rate();
        if sample_rate != 16000 {
            return Err(crate::Error::UnsupportedSampleRate(sample_rate));
        }

        Ok(Self {
            source,
            vad_session: VadSession::new(config)?,
            buffer: Vec::with_capacity(CHUNK_SIZE_16KHZ),
            pending_items: VecDeque::new(),
            finalized: false,
        })
    }
}

fn push_transitions(pending: &mut VecDeque<VadStreamItem>, transitions: Vec<VadTransition>) {
    for transition in transitions {
        let item = match transition {
            VadTransition::SpeechStart { sample_start } => {
                VadStreamItem::SpeechStart { sample_start }
            }
            VadTransition::SpeechEnd {
                detected_speech_samples,
                sample_start,
                sample_end,
                samples,
            } => VadStreamItem::SpeechEnd {
                detected_speech_samples,
                sample_start,
                sample_end,
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
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Some(sample)) => this.buffer.push(sample),
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

pub(crate) fn speech_chunks<S: AsyncSource + 'static>(
    source: S,
    config: VadChunkerConfig,
) -> Result<impl Stream<Item = Result<AudioChunk, crate::Error>>, crate::Error> {
    let redemption_time = config.redemption_time;
    let stream = ContinuousVadStream::new(source, config)?;
    Ok(normalize_speech_chunk_stream(stream, redemption_time))
}

#[cfg(test)]
mod tests {
    use std::num::NonZero;

    use futures_util::StreamExt;
    use rodio::nz;

    use super::*;
    use crate::SpeechChunkExt;

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
            VadChunkerConfig::default(),
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
        .speech_chunks(crate::SpeechChunkingConfig::speech(
            std::time::Duration::from_millis(50),
        ));

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
        let mut stream = sample_source(8_000, vec![0.0; CHUNK_SIZE_16KHZ]).speech_chunks(
            crate::SpeechChunkingConfig::speech(std::time::Duration::from_millis(50)),
        );

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
        .speech_chunks(crate::SpeechChunkingConfig::speech(
            std::time::Duration::from_millis(50),
        ))
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

        let mut previous_end = 0usize;
        for chunk in chunks {
            assert!(chunk.sample_start < chunk.sample_end);
            assert!(previous_end <= chunk.sample_start);
            previous_end = chunk.sample_end;
        }
    }
}
