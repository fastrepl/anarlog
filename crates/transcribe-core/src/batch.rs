use anlg_audio_chunking::AudioChunk;
use owhisper_interface::batch_sse::BatchSseMessage;
use owhisper_interface::progress::{InferencePhase, InferenceProgress};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use crate::TARGET_SAMPLE_RATE;

const BATCH_EVENT_BUFFER_CAPACITY: usize = 32;
const BATCH_TERMINAL_BUFFER_CAPACITY: usize = 1;
const BATCH_EVENT_SEND_TIMEOUT: Duration = Duration::from_secs(5);
const BATCH_EVENT_SEND_RETRY_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Clone)]
pub struct BatchEventSender {
    tx: mpsc::Sender<BatchSseMessage>,
    terminal_tx: mpsc::Sender<BatchSseMessage>,
    latest_progress: Arc<Mutex<Option<BatchSseMessage>>>,
}

pub struct BatchEventReceiver {
    rx: mpsc::Receiver<BatchSseMessage>,
    terminal_rx: mpsc::Receiver<BatchSseMessage>,
    latest_progress: Arc<Mutex<Option<BatchSseMessage>>>,
    finished: bool,
}

pub fn batch_event_channel() -> (BatchEventSender, BatchEventReceiver) {
    let (tx, rx) = mpsc::channel(BATCH_EVENT_BUFFER_CAPACITY);
    let (terminal_tx, terminal_rx) = mpsc::channel(BATCH_TERMINAL_BUFFER_CAPACITY);
    let latest_progress = Arc::new(Mutex::new(None));

    (
        BatchEventSender {
            tx,
            terminal_tx,
            latest_progress: latest_progress.clone(),
        },
        BatchEventReceiver {
            rx,
            terminal_rx,
            latest_progress,
            finished: false,
        },
    )
}

impl BatchEventSender {
    pub fn send_progress(&self, message: BatchSseMessage) -> bool {
        match self.tx.try_send(message) {
            Ok(()) => {
                if let Ok(mut latest) = self.latest_progress.lock() {
                    latest.take();
                }
                true
            }
            Err(mpsc::error::TrySendError::Full(message)) => {
                if let Ok(mut latest) = self.latest_progress.lock() {
                    *latest = Some(message);
                }
                true
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                if let Ok(mut latest) = self.latest_progress.lock() {
                    latest.take();
                }
                false
            }
        }
    }

    pub fn send_blocking(&self, message: BatchSseMessage) -> bool {
        self.send_blocking_timeout(message, BATCH_EVENT_SEND_TIMEOUT)
    }

    pub fn send_terminal(&self, message: BatchSseMessage) -> bool {
        self.terminal_tx.try_send(message).is_ok()
    }

    pub fn is_closed(&self) -> bool {
        self.tx.is_closed() || self.terminal_tx.is_closed()
    }

    fn send_blocking_timeout(&self, message: BatchSseMessage, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;

        if let Some(progress) = self.take_latest_progress()
            && !self.try_send_until(progress, deadline)
        {
            return false;
        }

        self.try_send_until(message, deadline)
    }

    fn take_latest_progress(&self) -> Option<BatchSseMessage> {
        self.latest_progress
            .lock()
            .ok()
            .and_then(|mut latest| latest.take())
    }

    fn try_send_until(&self, mut message: BatchSseMessage, deadline: Instant) -> bool {
        loop {
            match self.tx.try_send(message) {
                Ok(()) => return true,
                Err(mpsc::error::TrySendError::Closed(_)) => return false,
                Err(mpsc::error::TrySendError::Full(returned)) => {
                    message = returned;
                }
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return false;
            }
            std::thread::sleep(remaining.min(BATCH_EVENT_SEND_RETRY_INTERVAL));
        }
    }
}

impl BatchEventReceiver {
    pub async fn recv(&mut self) -> Option<BatchSseMessage> {
        if self.finished {
            return None;
        }

        if let Ok(message) = self.terminal_rx.try_recv() {
            return self.finish(message);
        }

        match self.rx.try_recv() {
            Ok(message) => return Some(message),
            Err(mpsc::error::TryRecvError::Empty) => {}
            Err(mpsc::error::TryRecvError::Disconnected) => {
                if let Ok(message) = self.terminal_rx.try_recv() {
                    return self.finish(message);
                }
                return self.take_latest_progress();
            }
        }

        if let Some(progress) = self.take_latest_progress() {
            return Some(progress);
        }

        loop {
            tokio::select! {
                biased;
                message = self.terminal_rx.recv(), if !self.terminal_rx.is_closed() => {
                    if let Some(message) = message {
                        return self.finish(message);
                    }
                }
                message = self.rx.recv() => {
                    return message.or_else(|| self.take_latest_progress());
                }
            }
        }
    }

