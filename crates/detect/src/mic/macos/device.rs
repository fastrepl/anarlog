use cidre::{core_audio as ca, os};

use super::DEVICE_IS_RUNNING_SOMEWHERE;
use super::state::SharedContext;

pub(super) struct ListenerData {
    pub(super) ctx: SharedContext,
    pub(super) device_listener_ptr: *mut (),
}

pub(super) fn is_mic_running(device: &ca::Device) -> Option<bool> {
    device
        .prop::<u32>(&DEVICE_IS_RUNNING_SOMEWHERE)
        .map(|v| v != 0)
        .ok()
}

fn is_current_default_device_callback(obj_id: ca::Obj, device: &ca::Device) -> bool {
    device.0 == obj_id
}

pub(super) extern "C-unwind" fn device_listener(
    obj_id: ca::Obj,
    number_addresses: u32,
    addresses: *const ca::PropAddr,
    client_data: *mut (),
) -> os::Status {
    let data = unsafe { &*(client_data as *const ListenerData) };
    if !data.ctx.listener_callbacks_active() {
        return os::Status::NO_ERR;
    }
    let addresses = unsafe { std::slice::from_raw_parts(addresses, number_addresses as usize) };

    for addr in addresses {
        if addr.selector != ca::PropSelector::DEVICE_IS_RUNNING_SOMEWHERE {
            continue;
        }
        if let Ok(device) = ca::System::default_input_device()
            && is_current_default_device_callback(obj_id, &device)
            && let Some(running) = is_mic_running(&device)
        {
            data.ctx.disable_polling_fallback();
            data.ctx.handle_mic_change(running);
        }
    }

    os::Status::NO_ERR
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_device_callbacks_do_not_match_the_current_default_device() {
        let current_device = ca::Device(ca::Obj(2));

        assert!(is_current_default_device_callback(
            ca::Obj(2),
            &current_device,
        ));
        assert!(!is_current_default_device_callback(
            ca::Obj(1),
            &current_device,
        ));
    }
}

pub(super) extern "C-unwind" fn system_listener(
    _obj_id: ca::Obj,
    number_addresses: u32,
    addresses: *const ca::PropAddr,
    client_data: *mut (),
) -> os::Status {
    let data = unsafe { &*(client_data as *const ListenerData) };
    if !data.ctx.listener_callbacks_active() {
        return os::Status::NO_ERR;
    }
    let addresses = unsafe { std::slice::from_raw_parts(addresses, number_addresses as usize) };

    for addr in addresses {
        if addr.selector != ca::PropSelector::HW_DEFAULT_INPUT_DEVICE {
            continue;
        }

        let Ok(mut device_guard) = data.ctx.current_device.lock() else {
            continue;
        };

        let new_device = ca::System::default_input_device();

        if let Ok(new_device) = new_device.as_ref()
            && device_guard.as_ref() == Some(new_device)
        {
            let mic_in_use = is_mic_running(new_device);
            drop(device_guard);
            if let Some(running) = mic_in_use {
                data.ctx.disable_polling_fallback();
                data.ctx.handle_mic_change(running);
            }
            continue;
        }

        let previous_device = device_guard.take();
        let previous_removal_error = previous_device.as_ref().and_then(|device| {
            device
                .remove_prop_listener(
                    &DEVICE_IS_RUNNING_SOMEWHERE,
                    device_listener,
                    data.device_listener_ptr,
                )
                .err()
        });
        if let Some(error) = previous_removal_error {
            tracing::error!(
                ?error,
                tags.error.code = error.status().0,
                "removing_previous_device_listener_failed"
            );
        }

        let Ok(new_device) = new_device else {
            if previous_removal_error.is_some() {
                *device_guard = previous_device;
            }
            drop(device_guard);
            data.ctx.enable_polling_fallback();
            tracing::warn!("no_default_input_device_found");
            continue;
        };

        match new_device.add_prop_listener(
            &DEVICE_IS_RUNNING_SOMEWHERE,
            device_listener,
            data.device_listener_ptr,
        ) {
            Ok(()) => {
                if previous_removal_error.is_some() {
                    data.ctx.require_device_listener_context_retention();
                }
                let mic_in_use = is_mic_running(&new_device);
                *device_guard = Some(new_device);
                drop(device_guard);

                if let Some(running) = mic_in_use {
                    data.ctx.disable_polling_fallback();
                    data.ctx.handle_mic_change(running);
                } else {
                    data.ctx.enable_polling_fallback();
                }
            }
            Err(error) => {
                if previous_removal_error.is_some() {
                    *device_guard = previous_device;
                }
                drop(device_guard);
                data.ctx.enable_polling_fallback();
                tracing::error!(
                    ?error,
                    tags.error.code = error.status().0,
                    "adding_replacement_device_listener_failed"
                );
            }
        }
    }

    os::Status::NO_ERR
}
