mod app;
mod device;
mod state;

use cidre::core_audio as ca;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use app::poll_apps;
use device::{ListenerData, device_listener, system_listener};
use state::SharedContext;

const DEVICE_IS_RUNNING_SOMEWHERE: ca::PropAddr = ca::PropAddr {
    selector: ca::PropSelector::DEVICE_IS_RUNNING_SOMEWHERE,
    scope: ca::PropScope::GLOBAL,
    element: ca::PropElement::MAIN,
};

const POLL_INTERVAL: Duration = Duration::from_secs(1);
const DEBOUNCE_TICK_INTERVAL: Duration = Duration::from_millis(50);
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
) -> ListenerRetention {
    ListenerRetention {
        system: system_unregister_failed,
        device: system_unregister_failed || device_unregister_failed,
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
    system_listener_registered: bool,
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
            system_listener_registered: false,
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
        self.system_listener_registered = ca::System::OBJ
            .add_prop_listener(
                &ca::PropSelector::HW_DEFAULT_INPUT_DEVICE.global_addr(),
                system_listener,
                system_listener_ptr,
            )
            .is_ok();

        if self.system_listener_registered {
            tracing::info!("adding_system_listener_success");
        } else {
            tracing::error!("adding_system_listener_failed");
        }

        match ca::System::default_input_device() {
            Ok(device) => {
                let mic_in_use = device::is_mic_running(&device);
                let device_listener_ptr = self.device_listener_ptr();
                if device
                    .add_prop_listener(
                        &DEVICE_IS_RUNNING_SOMEWHERE,
                        device_listener,
                        device_listener_ptr,
                    )
                    .is_ok()
                {
                    tracing::info!("adding_device_listener_success");

                    let mut device_guard = self
                        .ctx
                        .current_device
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    *device_guard = Some(device);
                    drop(device_guard);
                    if let Some(mic_in_use) = mic_in_use {
                        self.ctx.seed_running_state(mic_in_use);
                    }
                } else {
                    tracing::error!("adding_device_listener_failed");
                }
            }
            Err(_) => tracing::warn!("no_default_input_device_found"),
        }
    }
}

impl Drop for CoreAudioListeners {
    fn drop(&mut self) {
        self.ctx.deactivate_listener_callbacks();

        let system_unregister_failed = if self.system_listener_registered {
            let system_listener_ptr = self.system_listener_ptr();
            match ca::System::OBJ.remove_prop_listener(
                &ca::PropSelector::HW_DEFAULT_INPUT_DEVICE.global_addr(),
                system_listener,
                system_listener_ptr,
            ) {
                Ok(()) => false,
                Err(error) => {
                    tracing::error!(?error, "removing_system_listener_failed_retaining_context");
                    true
                }
            }
        } else {
            false
        };

        let registered_device = self
            .ctx
            .current_device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        let device_unregister_failed = if let Some(device) = registered_device {
            let device_listener_ptr = self.device_listener_ptr();
            match device.remove_prop_listener(
                &DEVICE_IS_RUNNING_SOMEWHERE,
                device_listener,
                device_listener_ptr,
            ) {
                Ok(()) => false,
                Err(error) => {
                    tracing::error!(?error, "removing_device_listener_failed_retaining_context");
                    true
                }
            }
        } else {
            false
        };

        let retention = listener_retention(system_unregister_failed, device_unregister_failed);
        if retention.system {
            retain_callback_context(&mut self.system_listener_data);
        }
        if retention.device {
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
    shutdown_tx: mpsc::Sender<()>,
    thread: JoinHandle<()>,
}

impl Worker {
    fn stop(self) {
        let _ = self.shutdown_tx.send(());

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

        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        let thread = match std::thread::Builder::new()
            .name("coreaudio-mic-detector".to_string())
            .spawn(move || {
                let generation = ListenerGeneration::new();
                let ctx = SharedContext::new(f);
                let _listeners = CoreAudioListeners::new(ctx.clone_shared(), generation);
                let mut last_app_poll = std::time::Instant::now();

                while let Err(mpsc::RecvTimeoutError::Timeout) =
                    shutdown_rx.recv_timeout(DEBOUNCE_TICK_INTERVAL)
                {
                    ctx.flush_pending_mic_change();
                    if last_app_poll.elapsed() >= POLL_INTERVAL {
                        poll_apps(&ctx);
                        last_app_poll = std::time::Instant::now();
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
            shutdown_tx,
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
            listener_retention(false, true),
            ListenerRetention {
                system: false,
                device: true,
            }
        );
    }

    #[test]
    fn system_unregister_failure_retains_both_reachable_contexts() {
        assert_eq!(
            listener_retention(true, false),
            ListenerRetention {
                system: true,
                device: true,
            }
        );
    }

    #[test]
    fn successful_unregisters_release_both_contexts() {
        assert_eq!(
            listener_retention(false, false),
            ListenerRetention {
                system: false,
                device: false,
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
