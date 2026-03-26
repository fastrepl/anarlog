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

pub(super) extern "C-unwind" fn device_listener(
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
            if let Some(running) = is_mic_running(&device) {
                data.ctx.handle_mic_change(running);
            }
        }
    }

    os::Status::NO_ERR
}

pub(super) extern "C-unwind" fn system_listener(
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

            if let Some(running) = mic_in_use {
                data.ctx.handle_mic_change(running);
            }
        }
    }

    os::Status::NO_ERR
}
