use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::Serialize;

#[derive(Default)]
pub(crate) struct Metrics {
    pub accepted: AtomicU64,
    pub busy: AtomicU64,
    pub queued: AtomicU64,
    pub peak_queued: AtomicU64,
    pub completed: AtomicU64,
    pub failures: AtomicU64,
    pub cancelled: AtomicU64,
    pub snapshot_count: AtomicU64,
    pub snapshot_rows: AtomicU64,
    pub snapshot_bytes: AtomicU64,
    pub queue_ns: AtomicU64,
    pub work_ns: AtomicU64,
}

#[derive(Clone, Debug, Serialize)]
pub struct RuntimeMetrics {
    pub accepted: u64,
    pub busy: u64,
    pub queued: u64,
    pub peak_queued: u64,
    pub completed: u64,
    pub failures: u64,
    pub cancelled: u64,
    pub snapshot_count: u64,
    pub snapshot_rows: u64,
    pub snapshot_bytes: u64,
    pub queue_ns: u64,
    pub work_ns: u64,
}

impl Metrics {
    pub fn snapshot(&self) -> RuntimeMetrics {
        RuntimeMetrics {
            accepted: self.accepted.load(Ordering::Relaxed),
            busy: self.busy.load(Ordering::Relaxed),
            queued: self.queued.load(Ordering::Relaxed),
            peak_queued: self.peak_queued.load(Ordering::Relaxed),
            completed: self.completed.load(Ordering::Relaxed),
            failures: self.failures.load(Ordering::Relaxed),
            cancelled: self.cancelled.load(Ordering::Relaxed),
            snapshot_count: self.snapshot_count.load(Ordering::Relaxed),
            snapshot_rows: self.snapshot_rows.load(Ordering::Relaxed),
            snapshot_bytes: self.snapshot_bytes.load(Ordering::Relaxed),
            queue_ns: self.queue_ns.load(Ordering::Relaxed),
            work_ns: self.work_ns.load(Ordering::Relaxed),
        }
    }

    pub fn admit(&self) {
        self.accepted.fetch_add(1, Ordering::Relaxed);
        let queued = self.queued.fetch_add(1, Ordering::Relaxed) + 1;
        self.peak_queued.fetch_max(queued, Ordering::Relaxed);
    }

    pub fn start(&self, age: Duration) {
        self.queued.fetch_sub(1, Ordering::Relaxed);
        self.queue_ns.fetch_add(nanos(age), Ordering::Relaxed);
    }

    pub fn complete(&self, elapsed: Duration, failed: bool) {
        self.completed.fetch_add(1, Ordering::Relaxed);
        self.work_ns.fetch_add(nanos(elapsed), Ordering::Relaxed);
        if failed {
            self.failures.fetch_add(1, Ordering::Relaxed);
        }
    }
}

fn nanos(duration: Duration) -> u64 {
    duration.as_nanos().min(u64::MAX as u128) as u64
}
