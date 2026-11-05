mod app;
mod device;
mod state;

use cidre::core_audio as ca;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use app::poll_apps;
use device::{
    ListenerData, RemovalOutcome, remove_device_listener, seed_mic_state, sync_input_devices,
    system_listener,
};
use state::SharedContext;

const DEVICE_IS_RUNNING_SOMEWHERE: ca::PropAddr = ca::PropAddr {
    selector: ca::PropSelector::DEVICE_IS_RUNNING_SOMEWHERE,
    scope: ca::PropScope::GLOBAL,
    element: ca::PropElement::MAIN,
};

const SYSTEM_SELECTORS: [ca::PropSelector; 2] = [
    ca::PropSelector::HW_DEFAULT_INPUT_DEVICE,
    ca::PropSelector::HW_DEVICES,
];

const POLL_INTERVAL: Duration = Duration::from_secs(1);
const LISTENER_GENERATION_IDLE: u8 = 0;
const LISTENER_GENERATION_ACTIVE: u8 = 1;
const LISTENER_GENERATION_DISABLED: u8 = 2;
static LISTENER_GENERATION_STATE: AtomicU8 = AtomicU8::new(LISTENER_GENERATION_IDLE);

#[derive(Debug, PartialEq, Eq)]
enum ListenerStartDecision {
    Started,
    AlreadyActive,
    PermanentlyDisabled,
}

fn try_start_listener_generation(state: &AtomicU8) -> ListenerStartDecision {
    match state.compare_exchange(
        LISTENER_GENERATION_IDLE,
        LISTENER_GENERATION_ACTIVE,
        Ordering::SeqCst,
        Ordering::SeqCst,
    ) {
        Ok(_) => ListenerStartDecision::Started,
        Err(LISTENER_GENERATION_ACTIVE) => ListenerStartDecision::AlreadyActive,
        Err(LISTENER_GENERATION_DISABLED) => ListenerStartDecision::PermanentlyDisabled,
        Err(value) => unreachable!("unexpected CoreAudio listener generation state: {value}"),
    }
}

fn finish_listener_generation(state: &AtomicU8, retained_callback_context: bool) {
    state.store(
        if retained_callback_context {
            LISTENER_GENERATION_DISABLED
        } else {
            LISTENER_GENERATION_IDLE
        },
        Ordering::SeqCst,
    );
}

struct ListenerGeneration {
    retained_callback_context: bool,
}

impl ListenerGeneration {
    fn new() -> Self {
        Self {
            retained_callback_context: false,
        }
    }

    fn retain_callback_context(&mut self) {
        self.retained_callback_context = true;
    }
}

