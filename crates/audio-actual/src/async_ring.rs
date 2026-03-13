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

    pub(crate) fn poll_next_sample(&mut self, cx: &mut Context<'_>) -> PollNextSample {
        if self.read_idx < self.read_len {
            let sample = self.read_buffer[self.read_idx];
            self.read_idx += 1;
            return PollNextSample {
                poll: Poll::Ready(Some(sample)),
                did_pop_chunk: false,
            };
        }

        self.maybe_log_dropped();

        let popped = {
            let consumer = &mut self.consumer;
            let read_buffer = &mut self.read_buffer;
            consumer.pop_slice(read_buffer)
        };
        if popped > 0 {
            self.read_len = popped;
            self.read_idx = 1;
            self.wake_pending.store(false, Ordering::Release);
            return PollNextSample {
                poll: Poll::Ready(Some(self.read_buffer[0])),
                did_pop_chunk: true,
            };
        }

        if let Some(alive) = &self.alive
            && !alive.load(Ordering::Acquire)
        {
            return PollNextSample {
                poll: Poll::Ready(None),
                did_pop_chunk: false,
            };
        }

        self.wake_pending.store(true, Ordering::Release);
        self.waker.register(cx.waker());

        let popped = {
            let consumer = &mut self.consumer;
            let read_buffer = &mut self.read_buffer;
            consumer.pop_slice(read_buffer)
        };
        if popped > 0 {
            self.read_len = popped;
            self.read_idx = 1;
            self.wake_pending.store(false, Ordering::Release);
            return PollNextSample {
                poll: Poll::Ready(Some(self.read_buffer[0])),
                did_pop_chunk: true,
            };
        }

        if let Some(alive) = &self.alive
            && !alive.load(Ordering::Acquire)
        {
            return PollNextSample {
                poll: Poll::Ready(None),
                did_pop_chunk: false,
            };
        }

        self.wake_pending.store(true, Ordering::Release);
        PollNextSample {
            poll: Poll::Pending,
            did_pop_chunk: false,
        }
    }
}
