mod app;
mod device;
mod state;

use cidre::core_audio as ca;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::BackgroundTask;

use app::spawn_polling_thread;
use device::{ListenerData, device_listener, system_listener};
use state::SharedContext;

const DEVICE_IS_RUNNING_SOMEWHERE: ca::PropAddr = ca::PropAddr {
    selector: ca::PropSelector::DEVICE_IS_RUNNING_SOMEWHERE,
    scope: ca::PropScope::GLOBAL,
    element: ca::PropElement::MAIN,
};

const POLL_INTERVAL: Duration = Duration::from_secs(1);

pub struct Detector {
    background: BackgroundTask,
}

impl Default for Detector {
    fn default() -> Self {
        Self {
            background: BackgroundTask::default(),
        }
    }
}

impl crate::Observer for Detector {
    fn start(&mut self, f: crate::DetectCallback) {
        self.background.start(|running, mut rx| async move {
            let (tx, mut notify_rx) = tokio::sync::mpsc::channel(1);

            std::thread::spawn(move || {
                let ctx = SharedContext::new(f);

                spawn_polling_thread(ctx.clone_shared());

                let device_listener_data = Box::new(ListenerData {
                    ctx: ctx.clone_shared(),
                    device_listener_ptr: std::ptr::null_mut(),
                });
                let device_listener_ptr = Box::into_raw(device_listener_data) as *mut ();

                let system_listener_data = Box::new(ListenerData {
                    ctx,
                    device_listener_ptr,
                });
                let system_listener_ptr = Box::into_raw(system_listener_data) as *mut ();

                if let Err(e) = ca::System::OBJ.add_prop_listener(
                    &ca::PropSelector::HW_DEFAULT_INPUT_DEVICE.global_addr(),
                    system_listener,
                    system_listener_ptr,
                ) {
                    tracing::error!("adding_system_listener_failed: {:?}", e);
                } else {
                    tracing::info!("adding_system_listener_success");
                }

                match ca::System::default_input_device() {
                    Ok(device) => {
                        let mic_in_use = device::is_mic_running(&device);
                        if device
                            .add_prop_listener(
                                &DEVICE_IS_RUNNING_SOMEWHERE,
                                device_listener,
                                device_listener_ptr,
                            )
                            .is_ok()
                        {
                            tracing::info!("adding_device_listener_success");

                            let data = unsafe { &*(system_listener_ptr as *const ListenerData) };
                            if let Ok(mut device_guard) = data.ctx.current_device.lock() {
                                *device_guard = Some(device);
                            }
                            if let Some(mic_in_use) = mic_in_use {
                                data.ctx.seed_running_state(mic_in_use);
                            }
                        } else {
                            tracing::error!("adding_device_listener_failed");
                        }
                    }
                    Err(_) => tracing::warn!("no_default_input_device_found"),
                }

                let _ = tx.blocking_send(());
                loop {
                    std::thread::park();
                }
            });

            let _ = notify_rx.recv().await;

            loop {
                tokio::select! {
                    _ = &mut rx => break,
                    _ = tokio::time::sleep(Duration::from_millis(500)) => {
                        if !running.load(Ordering::SeqCst) {
                            break;
                        }
                    }
                }
            }
        });
    }

    fn stop(&mut self) {
        self.background.stop();
    }
}
