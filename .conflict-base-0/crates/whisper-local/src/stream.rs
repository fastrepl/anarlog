use std::{
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::{Stream, StreamExt};
use rodio::Source;

use crate::{Segment, Whisper};

pub struct TranscriptionTask<S, T> {
    stream: S,
    whisper: Whisper,
    current_segment_task: Option<Pin<Box<dyn Stream<Item = Segment> + Send>>>,
    _phantom: PhantomData<T>,
}

pub trait AudioChunk: Send + 'static {
    fn samples(&self) -> &[f32];
    fn meta(&self) -> Option<serde_json::Value>;
}

#[derive(Default)]
pub struct SimpleAudioChunk {
    pub samples: Vec<f32>,
    pub meta: Option<serde_json::Value>,
}

impl AudioChunk for SimpleAudioChunk {
    fn samples(&self) -> &[f32] {
        &self.samples
    }

    fn meta(&self) -> Option<serde_json::Value> {
        self.meta.clone()
    }
}

pub struct AudioChunkStream<S>(pub S);

pub struct RodioSourceMarker;
pub struct MetadataAudioChunkMarker;

pub trait TranscribeRodioSourceStreamExt<S>: Sized {
    fn transcribe(self, whisper: Whisper) -> TranscriptionTask<S, RodioSourceMarker>;
}

impl<S> TranscribeRodioSourceStreamExt<S> for S
where
    S: Stream + Unpin + Send + 'static,
    <S as Stream>::Item: Source + Send + 'static,
{
    fn transcribe(self, whisper: Whisper) -> TranscriptionTask<S, RodioSourceMarker> {
        TranscriptionTask {
            stream: self,
            whisper,
            current_segment_task: None,
            _phantom: PhantomData,
        }
    }
}

pub trait TranscribeMetadataAudioStreamExt<S>: Sized {
    fn transcribe(self, whisper: Whisper) -> TranscriptionTask<S, MetadataAudioChunkMarker>;
}

impl<S, C> TranscribeMetadataAudioStreamExt<S> for AudioChunkStream<S>
where
    S: Stream<Item = C> + Unpin + Send + 'static,
    C: AudioChunk,
{
    fn transcribe(self, whisper: Whisper) -> TranscriptionTask<S, MetadataAudioChunkMarker> {
        TranscriptionTask {
            stream: self.0,
            whisper,
            current_segment_task: None,
            _phantom: PhantomData,
        }
    }
}

impl<S> Stream for TranscriptionTask<S, RodioSourceMarker>
where
    S: Stream + Unpin + Send + 'static,
    <S as Stream>::Item: Source + Send + 'static,
{
    type Item = Segment;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        loop {
            if let Some(task) = &mut this.current_segment_task {
                match task.as_mut().poll_next(cx) {
                    Poll::Ready(Some(segment)) => {
                        return Poll::Ready(Some(segment));
                    }
                    Poll::Ready(None) => {
                        this.current_segment_task = None;
                    }
                    Poll::Pending => return Poll::Pending,
                }
            }

            match this.stream.poll_next_unpin(cx) {
                Poll::Ready(Some(source)) => {
                    let samples: Vec<f32> = source.collect();
                    match process_transcription(
                        &mut this.whisper,
                        &samples,
                        &mut this.current_segment_task,
                        None,
                    ) {
                        Poll::Ready(result) => return Poll::Ready(result),
                        Poll::Pending => continue,
                    }
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl<S, C> Stream for TranscriptionTask<S, MetadataAudioChunkMarker>
where
    S: Stream<Item = C> + Unpin + Send + 'static,
    C: AudioChunk,
{
    type Item = Segment;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        loop {
            if let Some(task) = &mut this.current_segment_task {
                match task.as_mut().poll_next(cx) {
                    Poll::Ready(Some(segment)) => {
                        return Poll::Ready(Some(segment));
                    }
                    Poll::Ready(None) => {
                        this.current_segment_task = None;
                    }
                    Poll::Pending => return Poll::Pending,
                }
            }

            match this.stream.poll_next_unpin(cx) {
                Poll::Ready(Some(chunk)) => {
                    let meta = chunk.meta();
                    let samples = chunk.samples();

                    match process_transcription(
                        &mut this.whisper,
                        samples,
                        &mut this.current_segment_task,
                        meta,
                    ) {
                        Poll::Ready(result) => return Poll::Ready(result),
                        Poll::Pending => continue,
                    }
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

fn process_transcription<'a>(
    whisper: &'a mut Whisper,
    samples: &'a [f32],
    current_segment_task: &'a mut Option<Pin<Box<dyn Stream<Item = Segment> + Send>>>,
    meta: Option<serde_json::Value>,
) -> Poll<Option<Segment>> {
    if !samples.is_empty() {
        match whisper.transcribe(samples) {
            Err(e) => {
                tracing::error!("process_transcription: {:?}", e);
                // Return Ready(None) to terminate the stream on error
                // rather than Pending which could cause infinite polling
                Poll::Ready(None)
            }
            Ok(mut segments) => {
                for segment in &mut segments {
                    segment.meta = meta.clone();
                }

                *current_segment_task = Some(Box::pin(futures_util::stream::iter(segments)));
                Poll::Pending
            }
        }
    } else {
        Poll::Pending
    }
}
