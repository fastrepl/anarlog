use std::time::Duration;

use futures_util::{Stream, StreamExt, future, stream};
use hypr_audio_interface::AsyncSource;

use crate::{AudioChunk, Chunker};

#[derive(Debug, Clone)]
pub struct SpeechChunkingConfig {
    redemption_time: Duration,
}

impl Default for SpeechChunkingConfig {
    fn default() -> Self {
        Self {
            redemption_time: Duration::from_millis(600),
        }
    }
}

impl SpeechChunkingConfig {
    pub fn speech(redemption_time: Duration) -> Self {
        Self { redemption_time }
    }
}

pub struct SpeechChunker {
    inner: crate::vad::VadChunker,
}

impl SpeechChunker {
    pub fn new(config: SpeechChunkingConfig) -> Result<Self, crate::Error> {
        Ok(Self {
            inner: crate::vad::VadChunker::new(crate::vad::VadChunkerConfig::speech(
                config.redemption_time,
            ))?,
        })
    }
}

impl Chunker for SpeechChunker {
    type Error = crate::Error;

    fn chunk(&mut self, samples: &[f32], sample_rate: u32) -> Result<Vec<AudioChunk>, Self::Error> {
        self.inner.chunk(samples, sample_rate)
    }
}

pub trait SpeechChunkExt: AsyncSource + Sized {
    fn speech_chunks(
        self,
        config: SpeechChunkingConfig,
    ) -> impl Stream<Item = Result<AudioChunk, crate::Error>>
    where
        Self: 'static,
    {
        match crate::vad::speech_chunks(
            self,
            crate::vad::VadChunkerConfig::speech(config.redemption_time),
        ) {
            Ok(stream) => stream.left_stream(),
            Err(error) => stream::once(future::ready(Err(error))).right_stream(),
        }
    }
}

impl<T: AsyncSource> SpeechChunkExt for T {}

#[cfg(test)]
mod tests {
    use std::{num::NonZero, time::Duration};

    use futures_util::StreamExt;
    use rodio::nz;

    use super::*;

    fn sample_source(sample_rate: u32, samples: Vec<f32>) -> rodio::buffer::SamplesBuffer {
        rodio::buffer::SamplesBuffer::new(nz!(1u16), NonZero::new(sample_rate).unwrap(), samples)
    }

    #[test]
    fn speech_chunker_rejects_invalid_sample_rate() {
        let mut chunker = SpeechChunker::new(SpeechChunkingConfig::default()).unwrap();

        assert!(matches!(
            chunker.chunk(&[], 8_000),
            Err(crate::Error::UnsupportedSampleRate(8_000))
        ));
    }

    #[tokio::test]
    async fn speech_chunk_stream_rejects_invalid_sample_rate() {
        let chunks = sample_source(8_000, vec![0.0; 128])
            .speech_chunks(SpeechChunkingConfig::speech(Duration::from_millis(50)))
            .collect::<Vec<_>>()
            .await;

        assert_eq!(chunks.len(), 1);
        assert!(matches!(
            chunks.first(),
            Some(Err(crate::Error::UnsupportedSampleRate(8_000)))
        ));
    }
}
