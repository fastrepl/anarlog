use libpulse_binding as pulse;
use libpulse_binding::context::{Context, FlagSet as ContextFlagSet};
use libpulse_binding::mainloop::threaded::Mainloop;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, SyncSender, sync_channel},
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::DetectEvent;

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
    last_state: bool,
}

impl DetectorState {
    fn seed(&mut self, state: bool) {
        self.last_state = state;
    }

    fn transition(&mut self, new_state: bool) -> Option<bool> {
        if new_state == self.last_state {
            return None;
        }

        self.last_state = new_state;
        Some(new_state)
    }
}

impl Worker {
    fn shutdown(self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = self.wake_tx.try_send(());

        if self.handle.thread().id() == std::thread::current().id() {
            return;
        }

        if self.handle.join().is_err() {
            tracing::error!("pulseaudio_detector_thread_panicked");
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
    fn start(&mut self, f: crate::DetectCallback) {
        if self
            .worker
            .as_ref()
            .is_some_and(|worker| !worker.handle.is_finished())
        {
            return;
        }

        self.stop_worker();

        let shutdown = Arc::new(AtomicBool::new(false));
        let (wake_tx, wake_rx) = sync_channel(1);
        let thread_shutdown = shutdown.clone();
        let thread_wake_tx = wake_tx.clone();

        match std::thread::Builder::new()
            .name("pulseaudio-mic-detector".to_string())
            .spawn(move || run_detector(f, thread_shutdown, thread_wake_tx, wake_rx))
        {
            Ok(handle) => {
                self.worker = Some(Worker {
                    shutdown,
                    wake_tx,
                    handle,
                });
            }
            Err(error) => {
                tracing::error!(?error, "failed_to_spawn_pulseaudio_detector");
            }
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

fn run_detector(
    callback: crate::DetectCallback,
    shutdown: Arc<AtomicBool>,
    wake_tx: SyncSender<()>,
    wake_rx: Receiver<()>,
) {
    let mut mainloop = match Mainloop::new() {
        Some(m) => m,
        None => {
            tracing::error!("failed_to_create_pulseaudio_mainloop");
            return;
        }
    };

    let mut context = match Context::new(&mainloop, "hyprnote-mic-detector") {
        Some(c) => c,
        None => {
            tracing::error!("failed_to_create_pulseaudio_context");
            return;
        }
    };

    if context
        .connect(None, ContextFlagSet::NOFLAGS, None)
        .is_err()
    {
        tracing::error!("failed_to_connect_to_pulseaudio");
        return;
    }

    if mainloop.start().is_err() {
        tracing::error!("failed_to_start_pulseaudio_mainloop");
        context.disconnect();
        return;
    }

    if !wait_for_context(&mut mainloop, &context, &shutdown) {
        shutdown_context(&mut mainloop, &mut context);
        return;
    }

    tracing::info!("pulseaudio_context_connected");

    let event_wake_tx = wake_tx.clone();
    mainloop.lock();
    context.set_subscribe_callback(Some(Box::new(move |facility, operation, _index| {
        if is_source_output_event(facility, operation) {
            let _ = event_wake_tx.try_send(());
        }
    })));
    let subscribe_operation = context.subscribe(
        pulse::context::subscribe::InterestMaskSet::SOURCE_OUTPUT,
        |success| {
            if success {
                tracing::info!("subscribed_to_pulseaudio_source_output_events");
            } else {
                tracing::error!("failed_to_subscribe_to_pulseaudio_source_output_events");
            }
        },
    );
    mainloop.unlock();

    let mut detector_state = DetectorState::default();
    if let Some(initial_state) = check_mic_in_use(&mut mainloop, &context, &shutdown) {
        detector_state.seed(initial_state);
    }

    while !shutdown.load(Ordering::SeqCst) {
        if wake_rx.recv().is_err() || shutdown.load(Ordering::SeqCst) {
            break;
        }

        if let Some(mic_in_use) = check_mic_in_use(&mut mainloop, &context, &shutdown)
            && let Some(new_state) = detector_state.transition(mic_in_use)
        {
            emit_transition(&callback, new_state);
        }
    }

    mainloop.lock();
    context.set_subscribe_callback(None);
    drop(subscribe_operation);
    context.disconnect();
    mainloop.unlock();
    mainloop.stop();
}

fn wait_for_context(mainloop: &mut Mainloop, context: &Context, shutdown: &AtomicBool) -> bool {
    while !shutdown.load(Ordering::SeqCst) {
        mainloop.lock();
        let state = context.get_state();
        mainloop.unlock();

        match state {
            pulse::context::State::Ready => return true,
            pulse::context::State::Failed | pulse::context::State::Terminated => {
                tracing::error!("pulseaudio_context_connection_failed");
                return false;
            }
            _ => std::thread::sleep(Duration::from_millis(10)),
        }
    }

    false
}

fn shutdown_context(mainloop: &mut Mainloop, context: &mut Context) {
    mainloop.lock();
    context.set_subscribe_callback(None);
    context.disconnect();
    mainloop.unlock();
    mainloop.stop();
}

fn is_source_output_event(
    facility: Option<pulse::context::subscribe::Facility>,
    operation: Option<pulse::context::subscribe::Operation>,
) -> bool {
    matches!(
        (facility, operation),
        (
            Some(pulse::context::subscribe::Facility::SourceOutput),
            Some(
                pulse::context::subscribe::Operation::New
                    | pulse::context::subscribe::Operation::Changed
                    | pulse::context::subscribe::Operation::Removed
            )
        )
    )
}

fn check_mic_in_use(
    mainloop: &mut Mainloop,
    context: &Context,
    shutdown: &AtomicBool,
) -> Option<bool> {
    let result = Arc::new(AtomicBool::new(false));
    let query_done = Arc::new(AtomicBool::new(false));
    let query_failed = Arc::new(AtomicBool::new(false));

    let result_for_callback = result.clone();
    let done_for_callback = query_done.clone();
    let failed_for_callback = query_failed.clone();

    mainloop.lock();
    let mut operation = context
        .introspect()
        .get_source_output_info_list(move |list_result| match list_result {
            pulse::callbacks::ListResult::Item(info) => {
                if !info.corked {
                    result_for_callback.store(true, Ordering::SeqCst);
                }
            }
            pulse::callbacks::ListResult::End => {
                done_for_callback.store(true, Ordering::SeqCst);
            }
            pulse::callbacks::ListResult::Error => {
                failed_for_callback.store(true, Ordering::SeqCst);
                done_for_callback.store(true, Ordering::SeqCst);
            }
        });
    mainloop.unlock();

    let deadline = Instant::now() + Duration::from_secs(1);
    while !query_done.load(Ordering::SeqCst)
        && !shutdown.load(Ordering::SeqCst)
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(10));
    }

    mainloop.lock();
    let completed = query_done.load(Ordering::SeqCst);
    let failed = query_failed.load(Ordering::SeqCst);
    if !completed {
        operation.cancel();
    }
    drop(operation);
    mainloop.unlock();

    if !completed {
        if !shutdown.load(Ordering::SeqCst) {
            tracing::warn!("pulseaudio_source_output_query_timed_out");
        }
        return None;
    }

    if failed {
        tracing::warn!("pulseaudio_source_output_query_failed");
        return None;
    }

    Some(result.load(Ordering::SeqCst))
}

fn emit_transition(callback: &crate::DetectCallback, mic_in_use: bool) {
    let event = if mic_in_use {
        let apps = crate::list_mic_using_apps().unwrap_or_default();
        tracing::info!("mic_started_detected: {:?}", apps);
        DetectEvent::MicStarted(apps)
    } else {
        DetectEvent::MicStopped(vec![])
    };

    tracing::info!(event = ?event, "detected");
    callback(event);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Observer, new_callback};

    #[test]
    fn source_output_lifecycle_events_request_state_refresh() {
        use pulse::context::subscribe::{Facility, Operation};

        for operation in [Operation::New, Operation::Changed, Operation::Removed] {
            assert!(is_source_output_event(
                Some(Facility::SourceOutput),
                Some(operation)
            ));
        }
    }

    #[test]
    fn source_device_events_do_not_request_state_refresh() {
        use pulse::context::subscribe::{Facility, Operation};

        assert!(!is_source_output_event(
            Some(Facility::Source),
            Some(Operation::Changed)
        ));
        assert!(!is_source_output_event(None, Some(Operation::New)));
        assert!(!is_source_output_event(Some(Facility::SourceOutput), None));
    }

    #[test]
    fn detector_state_emits_only_real_transitions() {
        let mut state = DetectorState::default();

        assert_eq!(state.transition(false), None);
        assert_eq!(state.transition(true), Some(true));
        assert_eq!(state.transition(true), None);
        assert_eq!(state.transition(false), Some(false));

        state.seed(true);
        assert_eq!(state.transition(true), None);
        assert_eq!(state.transition(false), Some(false));
    }

    #[test]
    fn worker_shutdown_wakes_and_joins_thread() {
        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = shutdown.clone();
        let exited = Arc::new(AtomicBool::new(false));
        let thread_exited = exited.clone();
        let (wake_tx, wake_rx) = sync_channel(1);
        let handle = std::thread::spawn(move || {
            wake_rx.recv().unwrap();
            assert!(thread_shutdown.load(Ordering::SeqCst));
            thread_exited.store(true, Ordering::SeqCst);
        });

        Worker {
            shutdown,
            wake_tx,
            handle,
        }
        .shutdown();

        assert!(exited.load(Ordering::SeqCst));
    }

    #[tokio::test]
    #[ignore = "requires a live PulseAudio server and microphone client"]
    async fn test_detector() {
        let mut detector = Detector::default();
        detector.start(new_callback(|v| {
            println!("{:?}", v);
        }));

        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        detector.stop();
    }
}
