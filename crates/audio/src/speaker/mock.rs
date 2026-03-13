use futures_util::Stream;
use pin_project::pin_project;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::task::{Context, Poll};

#[pin_project]
pub struct MockInnerStream {
    chunks: Vec<Vec<f32>>,
    chunk_idx: usize,
    current_rate: Arc<AtomicU32>,
}

impl MockInnerStream {
    pub fn new(chunks: Vec<Vec<f32>>, initial_rate: u32) -> Self {
        Self {
            chunks,
            chunk_idx: 0,
            current_rate: Arc::new(AtomicU32::new(initial_rate)),
        }
    }

    pub fn rate_handle(&self) -> Arc<AtomicU32> {
        self.current_rate.clone()
    }

    pub fn sample_rate(&self) -> u32 {
        self.current_rate.load(Ordering::Acquire)
    }
}

impl Stream for MockInnerStream {
    type Item = Vec<f32>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        if *this.chunk_idx < this.chunks.len() {
            let chunk = this.chunks[*this.chunk_idx].clone();
            *this.chunk_idx += 1;
            Poll::Ready(Some(chunk))
        } else {
            Poll::Ready(None)
        }
    }
}
