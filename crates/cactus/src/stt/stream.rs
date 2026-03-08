use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures_util::Stream;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

use crate::error::Result;
use crate::model::Model;

use super::TranscribeOptions;
use super::transcriber::{CloudConfig, StreamResult, Transcriber};

#[derive(Debug, Clone)]
pub struct TranscribeEvent {
    pub result: StreamResult,
    pub chunk_duration_secs: f64,
}

pub struct TranscriptionSession {
    audio_tx: tokio::sync::mpsc::Sender<Vec<f32>>,
    inner: ReceiverStream<Result<TranscribeEvent>>,
    cancellation_token: CancellationToken,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl TranscriptionSession {
    pub fn audio_tx(&self) -> &tokio::sync::mpsc::Sender<Vec<f32>> {
        &self.audio_tx
    }

    pub fn cancellation_token(&self) -> &CancellationToken {
        &self.cancellation_token
    }

    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }
}

impl Stream for TranscriptionSession {
    type Item = Result<TranscribeEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

impl Drop for TranscriptionSession {
    fn drop(&mut self) {
        self.cancellation_token.cancel();
        if let Some(handle) = self.handle.take() {
            std::thread::spawn(move || {
                if let Err(panic) = handle.join() {
                    tracing::error!(?panic, "cactus_transcribe_worker_panicked");
                }
            });
        }
    }
}

pub fn transcribe_stream(
    model: Arc<Model>,
    options: TranscribeOptions,
    cloud: CloudConfig,
    chunk_size_ms: u32,
    sample_rate: u32,
) -> TranscriptionSession {
    let (audio_tx, audio_rx) = tokio::sync::mpsc::channel::<Vec<f32>>(64);
    let (event_tx, event_rx) = tokio::sync::mpsc::channel(64);
    let cancellation_token = CancellationToken::new();
    let worker_token = cancellation_token.clone();

    let handle = std::thread::spawn(move || {
        run_transcribe_worker(
            model,
            options,
            cloud,
            chunk_size_ms,
            sample_rate,
            audio_rx,
            event_tx,
            worker_token,
        );
    });

    let inner = ReceiverStream::new(event_rx);
    TranscriptionSession {
        audio_tx,
        inner,
        cancellation_token,
        handle: Some(handle),
    }
}

fn run_transcribe_worker(
    model: Arc<Model>,
    options: TranscribeOptions,
    cloud: CloudConfig,
    chunk_size_ms: u32,
    sample_rate: u32,
    mut audio_rx: tokio::sync::mpsc::Receiver<Vec<f32>>,
    event_tx: tokio::sync::mpsc::Sender<Result<TranscribeEvent>>,
    cancellation_token: CancellationToken,
) {
    let mut transcriber = match Transcriber::new(&model, &options, cloud) {
        Ok(t) => t,
        Err(e) => {
            let _ = event_tx.blocking_send(Err(e));
            return;
        }
    };

    let samples_per_chunk = (sample_rate as usize * chunk_size_ms as usize) / 1000;
    let mut buffer: Vec<f32> = Vec::with_capacity(samples_per_chunk * 2);
    let mut aborted = false;

    while let Some(samples) = audio_rx.blocking_recv() {
        if cancellation_token.is_cancelled() {
            break;
        }

        buffer.extend_from_slice(&samples);

        while buffer.len() >= samples_per_chunk {
            if cancellation_token.is_cancelled() {
                aborted = true;
                break;
            }

            let chunk: Vec<f32> = buffer.drain(..samples_per_chunk).collect();
            let chunk_duration_secs = chunk.len() as f64 / sample_rate as f64;

            match transcriber.process_f32(&chunk) {
                Ok(result) => {
                    let event = TranscribeEvent {
                        result,
                        chunk_duration_secs,
                    };
                    if event_tx.blocking_send(Ok(event)).is_err() {
                        aborted = true;
                        break;
                    }
                }
                Err(e) => {
                    let _ = event_tx.blocking_send(Err(e));
                    aborted = true;
                    break;
                }
            }
        }

        if aborted {
            break;
        }
    }

    if !aborted && !buffer.is_empty() {
        let chunk_duration_secs = buffer.len() as f64 / sample_rate as f64;
        match transcriber.process_f32(&buffer) {
            Ok(result) => {
                let _ = event_tx.blocking_send(Ok(TranscribeEvent {
                    result,
                    chunk_duration_secs,
                }));
            }
            Err(e) => {
                let _ = event_tx.blocking_send(Err(e));
                return;
            }
        }
    }

    if !aborted {
        match transcriber.stop() {
            Ok(result) => {
                let _ = event_tx.blocking_send(Ok(TranscribeEvent {
                    result,
                    chunk_duration_secs: 0.0,
                }));
            }
            Err(e) => {
                let _ = event_tx.blocking_send(Err(e));
            }
        }
    }
}
