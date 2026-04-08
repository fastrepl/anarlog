mod chunk_policy;
mod continuous;
mod session;

use hypr_vad::silero_onnx::CHUNK_SIZE_16KHZ;

use crate::{AudioChunk, Chunker};

pub(crate) use continuous::speech_chunks;
pub(crate) use session::{VadChunkerConfig, VadSession};

pub(crate) struct VadChunker {
    config: VadChunkerConfig,
}

impl VadChunker {
    pub(crate) fn new(config: VadChunkerConfig) -> Result<Self, crate::Error> {
        config.validate()?;
        Ok(Self { config })
    }
}

impl Chunker for VadChunker {
    type Error = crate::Error;

    fn chunk(&mut self, samples: &[f32], sample_rate: u32) -> Result<Vec<AudioChunk>, Self::Error> {
        if sample_rate != 16_000 {
            return Err(crate::Error::UnsupportedSampleRate(sample_rate));
        }

        let mut session = VadSession::new(self.config.clone())?;
        let mut speech_chunks = Vec::new();
        let mut processed = 0usize;

        while processed + CHUNK_SIZE_16KHZ <= samples.len() {
            let frame = &samples[processed..processed + CHUNK_SIZE_16KHZ];
            speech_chunks.extend(chunk_policy::speech_chunks_from_transitions(
                session.process(frame)?,
            ));
            processed += CHUNK_SIZE_16KHZ;
        }

        speech_chunks.extend(chunk_policy::speech_chunks_from_transitions(
            session.finish(&samples[processed..])?,
        ));

        Ok(chunk_policy::normalize_speech_chunks(
            speech_chunks,
            self.config.redemption_time,
        ))
    }
}
