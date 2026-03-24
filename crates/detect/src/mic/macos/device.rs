use cidre::{core_audio as ca, os};
use std::sync::atomic::Ordering;

use super::SharedContext;
use super::state::PendingTransition;

const DEVICE_IS_RUNNING_SOMEWHERE: ca::PropAddr = ca::PropAddr {
    selector: ca::PropSelector::DEVICE_IS_RUNNING_SOMEWHERE,
    scope: ca::PropScope::GLOBAL,
    element: ca::PropElement::MAIN,
};

struct ListenerData {
    ctx: SharedContext,
    device_listener_ptr: *mut (),
}

pub(super) fn start(ctx: SharedContext, tx: tokio::sync::mpsc::Sender<()>) {
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
            let mic_in_use = is_mic_running(&device);
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
                if let Some(mic_in_use) = mic_in_use
                    && let Ok(mut state_guard) = data.ctx.state.lock()
                {
                    state_guard.last_state = mic_in_use;
                    if mic_in_use {
                        state_guard.active_apps = crate::list_mic_using_apps();
                        data.ctx.polling_active.store(true, Ordering::SeqCst);
                    }
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
}

impl SharedContext {
    pub(super) fn observe_mic_change(&self, mic_in_use: bool) {
        let outcome = {
            let Ok(mut state_guard) = self.state.lock() else {
                return;
            };

            state_guard.observe(mic_in_use)
        };

        if outcome.cancelled_stop {
            tracing::info!("pending_device_stop_cancelled: recovered");
        }

        if let Some(pending) = outcome.pending {
            if !pending.expected_state {
                tracing::info!(
                    grace_ms = pending.delay.as_millis() as u64,
                    "pending_device_stop_started"
                );
            }

            let ctx = self.clone_shared();
            std::thread::spawn(move || {
                std::thread::sleep(pending.delay);
                ctx.confirm_mic_change(pending);
            });
        }
    }

    fn handle_default_device_change(&self) {
        let cancelled_stop = {
            let Ok(mut state_guard) = self.state.lock() else {
                return;
            };

            state_guard.cancel_pending()
        };

        tracing::info!("default_input_device_changed");
        if cancelled_stop {
            tracing::info!("pending_device_stop_cancelled: device_switched");
        }
    }

    fn confirm_mic_change(&self, pending: PendingTransition) {
        let Ok(device) = ca::System::default_input_device() else {
            tracing::warn!("mic_change_confirmation_skipped: no_default_input_device");
            return;
        };

        let Some(mic_in_use) = is_mic_running(&device) else {
            tracing::warn!("mic_change_confirmation_skipped: failed_to_read_device_state");
            return;
        };

        if mic_in_use != pending.expected_state {
            if !pending.expected_state {
                tracing::info!("pending_device_stop_cancelled: recovered_before_confirm");
            }
            return;
        }

        let Ok(mut state_guard) = self.state.lock() else {
            return;
        };

        if !state_guard.commit(mic_in_use, pending.generation) {
            return;
        }

        if mic_in_use {
            let apps = crate::list_mic_using_apps();
            state_guard.active_apps = apps.clone();
            self.polling_active.store(true, Ordering::SeqCst);
            drop(state_guard);
            self.emit(crate::DetectEvent::MicStarted(apps));
        } else {
            self.polling_active.store(false, Ordering::SeqCst);
            let stopped_apps = std::mem::take(&mut state_guard.active_apps);
            drop(state_guard);
            tracing::info!(
                grace_ms = pending.delay.as_millis() as u64,
                "device_stop_confirmed"
            );
            self.emit(crate::DetectEvent::MicStopped(stopped_apps));
        }
    }
}

fn is_mic_running(device: &ca::Device) -> Option<bool> {
    device
        .prop::<u32>(&DEVICE_IS_RUNNING_SOMEWHERE)
        .map(|v| v != 0)
        .ok()
}

extern "C-unwind" fn device_listener(
    _obj_id: ca::Obj,
    number_addresses: u32,
    addresses: *const ca::PropAddr,
    client_data: *mut (),
) -> os::Status {
    let data = unsafe { &*(client_data as *const ListenerData) };
    let addresses = unsafe { std::slice::from_raw_parts(addresses, number_addresses as usize) };

    for addr in addresses {
        if addr.selector != ca::PropSelector::DEVICE_IS_RUNNING_SOMEWHERE {
            continue;
        }
        if let Ok(device) = ca::System::default_input_device() {
            if let Some(mic_in_use) = is_mic_running(&device) {
                data.ctx.observe_mic_change(mic_in_use);
            }
        }
    }

    os::Status::NO_ERR
}

extern "C-unwind" fn system_listener(
    _obj_id: ca::Obj,
    number_addresses: u32,
    addresses: *const ca::PropAddr,
    client_data: *mut (),
) -> os::Status {
    let data = unsafe { &*(client_data as *const ListenerData) };
    let addresses = unsafe { std::slice::from_raw_parts(addresses, number_addresses as usize) };

    for addr in addresses {
        if addr.selector != ca::PropSelector::HW_DEFAULT_INPUT_DEVICE {
            continue;
        }

        data.ctx.handle_default_device_change();

        let Ok(mut device_guard) = data.ctx.current_device.lock() else {
            continue;
        };

        if let Some(old_device) = device_guard.take() {
            let _ = old_device.remove_prop_listener(
                &DEVICE_IS_RUNNING_SOMEWHERE,
                device_listener,
                data.device_listener_ptr,
            );
        }

        let Ok(new_device) = ca::System::default_input_device() else {
            continue;
        };

        if new_device
            .add_prop_listener(
                &DEVICE_IS_RUNNING_SOMEWHERE,
                device_listener,
                data.device_listener_ptr,
            )
            .is_ok()
        {
            let mic_in_use = is_mic_running(&new_device);
            *device_guard = Some(new_device);
            drop(device_guard);
            if let Some(mic_in_use) = mic_in_use {
                data.ctx.observe_mic_change(mic_in_use);
            }
        }
    }

    os::Status::NO_ERR
}
