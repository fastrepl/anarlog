use std::error::Error;
use std::time::Duration;

use async_trait::async_trait;

use anlg_meeting_capture::{CaptureEventPayload, RecordingChunk, Speaker, TranscriptSegment};

use crate::AudioFrame;

#[derive(Debug, Clone, PartialEq)]
pub enum AudioFrameSinkOutput {
    Transcript(TranscriptSegment),
    SpeakerUpserted(Speaker),
    RecordingChunkReady(RecordingChunk),
}

impl From<AudioFrameSinkOutput> for CaptureEventPayload {
    fn from(output: AudioFrameSinkOutput) -> Self {
        match output {
            AudioFrameSinkOutput::Transcript(segment) => Self::Transcript(segment),
            AudioFrameSinkOutput::SpeakerUpserted(speaker) => Self::SpeakerUpserted(speaker),
            AudioFrameSinkOutput::RecordingChunkReady(chunk) => Self::RecordingChunkReady(chunk),
        }
    }
}

#[async_trait]
pub trait AudioFrameSink: Send {
    type Error: Error + Send + Sync + 'static;

    async fn write_frame(
        &mut self,
        frame: AudioFrame,
    ) -> Result<Vec<AudioFrameSinkOutput>, Self::Error>;

    async fn finish(
        &mut self,
        capture_duration: Duration,
    ) -> Result<Vec<AudioFrameSinkOutput>, Self::Error>;
}