    fn take_latest_progress(&self) -> Option<BatchSseMessage> {
        self.latest_progress
            .lock()
            .ok()
            .and_then(|mut latest| latest.take())
    }

    fn finish(&mut self, message: BatchSseMessage) -> Option<BatchSseMessage> {
        self.finished = true;
        self.rx.close();
        self.terminal_rx.close();
        let _ = self.take_latest_progress();
        Some(message)
    }
}

pub fn initial_resolved_until(chunks: &[AudioChunk], channel_duration: f64) -> f64 {
    chunks
        .first()
        .map(|chunk| chunk.sample_start as f64 / TARGET_SAMPLE_RATE as f64)
        .unwrap_or(channel_duration)
}

pub fn next_resolved_until(chunks: &[AudioChunk], chunk_idx: usize, channel_duration: f64) -> f64 {
    chunks
        .get(chunk_idx + 1)
        .map(|chunk| chunk.sample_start as f64 / TARGET_SAMPLE_RATE as f64)
        .unwrap_or(channel_duration)
}

pub fn overall_resolved_audio(resolved_until: &[f64]) -> f64 {
    let count = resolved_until.len() as f64;
    if count == 0.0 {
        return 0.0;
    }

    resolved_until.iter().copied().sum::<f64>() / count
}

pub fn overall_resolved_with_channel(
    resolved_until: &[f64],
    channel_idx: usize,
    resolved: f64,
) -> f64 {
    let count = resolved_until.len() as f64;
    if count == 0.0 {
        return resolved;
    }

    resolved_until
        .iter()
        .enumerate()
        .map(|(idx, value)| if idx == channel_idx { resolved } else { *value })
        .sum::<f64>()
        / count
}