impl Drop for ListenerGeneration {
    fn drop(&mut self) {
        finish_listener_generation(&LISTENER_GENERATION_STATE, self.retained_callback_context);
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ListenerRetention {
    system: bool,
    device: bool,
}

fn listener_retention(
    system_unregister_failed: bool,
    device_unregister_failed: bool,
    device_context_retention_required: bool,
) -> ListenerRetention {
    ListenerRetention {
        system: system_unregister_failed,
        device: system_unregister_failed
            || device_unregister_failed
            || device_context_retention_required,
    }
}

fn retain_callback_context(data: &mut Option<Box<ListenerData>>) {
    if let Some(data) = data.take() {
        let _ = Box::into_raw(data);
    }
}

struct CoreAudioListeners {
    ctx: SharedContext,
    device_listener_data: Option<Box<ListenerData>>,
    system_listener_data: Option<Box<ListenerData>>,
    system_listeners_registered: Vec<ca::PropSelector>,
    generation: ListenerGeneration,
}

impl CoreAudioListeners {
    fn new(ctx: SharedContext, generation: ListenerGeneration) -> Self {
        let mut device_listener_data = Box::new(ListenerData {
            ctx: ctx.clone_shared(),
            device_listener_ptr: std::ptr::null_mut(),
        });
        let device_listener_ptr = (&mut *device_listener_data as *mut ListenerData).cast::<()>();
        let system_listener_data = Box::new(ListenerData {
            ctx: ctx.clone_shared(),
            device_listener_ptr,
        });

        let mut listeners = Self {
            ctx,
            device_listener_data: Some(device_listener_data),
            system_listener_data: Some(system_listener_data),
            system_listeners_registered: Vec::new(),
            generation,
        };
        listeners.register();
        listeners
    }

    fn device_listener_ptr(&mut self) -> *mut () {
        self.device_listener_data
            .as_deref_mut()
            .map(|data| (data as *mut ListenerData).cast::<()>())
            .expect("device listener context must exist while registered")
    }

    fn system_listener_ptr(&mut self) -> *mut () {
        self.system_listener_data
            .as_deref_mut()
            .map(|data| (data as *mut ListenerData).cast::<()>())
            .expect("system listener context must exist while registered")
    }

    fn register(&mut self) {
        let system_listener_ptr = self.system_listener_ptr();
        for selector in SYSTEM_SELECTORS {
            match ca::System::OBJ.add_prop_listener(
                &selector.global_addr(),
                system_listener,
                system_listener_ptr,
            ) {
                Ok(()) => {
                    self.system_listeners_registered.push(selector);
                    tracing::info!(?selector, "adding_system_listener_success");
                }
                Err(error) => {
                    self.ctx.require_polling_fallback();
                    tracing::error!(
                        ?error,
                        tags.error.code = error.status().0,
                        ?selector,
                        "adding_system_listener_failed"
                    );
                }
            }
        }

        let device_listener_ptr = self.device_listener_ptr();
        sync_input_devices(&self.ctx, device_listener_ptr);

        seed_mic_state(&self.ctx);
    }
}

impl Drop for CoreAudioListeners {
    fn drop(&mut self) {
        self.ctx.deactivate_listener_callbacks();

        let mut system_unregister_failed = false;
        for selector in std::mem::take(&mut self.system_listeners_registered) {
            let system_listener_ptr = self.system_listener_ptr();
            if let Err(error) = ca::System::OBJ.remove_prop_listener(
                &selector.global_addr(),
                system_listener,
                system_listener_ptr,
            ) {
                tracing::error!(?error, "removing_system_listener_failed_retaining_context");
                system_unregister_failed = true;
            }
        }

        // Callbacks are deactivated, so no sync can register a listener after this.
        let ctx = self.ctx.clone_shared();
        let device_listener_ptr = self.device_listener_ptr();
        let _sync = ctx.lock_sync();
        let registered_devices = std::mem::take(
            &mut *ctx
                .input_devices
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        let mut device_unregister_failed = false;
        for device in registered_devices {
            match remove_device_listener(&device, device_listener_ptr) {
                RemovalOutcome::Released => {}
                RemovalOutcome::Gone => ctx.leak_device_listener_context(),
                _ => device_unregister_failed = true,
            }
        }

        let retention = listener_retention(
            system_unregister_failed,
            device_unregister_failed,
            self.ctx.device_listener_context_retention_required(),
        );
        if retention.system {
            retain_callback_context(&mut self.system_listener_data);
        }
        if retention.device || self.ctx.device_listener_context_leaked() {
            retain_callback_context(&mut self.device_listener_data);
        }
        if retention.system || retention.device {
            self.generation.retain_callback_context();
            tracing::error!(
                state = "permanently_disabled",
                reason = "coreaudio_listener_unregister_failed",
                "mic_detector_restart_disabled"
            );
        }
    }
}

struct Worker {
    shutdown: std::sync::Arc<AtomicBool>,
    signal_tx: mpsc::SyncSender<WorkerSignal>,
    thread: JoinHandle<()>,
}

pub(super) enum WorkerSignal {
    Wake,
}

impl Worker {
    fn stop(self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = self.signal_tx.try_send(WorkerSignal::Wake);

        if self.thread.thread().id() == std::thread::current().id() {
            return;
        }

        if let Err(error) = self.thread.join() {
            tracing::error!(?error, "mic_detector_thread_join_failed");
        }
    }
}

#[derive(Default)]
pub struct Detector {
    worker: Option<Worker>,
}

impl Detector {
    fn stop_worker(&mut self) {
        if let Some(worker) = self.worker.take() {
            worker.stop();
        }
    }
}

impl Drop for Detector {
    fn drop(&mut self) {
        self.stop_worker();
    }
}

impl crate::Observer for Detector {
    fn start(&mut self, f: crate::DetectCallback) {
        if self.worker.is_some() {
            return;
        }

        match try_start_listener_generation(&LISTENER_GENERATION_STATE) {
            ListenerStartDecision::Started => {}
            ListenerStartDecision::AlreadyActive => {
                tracing::error!(state = "already_active", "mic_detector_start_rejected");
                return;
            }
            ListenerStartDecision::PermanentlyDisabled => {
                tracing::error!(
                    state = "permanently_disabled",
                    reason = "previous_coreaudio_listener_unregister_failed",
                    "mic_detector_start_rejected"
                );
                return;
            }
        }

        let shutdown = std::sync::Arc::new(AtomicBool::new(false));
        let thread_shutdown = shutdown.clone();
        let (signal_tx, signal_rx) = mpsc::sync_channel(1);
        let context_signal_tx = signal_tx.clone();
        let thread = match std::thread::Builder::new()
            .name("coreaudio-mic-detector".to_string())
            .spawn(move || {
                let generation = ListenerGeneration::new();
                let ctx = SharedContext::with_worker_signal(f, context_signal_tx);
                let _listeners = CoreAudioListeners::new(ctx.clone_shared(), generation);
                let mut last_app_poll = std::time::Instant::now();

                loop {
                    if thread_shutdown.load(Ordering::SeqCst) {
                        break;
                    }
                    ctx.flush_pending_mic_change();
                    if ctx.app_polling_active() && last_app_poll.elapsed() >= POLL_INTERVAL {
                        poll_apps(&ctx);
                        last_app_poll = std::time::Instant::now();
                    }

                    let now = std::time::Instant::now();
                    let debounce_wait = ctx.next_pending_delay(now);
                    let app_poll_wait = ctx.app_polling_active().then(|| {
                        POLL_INTERVAL.saturating_sub(now.saturating_duration_since(last_app_poll))
                    });
                    let wait = match (debounce_wait, app_poll_wait) {
                        (Some(debounce), Some(app_poll)) => Some(debounce.min(app_poll)),
                        (Some(debounce), None) => Some(debounce),
                        (None, Some(app_poll)) => Some(app_poll),
                        (None, None) => None,
                    };

                    match wait {
                        Some(wait) => match signal_rx.recv_timeout(wait) {
                            Ok(WorkerSignal::Wake) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        },
                        None => match signal_rx.recv() {
                            Ok(WorkerSignal::Wake) => {}
                            Err(_) => break,
                        },
                    }
                }
            }) {
            Ok(thread) => thread,
            Err(error) => {
                finish_listener_generation(&LISTENER_GENERATION_STATE, false);
                tracing::error!(?error, "mic_detector_thread_spawn_failed");
                return;
            }
        };

        self.worker = Some(Worker {
            shutdown,
            signal_tx,
            thread,
        });
    }

    fn stop(&mut self) {
        self.stop_worker();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_only_device_context_when_device_unregister_fails() {
        assert_eq!(
            listener_retention(false, true, false),
            ListenerRetention {
                system: false,
                device: true,
            }
        );
    }

    #[test]
    fn system_unregister_failure_retains_both_reachable_contexts() {
        assert_eq!(
            listener_retention(true, false, false),
            ListenerRetention {
                system: true,
                device: true,
            }
        );
    }

    #[test]
    fn successful_unregisters_release_both_contexts() {
        assert_eq!(
            listener_retention(false, false, false),
            ListenerRetention {
                system: false,
                device: false,
            }
        );
    }

    #[test]
    fn orphaned_device_listener_retains_device_context() {
        assert_eq!(
            listener_retention(false, false, true),
            ListenerRetention {
                system: false,
                device: true,
            }
        );
    }

    #[test]
    fn listener_generation_allows_only_one_active_detector() {
        let state = AtomicU8::new(LISTENER_GENERATION_IDLE);

        assert_eq!(
            try_start_listener_generation(&state),
            ListenerStartDecision::Started
        );
        assert_eq!(
            try_start_listener_generation(&state),
            ListenerStartDecision::AlreadyActive
        );

        finish_listener_generation(&state, false);
        assert_eq!(
            try_start_listener_generation(&state),
            ListenerStartDecision::Started
        );
    }

    #[test]
    fn retained_listener_generation_permanently_disables_restart() {
        let state = AtomicU8::new(LISTENER_GENERATION_ACTIVE);

        finish_listener_generation(&state, true);

        assert_eq!(
            try_start_listener_generation(&state),
            ListenerStartDecision::PermanentlyDisabled
        );
    }
}
