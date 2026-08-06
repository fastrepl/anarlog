use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

const TERMINATION_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Clone, Default)]
pub(crate) struct CompetitorTerminationState(Arc<AtomicUsize>);

impl CompetitorTerminationState {
    pub fn set_paused(&self, paused: bool) {
        if paused {
            self.0.fetch_add(1, Ordering::SeqCst);
        } else {
            let _ = self
                .0
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
                    count.checked_sub(1)
                });
        }
    }

    pub fn is_paused(&self) -> bool {
        self.0.load(Ordering::SeqCst) > 0
    }
}

#[cfg(not(any(test, feature = "test-support")))]
pub fn start(state: CompetitorTerminationState) {
    tauri::async_runtime::spawn(run(state, || anlg_detect::terminate_competing_apps()));
}

async fn run(
    state: CompetitorTerminationState,
    terminate: impl Fn() -> Vec<anlg_detect::InstalledApp> + Send + 'static,
) {
    loop {
        if !state.is_paused() {
            let terminated = terminate();
            if !terminated.is_empty() {
                tracing::info!(
                    apps = ?terminated.iter().map(|app| &app.name).collect::<Vec<_>>(),
                    "terminated competing meeting assistants"
                );
            }
        }
        tokio::time::sleep(TERMINATION_INTERVAL).await;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[tokio::test(start_paused = true)]
    async fn terminates_immediately_and_while_anarlog_remains_running() {
        let state = CompetitorTerminationState::default();
        let termination_count = Arc::new(AtomicUsize::new(0));
        let task_count = Arc::clone(&termination_count);
        let task = tokio::spawn(run(state.clone(), move || {
            task_count.fetch_add(1, Ordering::SeqCst);
            Vec::new()
        }));

        tokio::task::yield_now().await;
        assert_eq!(termination_count.load(Ordering::SeqCst), 1);

        state.set_paused(true);
        tokio::time::advance(TERMINATION_INTERVAL).await;
        tokio::task::yield_now().await;
        assert_eq!(termination_count.load(Ordering::SeqCst), 1);

        state.set_paused(true);
        state.set_paused(false);
        tokio::time::advance(TERMINATION_INTERVAL).await;
        tokio::task::yield_now().await;
        assert_eq!(termination_count.load(Ordering::SeqCst), 1);

        state.set_paused(false);
        tokio::time::advance(TERMINATION_INTERVAL).await;
        tokio::task::yield_now().await;
        assert_eq!(termination_count.load(Ordering::SeqCst), 2);

        task.abort();
    }
}
