use std::collections::HashSet;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{RecvTimeoutError, SyncSender, sync_channel},
};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::{DetectEvent, InstalledApp};

const POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Default)]
pub struct Detector {
    worker: Option<Worker>,
}

struct Worker {
    shutdown: Arc<AtomicBool>,
    wake_tx: SyncSender<()>,
    handle: JoinHandle<()>,
}

#[derive(Default)]
struct DetectorState {
    active_apps: Vec<InstalledApp>,
}

impl DetectorState {
    fn seed(&mut self, apps: Vec<InstalledApp>) {
        self.active_apps = apps;
    }

    fn reconcile(&mut self, current_apps: Vec<InstalledApp>) -> Vec<DetectEvent> {
        let previous_ids = self
            .active_apps
            .iter()
            .map(|app| &app.id)
            .collect::<HashSet<_>>();
        let current_ids = current_apps
            .iter()
            .map(|app| &app.id)
            .collect::<HashSet<_>>();
        let started = current_apps
            .iter()
            .filter(|app| !previous_ids.contains(&app.id))
            .cloned()
            .collect::<Vec<_>>();
        let stopped = self
            .active_apps
            .iter()
            .filter(|app| !current_ids.contains(&app.id))
            .cloned()
            .collect::<Vec<_>>();

        self.active_apps = current_apps;

        let mut events = Vec::with_capacity(2);
        if !started.is_empty() {
            events.push(DetectEvent::MicStarted(started));
        }
        if !stopped.is_empty() {
            events.push(DetectEvent::MicStopped(stopped));
        }
        events
    }
}

impl Worker {
    fn shutdown(self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = self.wake_tx.try_send(());

        if self.handle.thread().id() != std::thread::current().id() && self.handle.join().is_err() {
            tracing::error!("windows_mic_detector_thread_panicked");
        }
    }
}

impl Detector {
    fn stop_worker(&mut self) {
        if let Some(worker) = self.worker.take() {
            worker.shutdown();
        }
    }
}

impl crate::Observer for Detector {
    fn start(&mut self, callback: crate::DetectCallback) {
        if self
            .worker
            .as_ref()
            .is_some_and(|worker| !worker.handle.is_finished())
        {
            return;
        }

        self.stop_worker();

        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = shutdown.clone();
        let (wake_tx, wake_rx) = sync_channel(1);

        match std::thread::Builder::new()
            .name("windows-mic-detector".to_string())
            .spawn(move || {
                let mut state = DetectorState::default();
                match crate::list_mic_using_apps() {
                    Ok(apps) => state.seed(apps),
                    Err(error) => tracing::warn!(?error, "failed_to_seed_windows_mic_apps"),
                }

                while !thread_shutdown.load(Ordering::SeqCst) {
                    match wake_rx.recv_timeout(POLL_INTERVAL) {
                        Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
                        Err(RecvTimeoutError::Timeout) => {}
                    }
                    if thread_shutdown.load(Ordering::SeqCst) {
                        break;
                    }

                    match crate::list_mic_using_apps() {
                        Ok(apps) => {
                            for event in state.reconcile(apps) {
                                tracing::info!(event = ?event, "detected");
                                callback(event);
                            }
                        }
                        Err(error) => {
                            tracing::warn!(?error, "failed_to_refresh_windows_mic_apps");
                        }
                    }
                }
            }) {
            Ok(handle) => {
                self.worker = Some(Worker {
                    shutdown,
                    wake_tx,
                    handle,
                });
            }
            Err(error) => tracing::error!(?error, "failed_to_spawn_windows_mic_detector"),
        }
    }

    fn stop(&mut self) {
        self.stop_worker();
    }
}

impl Drop for Detector {
    fn drop(&mut self) {
        self.stop_worker();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str) -> InstalledApp {
        InstalledApp {
            id: id.to_string(),
            name: id.to_string(),
        }
    }

    #[test]
    fn detector_state_emits_per_app_changes() {
        let mut state = DetectorState::default();
        let zoom = app("zoom");
        let chrome = app("chrome");

        assert!(state.reconcile(vec![]).is_empty());
        assert!(matches!(
            state.reconcile(vec![zoom.clone()]).as_slice(),
            [DetectEvent::MicStarted(apps)] if apps.len() == 1 && apps[0].id == zoom.id
        ));
        assert!(matches!(
            state.reconcile(vec![chrome.clone()]).as_slice(),
            [DetectEvent::MicStarted(started), DetectEvent::MicStopped(stopped)]
                if started.len() == 1
                    && started[0].id == chrome.id
                    && stopped.len() == 1
                    && stopped[0].id == zoom.id
        ));
    }
}
