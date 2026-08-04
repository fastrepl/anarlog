use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tokio_util::sync::CancellationToken;

use crate::LocalModel;

const MAX_ACTIVE_DOWNLOAD_POLLERS: usize = 16;

#[derive(Clone)]
pub(crate) struct DownloadPollers {
    inner: Arc<Mutex<PollerState>>,
    native_jobs: Arc<tokio::sync::Semaphore>,
    capacity: usize,
}

#[derive(Default)]
struct PollerState {
    next_generation: u64,
    entries: HashMap<LocalModel, PollerEntry>,
}

struct PollerEntry {
    generation: u64,
    cancellation: CancellationToken,
}

pub(crate) struct DownloadPoller {
    registry: DownloadPollers,
    model: LocalModel,
    generation: u64,
    cancellation: CancellationToken,
}

impl Default for DownloadPollers {
    fn default() -> Self {
        Self::with_capacity(MAX_ACTIVE_DOWNLOAD_POLLERS)
    }
}

impl DownloadPollers {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(PollerState::default())),
            native_jobs: Arc::new(tokio::sync::Semaphore::new(capacity.max(1))),
            capacity: capacity.max(1),
        }
    }

    pub(crate) fn reserve(&self, model: LocalModel) -> Option<DownloadPoller> {
        let mut state = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        if state.entries.contains_key(&model) {
            return None;
        }

        if state.entries.len() >= self.capacity
            && let Some(oldest_model) = state
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.generation)
                .map(|(model, _)| model.clone())
            && let Some(entry) = state.entries.remove(&oldest_model)
        {
            entry.cancellation.cancel();
        }

        let generation = state.next_generation;
        state.next_generation = state.next_generation.wrapping_add(1);
        let cancellation = CancellationToken::new();
        state.entries.insert(
            model.clone(),
            PollerEntry {
                generation,
                cancellation: cancellation.clone(),
            },
        );

        Some(DownloadPoller {
            registry: self.clone(),
            model,
            generation,
            cancellation,
        })
    }

    pub(crate) fn cancel(&self, model: &LocalModel) -> bool {
        let entry = self
            .inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .entries
            .remove(model);
        if let Some(entry) = entry {
            entry.cancellation.cancel();
            true
        } else {
            false
        }
    }

    fn remove(&self, model: &LocalModel, generation: u64) {
        let mut state = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        if state
            .entries
            .get(model)
            .is_some_and(|entry| entry.generation == generation)
        {
            state.entries.remove(model);
        }
    }
}

impl DownloadPoller {
    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    pub(crate) async fn cancelled(&self) {
        self.cancellation.cancelled().await;
    }

    pub(crate) async fn acquire_native_job(&self) -> Option<tokio::sync::OwnedSemaphorePermit> {
        let native_jobs = self.registry.native_jobs.clone();
        tokio::select! {
            biased;
            _ = self.cancelled() => None,
            permit = native_jobs.acquire_owned() => permit.ok(),
        }
    }
}

impl Drop for DownloadPoller {
    fn drop(&mut self) {
        self.registry.remove(&self.model, self.generation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AppleSpeechModel, SoniqoModel};

    fn apple_speech() -> LocalModel {
        LocalModel::AppleSpeech(AppleSpeechModel::Default)
    }

    fn soniqo(model: SoniqoModel) -> LocalModel {
        LocalModel::Soniqo(model)
    }

    #[test]
    fn duplicate_pollers_are_rejected_until_the_first_finishes() {
        let registry = DownloadPollers::default();
        let model = apple_speech();

        let first = registry.reserve(model.clone()).unwrap();
        assert!(registry.reserve(model.clone()).is_none());

        drop(first);
        assert!(registry.reserve(model).is_some());
    }

    #[test]
    fn capacity_evicts_and_cancels_the_oldest_poller() {
        let registry = DownloadPollers::with_capacity(2);
        let first = registry
            .reserve(soniqo(SoniqoModel::ParakeetStreaming))
            .unwrap();
        let second = registry
            .reserve(soniqo(SoniqoModel::ParakeetBatch))
            .unwrap();

        let third = registry.reserve(soniqo(SoniqoModel::Omnilingual)).unwrap();

        assert!(first.is_cancelled());
        assert!(!second.is_cancelled());
        assert!(!third.is_cancelled());
    }

    #[test]
    fn cancel_removes_and_signals_the_registered_poller() {
        let registry = DownloadPollers::default();
        let model = apple_speech();
        let first = registry.reserve(model.clone()).unwrap();

        assert!(registry.cancel(&model));
        assert!(first.is_cancelled());
        assert!(registry.reserve(model).is_some());
    }

    #[test]
    fn stale_poller_drop_does_not_remove_its_replacement() {
        let registry = DownloadPollers::default();
        let model = apple_speech();
        let first = registry.reserve(model.clone()).unwrap();
        assert!(registry.cancel(&model));
        let second = registry.reserve(model.clone()).unwrap();

        drop(first);

        assert!(registry.reserve(model).is_none());
        drop(second);
    }

    #[tokio::test]
    async fn cancelled_poller_keeps_native_job_capacity_until_blocking_work_finishes() {
        let registry = DownloadPollers::with_capacity(1);
        let model = apple_speech();
        let first = registry.reserve(model.clone()).unwrap();
        let native_job = first.acquire_native_job().await.unwrap();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let blocking = tokio::task::spawn_blocking(move || {
            let _native_job = native_job;
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        });
        started_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap();

        assert!(registry.cancel(&model));
        drop(first);
        let replacement = registry.reserve(model).unwrap();
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(25),
                replacement.acquire_native_job()
            )
            .await
            .is_err()
        );

        release_tx.send(()).unwrap();
        blocking.await.unwrap();
        let replacement_job = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            replacement.acquire_native_job(),
        )
        .await
        .unwrap();
        assert!(replacement_job.is_some());
    }
}
