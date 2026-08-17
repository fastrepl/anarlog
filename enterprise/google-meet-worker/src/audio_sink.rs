use std::error::Error;

use async_trait::async_trait;

use crate::AudioFrame;

#[async_trait]
pub trait AudioFrameSink: Send {
    type Error: Error + Send + Sync + 'static;

    async fn write_frame(&mut self, frame: AudioFrame) -> Result<(), Self::Error>;

    async fn finish(&mut self) -> Result<(), Self::Error>;
}
