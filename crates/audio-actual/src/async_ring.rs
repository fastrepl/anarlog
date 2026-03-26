use futures_util::task::AtomicWaker;
use ringbuf::traits::Consumer;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

pub(crate) struct PollNextSample {
    pub(crate) poll: Poll<Option<f32>>,
    pub(crate) did_pop_chunk: bool,
}

#[cfg_attr(
    not(any(target_os = "linux", target_os = "windows", test)),
    allow(dead_code)
)]
pub(crate) struct PollNextChunk {
    pub(crate) poll: Poll<Option<Vec<f32>>>,
    pub(crate) did_pop_chunk: bool,
}

pub(crate) struct RingbufAsyncReader<C> {
    consumer: C,
    waker: Arc<AtomicWaker>,
    wake_pending: Arc<AtomicBool>,
    alive: Option<Arc<AtomicBool>>,
    dropped_samples: Option<Arc<AtomicUsize>>,
    dropped_log_message: &'static str,
    dropped_log_pending: usize,
    dropped_log_last: Option<Instant>,
    read_buffer: Vec<f32>,
    read_len: usize,
    read_idx: usize,
}

impl<C> RingbufAsyncReader<C>
where
    C: Consumer<Item = f32>,
{
    const DROPPED_LOG_INTERVAL: Duration = Duration::from_secs(1);

    pub(crate) fn new(
        consumer: C,
        waker: Arc<AtomicWaker>,
        wake_pending: Arc<AtomicBool>,
        read_buffer: Vec<f32>,
    ) -> Self {
        Self {
            consumer,
            waker,
            wake_pending,
            alive: None,
            dropped_samples: None,
            dropped_log_message: "samples_dropped",
            dropped_log_pending: 0,
            dropped_log_last: None,
            read_buffer,
            read_len: 0,
            read_idx: 0,
        }
    }

    pub(crate) fn with_alive(mut self, alive: Arc<AtomicBool>) -> Self {
        self.alive = Some(alive);
        self
    }

    pub(crate) fn with_dropped_samples(
        mut self,
        dropped_samples: Arc<AtomicUsize>,
        dropped_log_message: &'static str,
    ) -> Self {
        self.dropped_samples = Some(dropped_samples);
        self.dropped_log_message = dropped_log_message;
        self
    }

    pub(crate) fn has_buffered_samples(&self) -> bool {
        self.read_idx < self.read_len
    }

    #[cfg_attr(
        not(any(target_os = "linux", target_os = "windows", test)),
        allow(dead_code)
    )]
    fn clear_buffered(&mut self) {
        self.read_len = 0;
        self.read_idx = 0;
    }

    fn maybe_log_dropped(&mut self) {
        let Some(dropped_samples) = &self.dropped_samples else {
            return;
        };

        let dropped = dropped_samples.swap(0, Ordering::Relaxed);
        if dropped == 0 {
            return;
        }

        self.dropped_log_pending = self.dropped_log_pending.saturating_add(dropped);
        let now = Instant::now();
        let should_log = self
            .dropped_log_last
            .is_none_or(|last| now.duration_since(last) >= Self::DROPPED_LOG_INTERVAL);
        if should_log {
            let dropped = std::mem::replace(&mut self.dropped_log_pending, 0);
            self.dropped_log_last = Some(now);
            tracing::warn!(dropped, "{}", self.dropped_log_message);
        }
    }

    fn is_alive(&self) -> bool {
        self.alive
            .as_ref()
            .is_none_or(|alive| alive.load(Ordering::Acquire))
    }

    fn try_pop_chunk(&mut self) -> Option<usize> {
        let popped = {
            let consumer = &mut self.consumer;
            let read_buffer = &mut self.read_buffer;
            consumer.pop_slice(read_buffer)
        };

        if popped > 0 {
            self.read_len = popped;
            self.read_idx = 0;
            self.wake_pending.store(false, Ordering::Release);
            Some(popped)
        } else {
            None
        }
    }

    fn poll_ready_chunk(&mut self, cx: &mut Context<'_>) -> Option<bool> {
        self.maybe_log_dropped();

        if self.try_pop_chunk().is_some() {
            return Some(true);
        }

        if !self.is_alive() {
            return Some(false);
        }

        self.wake_pending.store(true, Ordering::Release);
        self.waker.register(cx.waker());

        if self.try_pop_chunk().is_some() {
            return Some(true);
        }

        if !self.is_alive() {
            return Some(false);
        }

        self.wake_pending.store(true, Ordering::Release);
        None
    }

    pub(crate) fn poll_next_sample(&mut self, cx: &mut Context<'_>) -> PollNextSample {
        if self.read_idx < self.read_len {
            let sample = self.read_buffer[self.read_idx];
            self.read_idx += 1;
            return PollNextSample {
                poll: Poll::Ready(Some(sample)),
                did_pop_chunk: false,
            };
        }

        match self.poll_ready_chunk(cx) {
            Some(true) => {
                let sample = self.read_buffer[0];
                self.read_idx = 1;
                PollNextSample {
                    poll: Poll::Ready(Some(sample)),
                    did_pop_chunk: true,
                }
            }
            Some(false) => PollNextSample {
                poll: Poll::Ready(None),
                did_pop_chunk: false,
            },
            None => PollNextSample {
                poll: Poll::Pending,
                did_pop_chunk: false,
            },
        }
    }

    #[cfg_attr(
        not(any(target_os = "linux", target_os = "windows", test)),
        allow(dead_code)
    )]
    pub(crate) fn poll_next_chunk(&mut self, cx: &mut Context<'_>) -> PollNextChunk {
        if self.read_idx < self.read_len {
            let chunk = self.read_buffer[self.read_idx..self.read_len].to_vec();
            self.clear_buffered();
            return PollNextChunk {
                poll: Poll::Ready(Some(chunk)),
                did_pop_chunk: false,
            };
        }

        match self.poll_ready_chunk(cx) {
            Some(true) => {
                let chunk = self.read_buffer[..self.read_len].to_vec();
                self.clear_buffered();
                PollNextChunk {
                    poll: Poll::Ready(Some(chunk)),
                    did_pop_chunk: true,
                }
            }
            Some(false) => PollNextChunk {
                poll: Poll::Ready(None),
                did_pop_chunk: false,
            },
            None => PollNextChunk {
                poll: Poll::Pending,
                did_pop_chunk: false,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringbuf::{
        HeapRb,
        traits::{Producer, Split},
    };

    #[test]
    fn poll_next_chunk_rechecks_after_register() {
        let rb = HeapRb::<f32>::new(8);
        let (mut producer, consumer) = rb.split();
        let waker = Arc::new(AtomicWaker::new());
        let wake_pending = Arc::new(AtomicBool::new(false));
        let mut reader = RingbufAsyncReader::new(consumer, waker, wake_pending, vec![0.0; 8]);

        let waker = futures_util::task::noop_waker();
        let mut cx = Context::from_waker(&waker);

        reader.wake_pending.store(true, Ordering::Release);
        producer.push_slice(&[1.0, 2.0, 3.0]);

        let res = reader.poll_next_chunk(&mut cx);
        assert!(res.did_pop_chunk);
        assert_eq!(res.poll, Poll::Ready(Some(vec![1.0, 2.0, 3.0])));
    }

    #[test]
    fn poll_next_chunk_returns_none_when_source_dies() {
        let rb = HeapRb::<f32>::new(8);
        let (_producer, consumer) = rb.split();
        let waker = Arc::new(AtomicWaker::new());
        let wake_pending = Arc::new(AtomicBool::new(false));
        let alive = Arc::new(AtomicBool::new(false));
        let mut reader =
            RingbufAsyncReader::new(consumer, waker, wake_pending, vec![0.0; 8]).with_alive(alive);

        let waker = futures_util::task::noop_waker();
        let mut cx = Context::from_waker(&waker);

        let res = reader.poll_next_chunk(&mut cx);
        assert!(!res.did_pop_chunk);
        assert_eq!(res.poll, Poll::Ready(None));
    }
}
