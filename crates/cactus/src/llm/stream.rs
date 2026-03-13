use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures_util::Stream;
use hypr_llm_types::{Response, StreamingParser};
use tokio::sync::mpsc::UnboundedSender;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_util::sync::CancellationToken;

use crate::error::Result;
use crate::model::Model;

use super::CompleteOptions;
use super::Message;

struct StreamWorker {
    model: Arc<Model>,
    cancellation_token: CancellationToken,
    tx: UnboundedSender<Response>,
    parser: StreamingParser,
}

impl StreamWorker {
    fn new(
        model: Arc<Model>,
        cancellation_token: CancellationToken,
        tx: UnboundedSender<Response>,
    ) -> Self {
        Self {
            model,
            cancellation_token,
            tx,
            parser: StreamingParser::new(),
        }
    }

    fn on_token(&mut self, chunk: &str) -> bool {
        if self.cancellation_token.is_cancelled() || self.tx.is_closed() {
            self.model.stop();
            return false;
        }
        for response in self.parser.process_chunk(chunk) {
            if self.tx.send(response).is_err() {
                self.model.stop();
                return false;
            }
        }
        true
    }

    fn run(&mut self, messages: &[Message], options: &CompleteOptions) {
        let model = Arc::clone(&self.model);
        let _ = model.complete_streaming(messages, options, |chunk| self.on_token(chunk));
        if let Some(response) = self.parser.flush() {
            let _ = self.tx.send(response);
        }
    }
}

pub struct CompletionStream {
    inner: UnboundedReceiverStream<Response>,
    cancellation_token: CancellationToken,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl CompletionStream {
    pub fn cancellation_token(&self) -> &CancellationToken {
        &self.cancellation_token
    }

    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }
}

impl Stream for CompletionStream {
    type Item = Response;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

impl Drop for CompletionStream {
    fn drop(&mut self) {
        self.cancellation_token.cancel();
        if let Some(handle) = self.handle.take() {
            std::thread::spawn(move || {
                if let Err(panic) = handle.join() {
                    tracing::error!(?panic, "cactus_completion_worker_panicked");
                }
            });
        }
    }
}

pub fn complete_stream(
    model: &Arc<Model>,
    messages: Vec<Message>,
    options: CompleteOptions,
) -> Result<CompletionStream> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let cancellation_token = CancellationToken::new();

    let model = Arc::clone(model);
    let worker_token = cancellation_token.clone();

    let handle = std::thread::spawn(move || {
        let mut worker = StreamWorker::new(model, worker_token, tx);
        worker.run(&messages, &options);
    });

    let inner = UnboundedReceiverStream::new(rx);
    Ok(CompletionStream {
        inner,
        cancellation_token,
        handle: Some(handle),
    })
}