pub fn record_progress(resolved_audio: f64, total_duration: f64, last_progress: &mut f64) -> f64 {
    let raw = if total_duration > 0.0 {
        (resolved_audio / total_duration).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let progress = raw.max(*last_progress).min(0.99);
    *last_progress = progress;
    progress
}

pub struct ProgressTracker {
    resolved_until: Vec<f64>,
    total_duration: f64,
    last_progress: f64,
    event_tx: Option<BatchEventSender>,
}

impl ProgressTracker {
    pub fn new(
        resolved_until: Vec<f64>,
        total_duration: f64,
        event_tx: Option<BatchEventSender>,
    ) -> Self {
        Self {
            resolved_until,
            total_duration,
            last_progress: 0.0,
            event_tx,
        }
    }

    pub fn update_channel(&mut self, channel_idx: usize, resolved: f64) {
        self.resolved_until[channel_idx] = resolved;
    }

    pub fn emit(&mut self, partial_text: Option<String>) {
        let Some(ref tx) = self.event_tx else { return };
        let resolved_audio = overall_resolved_audio(&self.resolved_until);
        self.emit_inner(tx.clone(), resolved_audio, partial_text);
    }

    pub fn emit_for_channel(
        &mut self,
        channel_idx: usize,
        resolved: f64,
        partial_text: Option<String>,
    ) {
        let Some(ref tx) = self.event_tx else { return };
        let overall = overall_resolved_with_channel(&self.resolved_until, channel_idx, resolved);
        self.emit_inner(tx.clone(), overall, partial_text);
    }

    pub fn has_tx(&self) -> bool {
        self.event_tx.is_some()
    }

    pub fn event_tx(&self) -> Option<&BatchEventSender> {
        self.event_tx.as_ref()
    }

    pub fn is_cancelled(&self) -> bool {
        self.event_tx
            .as_ref()
            .is_some_and(BatchEventSender::is_closed)
    }

    fn emit_inner(
        &mut self,
        tx: BatchEventSender,
        resolved_audio: f64,
        partial_text: Option<String>,
    ) {
        let previous = self.last_progress;
        let percentage =
            record_progress(resolved_audio, self.total_duration, &mut self.last_progress);
        if percentage <= previous {
            return;
        }

        tx.send_progress(BatchSseMessage::Progress {
            progress: InferenceProgress {
                percentage,
                partial_text,
                phase: InferencePhase::Decoding,
            },
        });
    }
}

#[cfg(test)]
mod tests {
    use owhisper_interface::batch_sse::BatchSseMessage;
    use owhisper_interface::progress::{InferencePhase, InferenceProgress};

    use std::time::{Duration, Instant};

    use super::{BATCH_EVENT_BUFFER_CAPACITY, batch_event_channel};

    fn progress(percentage: f64) -> BatchSseMessage {
        BatchSseMessage::Progress {
            progress: InferenceProgress {
                percentage,
                partial_text: None,
                phase: InferencePhase::Decoding,
            },
        }
    }

    fn percentage(message: BatchSseMessage) -> f64 {
        match message {
            BatchSseMessage::Progress { progress } => progress.percentage,
            _ => panic!("expected progress message"),
        }
    }

    #[tokio::test]
    async fn coalesces_progress_when_the_buffer_is_full() {
        let (tx, mut rx) = batch_event_channel();
        for index in 0..BATCH_EVENT_BUFFER_CAPACITY {
            tx.send_progress(progress(index as f64));
        }

        tx.send_progress(progress(100.0));
        tx.send_progress(progress(101.0));

        for index in 0..BATCH_EVENT_BUFFER_CAPACITY {
            assert_eq!(percentage(rx.recv().await.unwrap()), index as f64);
        }
        assert_eq!(percentage(rx.recv().await.unwrap()), 101.0);
    }

    #[tokio::test]
    async fn newer_queued_progress_supersedes_a_coalesced_update() {
        let (tx, mut rx) = batch_event_channel();
        for index in 0..BATCH_EVENT_BUFFER_CAPACITY {
            tx.send_progress(progress(index as f64));
        }
        tx.send_progress(progress(100.0));

        rx.recv().await.unwrap();
        tx.send_progress(progress(101.0));
        for _ in 0..BATCH_EVENT_BUFFER_CAPACITY - 1 {
            rx.recv().await.unwrap();
        }
        assert_eq!(percentage(rx.recv().await.unwrap()), 101.0);

        drop(tx);
        assert!(rx.recv().await.is_none());
    }

    #[tokio::test]
    async fn terminal_message_preempts_buffered_progress_and_ends_the_stream() {
        let (tx, mut rx) = batch_event_channel();
        for index in 0..BATCH_EVENT_BUFFER_CAPACITY {
            tx.send_progress(progress(index as f64));
        }
        tx.send_progress(progress(100.0));

        assert!(tx.send_terminal(BatchSseMessage::Error {
            error: "done".to_string(),
            detail: "done".to_string(),
        }));
        assert!(matches!(
            rx.recv().await.unwrap(),
            BatchSseMessage::Error { .. }
        ));
        assert!(rx.recv().await.is_none());
        assert!(tx.is_closed());
    }

    #[tokio::test]
    async fn priority_messages_flush_coalesced_progress_first() {
        let (tx, mut rx) = batch_event_channel();
        for index in 0..BATCH_EVENT_BUFFER_CAPACITY {
            tx.send_progress(progress(index as f64));
        }
        tx.send_progress(progress(100.0));

        let send = std::thread::spawn(move || {
            tx.send_blocking(BatchSseMessage::Error {
                error: "done".to_string(),
                detail: "done".to_string(),
            })
        });

        for _ in 0..BATCH_EVENT_BUFFER_CAPACITY {
            rx.recv().await.unwrap();
        }
        assert_eq!(percentage(rx.recv().await.unwrap()), 100.0);
        assert!(matches!(
            rx.recv().await.unwrap(),
            BatchSseMessage::Error { .. }
        ));
        assert!(send.join().unwrap());
    }

    #[test]
    fn priority_messages_time_out_when_the_receiver_stalls() {
        let (tx, _rx) = batch_event_channel();
        for index in 0..BATCH_EVENT_BUFFER_CAPACITY {
            tx.send_progress(progress(index as f64));
        }

        let started = Instant::now();
        assert!(!tx.send_blocking_timeout(
            BatchSseMessage::Error {
                error: "done".to_string(),
                detail: "done".to_string(),
            },
            Duration::from_millis(25),
        ));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn sender_reports_a_disconnected_receiver() {
        let (tx, rx) = batch_event_channel();
        drop(rx);

        assert!(tx.is_closed());
        assert!(!tx.send_progress(progress(1.0)));
        assert!(!tx.send_terminal(BatchSseMessage::Error {
            error: "done".to_string(),
            detail: "done".to_string(),
        }));
        assert!(!tx.send_blocking_timeout(
            BatchSseMessage::Error {
                error: "done".to_string(),
                detail: "done".to_string(),
            },
            Duration::from_secs(1),
        ));
    }
}
