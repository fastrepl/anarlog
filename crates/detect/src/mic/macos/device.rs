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

fn aggregate_running(states: impl IntoIterator<Item = Option<bool>>) -> Option<bool> {
    let mut any_unknown = false;
    for state in states {
        match state {
            Some(true) => return Some(true),
            Some(false) => {}
            None => any_unknown = true,
        }
    }
    (!any_unknown).then_some(false)
}

fn watch_set<T: Copy + PartialEq>(
    registered: &[T],
    listed: &[(T, Option<bool>)],
    default: Option<T>,
) -> Vec<T> {
    listed
        .iter()
        .filter(|(device, has_input)| {
            Some(*device) == default
                || match has_input {
                    Some(has_input) => *has_input,
                    None => registered.contains(device),
                }
        })
        .map(|(device, _)| *device)
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum RemovalOutcome {
    Released,
    Gone,
    Orphaned,
    Retry,
}

fn removal_outcome(is_bad_obj: bool, device_still_listed: bool) -> RemovalOutcome {
    if !device_still_listed {
        // Unplugging can give BAD_OBJ or BAD_DEVICE; any error means "gone".
        RemovalOutcome::Gone
    } else if is_bad_obj {
        RemovalOutcome::Orphaned
    } else {
        RemovalOutcome::Retry
    }
}

fn list_devices_with_input() -> os::Result<Vec<(ca::Device, Option<bool>)>> {
    Ok(ca::System::devices()?
        .into_iter()
        .map(|device| {
            let has_input = device
                .prop_size(&ca::PropSelector::DEVICE_STREAMS.input_addr())
                .ok()
                .map(|size| size > 0);
            (device, has_input)
        })
        .collect())
}

pub(super) fn any_input_device_running() -> Option<bool> {
    let listed = list_devices_with_input().ok()?;
    let default = ca::System::default_input_device().ok();
    aggregate_running(watch_set(&[], &listed, default).iter().map(is_mic_running))
}

pub(super) fn registered_devices_running(ctx: &SharedContext) -> Option<bool> {
    let devices = ctx.input_devices_snapshot();
    if devices.is_empty() {
        return any_input_device_running();
    }
    aggregate_running(devices.iter().map(is_mic_running))
}

fn device_still_listed(device: &ca::Device) -> bool {
    // If listing fails, assume it exists: the safe (retaining) answer.
    ca::System::devices().map_or(true, |devices| devices.contains(device))
}

pub(super) fn remove_device_listener(
    device: &ca::Device,
    device_listener_ptr: *mut (),
) -> RemovalOutcome {
    match device.remove_prop_listener(
        &DEVICE_IS_RUNNING_SOMEWHERE,
        device_listener,
        device_listener_ptr,
    ) {
        Ok(()) => RemovalOutcome::Released,
        Err(error) => {
            let outcome = removal_outcome(
                error == ca::hardware_err::BAD_OBJ,
                device_still_listed(device),
            );
            match outcome {
                RemovalOutcome::Gone => tracing::debug!("input_device_already_removed"),
                _ => tracing::error!(
                    ?error,
                    tags.error.code = error.status().0,
                    ?outcome,
                    "removing_device_listener_failed"
                ),
            }
            outcome
        }
    }
}

pub(super) fn sync_input_devices(ctx: &SharedContext, device_listener_ptr: *mut ()) {
    let _sync = ctx.lock_sync();
    if !ctx.listener_callbacks_active() {
        return;
    }

    let listed = match list_devices_with_input() {
        Ok(listed) => listed,
        Err(error) => {
            ctx.require_polling_fallback();
            tracing::warn!(
                ?error,
                tags.error.code = error.status().0,
                "listing_input_devices_failed"
            );
            return;
        }
    };

    let registered = ctx.input_devices_snapshot();
    let default = ca::System::default_input_device().ok();
    let wanted = watch_set(&registered, &listed, default);
    let removed = registered.iter().filter(|device| !wanted.contains(device));
    let added = wanted.iter().filter(|device| !registered.contains(device));

    let mut forgotten = Vec::new();
    for &device in removed {
        match remove_device_listener(&device, device_listener_ptr) {
            RemovalOutcome::Retry => continue,
            RemovalOutcome::Gone => ctx.leak_device_listener_context(),
            RemovalOutcome::Orphaned => ctx.require_device_listener_context_retention(),
            RemovalOutcome::Released => {}
        }
        forgotten.push(device);
    }

    let mut registered_now = Vec::new();
    for &device in added {
        match device.add_prop_listener(
            &DEVICE_IS_RUNNING_SOMEWHERE,
            device_listener,
            device_listener_ptr,
        ) {
            Ok(()) => registered_now.push(device),
            Err(error) => {
                ctx.require_polling_fallback();
                tracing::error!(
                    ?error,
                    tags.error.code = error.status().0,
                    "adding_device_listener_failed"
                );
            }
        }
    }

    let mut devices = ctx
        .input_devices
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    devices.retain(|device| !forgotten.contains(device));
    devices.extend(registered_now);
}

pub(super) fn refresh_mic_state(ctx: &SharedContext) {
    let _serialized = ctx.lock_refresh();

    match registered_devices_running(ctx) {
        Some(running) => {
            ctx.disable_polling_fallback();
            ctx.handle_mic_change(running);
        }
        None => {
            ctx.enable_polling_fallback();
            tracing::warn!("no_input_device_state_available");
        }
    }
}

pub(super) fn seed_mic_state(ctx: &SharedContext) {
    let _serialized = ctx.lock_refresh();

    match registered_devices_running(ctx) {
        Some(mic_in_use) => ctx.seed_running_state(mic_in_use),
        None => {
            ctx.enable_polling_fallback();
            tracing::warn!("no_input_device_state_available");
        }
    }
}

pub(super) extern "C-unwind" fn device_listener(
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

    if addresses
        .iter()
        .any(|addr| addr.selector == ca::PropSelector::DEVICE_IS_RUNNING_SOMEWHERE)
    {
        refresh_mic_state(&data.ctx);
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
    if !data.ctx.listener_callbacks_active() {
        return os::Status::NO_ERR;
    }
    let addresses = unsafe { std::slice::from_raw_parts(addresses, number_addresses as usize) };

    if addresses.iter().any(|addr| {
        addr.selector == ca::PropSelector::HW_DEVICES
            || addr.selector == ca::PropSelector::HW_DEFAULT_INPUT_DEVICE
    }) {
        sync_input_devices(&data.ctx, data.device_listener_ptr);
        refresh_mic_state(&data.ctx);
    }

    os::Status::NO_ERR
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_running_device_means_mic_in_use() {
        assert_eq!(
            aggregate_running([Some(false), Some(true), Some(false)]),
            Some(true)
        );
        assert_eq!(aggregate_running([None, Some(true)]), Some(true));
        assert_eq!(aggregate_running([Some(true), None]), Some(true));
    }

    #[test]
    fn idle_unknown_and_empty_device_sets() {
        assert_eq!(aggregate_running([Some(false), Some(false)]), Some(false));
        assert_eq!(aggregate_running([None, Some(false)]), None);
        assert_eq!(aggregate_running([None, None]), None);
        assert_eq!(aggregate_running([]), Some(false));
    }

    #[test]
    fn watch_set_follows_input_streams_and_the_device_list() {
        let listed = [(1, Some(true)), (2, Some(false)), (3, Some(true))];
        assert_eq!(watch_set(&[], &listed, None), vec![1, 3]);
        assert_eq!(watch_set(&[1, 2, 3], &[(2, Some(true))], None), vec![2]);
    }

    #[test]
    fn watch_set_keeps_registered_device_on_transient_read_error() {
        let listed = [(1, None), (2, None)];
        assert_eq!(watch_set(&[1], &listed, None), vec![1]);
    }

    #[test]
    fn watch_set_always_includes_default_input_device() {
        let listed = [(1, Some(false)), (2, Some(true))];
        assert_eq!(watch_set(&[], &listed, Some(1)), vec![1, 2]);
        assert_eq!(watch_set(&[], &listed, Some(9)), vec![2]);
    }

    #[test]
    fn removal_error_on_an_unlisted_device_means_gone() {
        assert_eq!(removal_outcome(true, false), RemovalOutcome::Gone);
    }

    #[test]
    fn removal_error_on_a_listed_device_keeps_the_old_behaviour() {
        assert_eq!(removal_outcome(true, true), RemovalOutcome::Orphaned);
        assert_eq!(removal_outcome(false, true), RemovalOutcome::Retry);
    }
}
