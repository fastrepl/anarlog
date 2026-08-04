use cidre::{core_audio as ca, ns, os};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

use crate::{DeviceEvent, DeviceSwitch, DeviceUpdate};
use anlg_audio_device::macos::is_headphone_from_default_output_device;

type ListenerFn = extern "C-unwind" fn(ca::Obj, u32, *const ca::PropAddr, *mut ()) -> os::Status;

const SELECTORS: [ca::PropSelector; 3] = [
    ca::PropSelector::HW_DEFAULT_INPUT_DEVICE,
    ca::PropSelector::HW_DEFAULT_OUTPUT_DEVICE,
    ca::PropSelector::HW_DEVICES,
];

static CALLBACK_REGISTRATION_POISONED: AtomicBool = AtomicBool::new(false);

trait EventSender: Clone + Send + Sync + 'static {
    fn send_switch(&self, switch: DeviceSwitch) -> bool;
    fn send_update(&self, update: DeviceUpdate);
}

impl EventSender for mpsc::Sender<DeviceSwitch> {
    fn send_switch(&self, switch: DeviceSwitch) -> bool {
        self.send(switch).is_ok()
    }
    fn send_update(&self, _update: DeviceUpdate) {}
}

impl EventSender for mpsc::SyncSender<DeviceSwitch> {
    fn send_switch(&self, switch: DeviceSwitch) -> bool {
        match self.try_send(switch) {
            Ok(()) | Err(mpsc::TrySendError::Full(_)) => true,
            Err(mpsc::TrySendError::Disconnected(_)) => false,
        }
    }
    fn send_update(&self, _update: DeviceUpdate) {}
}

impl EventSender for mpsc::Sender<DeviceUpdate> {
    fn send_switch(&self, _switch: DeviceSwitch) -> bool {
        true
    }
    fn send_update(&self, update: DeviceUpdate) {
        let _ = self.send(update);
    }
}

impl EventSender for mpsc::Sender<DeviceEvent> {
    fn send_switch(&self, switch: DeviceSwitch) -> bool {
        self.send(DeviceEvent::Switch(switch)).is_ok()
    }
    fn send_update(&self, update: DeviceUpdate) {
        let _ = self.send(DeviceEvent::Update(update));
    }
}

impl EventSender for mpsc::SyncSender<DeviceEvent> {
    fn send_switch(&self, switch: DeviceSwitch) -> bool {
        match self.try_send(DeviceEvent::Switch(switch)) {
            Ok(()) | Err(mpsc::TrySendError::Full(_)) => true,
            Err(mpsc::TrySendError::Disconnected(_)) => false,
        }
    }
    fn send_update(&self, update: DeviceUpdate) {
        let _ = self.try_send(DeviceEvent::Update(update));
    }
}

fn as_ptr<T>(value: &T) -> *mut () {
    value as *const T as *mut ()
}

fn run_event_loop<F>(stop_rx: mpsc::Receiver<()>, mut on_tick: F)
where
    F: FnMut() -> bool,
{
    let run_loop = ns::RunLoop::current();

    loop {
        run_loop.run_until_date(&ns::Date::distant_future());

        if !on_tick() || stop_rx.try_recv().is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

fn get_volume_elements(device: &ca::Device) -> Vec<ca::PropElement> {
    let has_volume = |element: ca::PropElement| {
        let addr = ca::PropSelector::DEVICE_VOLUME_SCALAR.addr(ca::PropScope::OUTPUT, element);
        device.prop::<f32>(&addr).is_ok()
    };

    if has_volume(ca::PropElement::MAIN) {
        return vec![ca::PropElement::MAIN];
    }

    (1..=2)
        .map(ca::PropElement)
        .filter(|&e| has_volume(e))
        .collect()
}

struct ListenerBase {
    device: ca::Device,
    listener: ListenerFn,
    ptr: *mut (),
    active: Arc<AtomicBool>,
    cleanup_failed: Arc<AtomicBool>,
}

impl ListenerBase {
    fn add(&self, addr: &ca::PropAddr) -> bool {
        self.device
            .add_prop_listener(addr, self.listener, self.ptr)
            .is_ok()
    }

    fn remove(&self, addr: &ca::PropAddr) {
        if let Err(error) = self
            .device
            .remove_prop_listener(addr, self.listener, self.ptr)
        {
            tracing::error!(?error, "device_listener_remove_failed");
            deactivate_callbacks(&self.active);
            self.cleanup_failed.store(true, Ordering::Release);
            CALLBACK_REGISTRATION_POISONED.store(true, Ordering::Release);
        }
    }
}

struct OutputListeners {
    base: ListenerBase,
    volume_elements: Vec<ca::PropElement>,
    mute_registered: bool,
}

impl OutputListeners {
    const fn mute_addr() -> ca::PropAddr {
        ca::PropSelector::DEVICE_MUTE.addr(ca::PropScope::OUTPUT, ca::PropElement::MAIN)
    }

    fn volume_addr(element: ca::PropElement) -> ca::PropAddr {
        ca::PropSelector::DEVICE_VOLUME_SCALAR.addr(ca::PropScope::OUTPUT, element)
    }

    fn new(
        device: ca::Device,
        listener: ListenerFn,
        ptr: *mut (),
        active: Arc<AtomicBool>,
        cleanup_failed: Arc<AtomicBool>,
    ) -> Option<Self> {
        if device.is_unknown() {
            return None;
        }

        let base = ListenerBase {
            device,
            listener,
            ptr,
            active,
            cleanup_failed,
        };
        let mut volume_elements = Vec::new();
        for element in get_volume_elements(&base.device) {
            if !base.add(&Self::volume_addr(element)) {
                for registered in volume_elements.drain(..) {
                    base.remove(&Self::volume_addr(registered));
                }
                return None;
            }
            volume_elements.push(element);
        }
        let mute_registered = base.add(&Self::mute_addr());

        Some(Self {
            base,
            volume_elements,
            mute_registered,
        })
    }

    fn update(&mut self) {
        self.remove_listeners();
        if self.base.cleanup_failed.load(Ordering::Acquire) {
            return;
        }

        let Ok(device) = ca::System::default_output_device() else {
            return;
        };
        if device.is_unknown() {
            return;
        }

        self.base.device = device;
        for element in get_volume_elements(&self.base.device) {
            if !self.base.add(&Self::volume_addr(element)) {
                self.remove_listeners();
                tracing::error!("device_listener_update_failed");
                return;
            }
            self.volume_elements.push(element);
        }
        self.mute_registered = self.base.add(&Self::mute_addr());
    }

    fn remove_listeners(&mut self) {
        for element in self.volume_elements.drain(..) {
            self.base.remove(&Self::volume_addr(element));
        }
        if self.mute_registered {
            self.base.remove(&Self::mute_addr());
            self.mute_registered = false;
        }
    }
}

impl Drop for OutputListeners {
    fn drop(&mut self) {
        self.remove_listeners();
    }
}

struct InputMuteListener {
    base: ListenerBase,
    registered: bool,
}

impl InputMuteListener {
    const fn mute_addr() -> ca::PropAddr {
        ca::PropSelector::DEVICE_MUTE.addr(ca::PropScope::INPUT, ca::PropElement::MAIN)
    }

    fn new(
        device: ca::Device,
        listener: ListenerFn,
        ptr: *mut (),
        active: Arc<AtomicBool>,
        cleanup_failed: Arc<AtomicBool>,
    ) -> Option<Self> {
        if device.is_unknown() {
            return None;
        }

        let base = ListenerBase {
            device,
            listener,
            ptr,
            active,
            cleanup_failed,
        };
        base.add(&Self::mute_addr()).then_some(Self {
            base,
            registered: true,
        })
    }

    fn update(&mut self) {
        self.remove_listener();
        if self.base.cleanup_failed.load(Ordering::Acquire) {
            return;
        }

        if let Ok(device) = ca::System::default_input_device()
            && !device.is_unknown()
        {
            self.base.device = device;
            self.registered = self.base.add(&Self::mute_addr());
        }
    }

    fn remove_listener(&mut self) {
        if self.registered {
            self.base.remove(&Self::mute_addr());
            self.registered = false;
        }
    }
}

impl Drop for InputMuteListener {
    fn drop(&mut self) {
        self.remove_listener();
    }
}

fn send_volume_update<S: EventSender>(sender: &S, device: &ca::Device, element: ca::PropElement) {
    let addr = ca::PropSelector::DEVICE_VOLUME_SCALAR.addr(ca::PropScope::OUTPUT, element);
    if let Ok(uid) = device.uid()
        && let Ok(volume) = device.prop::<f32>(&addr)
    {
        sender.send_update(DeviceUpdate::VolumeChanged {
            device_uid: uid.to_string(),
            volume,
        });
    }
}

fn send_mute_update<S: EventSender>(
    sender: &S,
    device: &ca::Device,
    selector: ca::PropSelector,
    scope: ca::PropScope,
    element: ca::PropElement,
) {
    let addr = selector.addr(scope, element);
    if let Ok(uid) = device.uid()
        && let Ok(mute_value) = device.prop::<u32>(&addr)
    {
        sender.send_update(DeviceUpdate::MuteChanged {
            device_uid: uid.to_string(),
            is_muted: mute_value != 0,
        });
    }
}

fn handle_volume_mute_event<S: EventSender>(sender: &S, addr: &ca::PropAddr) {
    match addr.selector {
        ca::PropSelector::DEVICE_VOLUME_SCALAR => {
            if let Ok(device) = ca::System::default_output_device() {
                send_volume_update(sender, &device, addr.element);
            }
        }
        ca::PropSelector::DEVICE_MUTE => {
            if addr.scope == ca::PropScope::OUTPUT {
                if let Ok(device) = ca::System::default_output_device() {
                    send_mute_update(
                        sender,
                        &device,
                        ca::PropSelector::DEVICE_MUTE,
                        ca::PropScope::OUTPUT,
                        addr.element,
                    );
                }
            } else if addr.scope == ca::PropScope::INPUT
                && let Ok(device) = ca::System::default_input_device()
            {
                send_mute_update(
                    sender,
                    &device,
                    ca::PropSelector::DEVICE_MUTE,
                    ca::PropScope::INPUT,
                    addr.element,
                );
            }
        }
        _ => {}
    }
}

struct MonitorContext<S> {
    active: Arc<AtomicBool>,
    event_tx: S,
    update_output_pending: AtomicBool,
    update_input_pending: AtomicBool,
    listen_switch: bool,
    listen_volume_mute: bool,
}

fn callbacks_active(active: &AtomicBool) -> bool {
    active.load(Ordering::Acquire)
}

fn deactivate_callbacks(active: &AtomicBool) {
    active.store(false, Ordering::Release);
}

fn request_update(pending: &AtomicBool) {
    pending.store(true, Ordering::Release);
}

fn take_update(pending: &AtomicBool) -> bool {
    pending.swap(false, Ordering::AcqRel)
}

extern "C-unwind" fn system_listener<S: EventSender>(
    _obj_id: ca::Obj,
    number_addresses: u32,
    addresses: *const ca::PropAddr,
    client_data: *mut (),
) -> os::Status {
    let ctx = unsafe { &*(client_data as *const MonitorContext<S>) };
    if !callbacks_active(&ctx.active) {
        return os::Status::NO_ERR;
    }
    let addresses = unsafe { std::slice::from_raw_parts(addresses, number_addresses as usize) };

    for addr in addresses {
        match addr.selector {
            ca::PropSelector::HW_DEFAULT_INPUT_DEVICE => {
                if ctx.listen_switch && !ctx.event_tx.send_switch(DeviceSwitch::DefaultInputChanged)
                {
                    deactivate_callbacks(&ctx.active);
                    return os::Status::NO_ERR;
                }
                if ctx.listen_volume_mute {
                    request_update(&ctx.update_input_pending);
                }
            }
            ca::PropSelector::HW_DEFAULT_OUTPUT_DEVICE => {
                if ctx.listen_switch {
                    let headphone = is_headphone_from_default_output_device();
                    if !ctx
                        .event_tx
                        .send_switch(DeviceSwitch::DefaultOutputChanged { headphone })
                    {
                        deactivate_callbacks(&ctx.active);
                        return os::Status::NO_ERR;
                    }
                }
                if ctx.listen_volume_mute {
                    request_update(&ctx.update_output_pending);
                }
            }
            ca::PropSelector::HW_DEVICES => {
                if ctx.listen_switch && !ctx.event_tx.send_switch(DeviceSwitch::DeviceListChanged) {
                    deactivate_callbacks(&ctx.active);
                    return os::Status::NO_ERR;
                }
                if ctx.listen_volume_mute {
                    request_update(&ctx.update_output_pending);
                    request_update(&ctx.update_input_pending);
                }
            }
            _ => {}
        }
    }
    os::Status::NO_ERR
}

extern "C-unwind" fn device_listener<S: EventSender>(
    _obj_id: ca::Obj,
    number_addresses: u32,
    addresses: *const ca::PropAddr,
    client_data: *mut (),
) -> os::Status {
    let ctx = unsafe { &*(client_data as *const MonitorContext<S>) };
    if !callbacks_active(&ctx.active) {
        return os::Status::NO_ERR;
    }
    let addresses = unsafe { std::slice::from_raw_parts(addresses, number_addresses as usize) };

    for addr in addresses {
        handle_volume_mute_event(&ctx.event_tx, addr);
    }
    os::Status::NO_ERR
}

fn monitor_internal<S: EventSender>(
    event_tx: S,
    stop_rx: mpsc::Receiver<()>,
    listen_switch: bool,
    listen_volume_mute: bool,
    name: &str,
) {
    if CALLBACK_REGISTRATION_POISONED.load(Ordering::Acquire) {
        tracing::error!("device_monitor_callbacks_poisoned");
        return;
    }

    let active = Arc::new(AtomicBool::new(true));

    let callback_context = Box::new(MonitorContext {
        active: active.clone(),
        event_tx,
        update_output_pending: AtomicBool::new(false),
        update_input_pending: AtomicBool::new(false),
        listen_switch,
        listen_volume_mute,
    });
    let context_ptr = as_ptr(callback_context.as_ref());
    let cleanup_failed = Arc::new(AtomicBool::new(false));
    let mut registered_selectors: Vec<ca::PropSelector> = Vec::new();

    for selector in SELECTORS {
        if let Err(e) = ca::System::OBJ.add_prop_listener(
            &selector.global_addr(),
            system_listener::<S>,
            context_ptr,
        ) {
            tracing::error!("system_listener_add_failed: {:?}", e);
            deactivate_callbacks(&active);
            for registered in registered_selectors.drain(..) {
                if let Err(error) = ca::System::OBJ.remove_prop_listener(
                    &registered.global_addr(),
                    system_listener::<S>,
                    context_ptr,
                ) {
                    tracing::error!(?error, "system_listener_rollback_failed");
                    cleanup_failed.store(true, Ordering::Release);
                    CALLBACK_REGISTRATION_POISONED.store(true, Ordering::Release);
                }
            }
            if cleanup_failed.load(Ordering::Acquire) {
                std::mem::forget(callback_context);
            }
            return;
        }
        registered_selectors.push(selector);
    }

    let (mut output_listeners, mut input_listener) = if listen_volume_mute {
        let output = ca::System::default_output_device().ok().and_then(|d| {
            OutputListeners::new(
                d,
                device_listener::<S>,
                context_ptr,
                active.clone(),
                cleanup_failed.clone(),
            )
        });
        let input = ca::System::default_input_device().ok().and_then(|d| {
            InputMuteListener::new(
                d,
                device_listener::<S>,
                context_ptr,
                active.clone(),
                cleanup_failed.clone(),
            )
        });
        (output, input)
    } else {
        (None, None)
    };

    tracing::info!("{}_started", name);

    run_event_loop(stop_rx, || {
        if cleanup_failed.load(Ordering::Acquire) {
            return false;
        }

        if listen_volume_mute {
            if take_update(&callback_context.update_output_pending) {
                if let Some(ref mut l) = output_listeners {
                    l.update();
                } else if let Ok(device) = ca::System::default_output_device() {
                    output_listeners = OutputListeners::new(
                        device,
                        device_listener::<S>,
                        context_ptr,
                        active.clone(),
                        cleanup_failed.clone(),
                    );
                }
            }
            if take_update(&callback_context.update_input_pending) {
                if let Some(ref mut l) = input_listener {
                    l.update();
                } else if let Ok(device) = ca::System::default_input_device() {
                    input_listener = InputMuteListener::new(
                        device,
                        device_listener::<S>,
                        context_ptr,
                        active.clone(),
                        cleanup_failed.clone(),
                    );
                }
            }
        }
        !cleanup_failed.load(Ordering::Acquire)
    });

    deactivate_callbacks(&active);
    drop(output_listeners);
    drop(input_listener);

    for selector in registered_selectors {
        if let Err(error) = ca::System::OBJ.remove_prop_listener(
            &selector.global_addr(),
            system_listener::<S>,
            context_ptr,
        ) {
            tracing::error!(?error, "system_listener_remove_failed");
            cleanup_failed.store(true, Ordering::Release);
            CALLBACK_REGISTRATION_POISONED.store(true, Ordering::Release);
        }
    }

    if cleanup_failed.load(Ordering::Acquire) {
        tracing::error!("device_monitor_callback_context_retained");
        std::mem::forget(callback_context);
    }

    tracing::info!("{}_stopped", name);
}

pub(crate) fn monitor_device_change(
    event_tx: mpsc::SyncSender<DeviceSwitch>,
    stop_rx: mpsc::Receiver<()>,
) {
    monitor_internal(event_tx, stop_rx, true, false, "monitor_device_change");
}

pub(crate) fn monitor_volume_mute(
    event_tx: mpsc::Sender<DeviceUpdate>,
    stop_rx: mpsc::Receiver<()>,
) {
    monitor_internal(event_tx, stop_rx, false, true, "monitor_volume_mute");
}

pub(crate) fn monitor(event_tx: mpsc::SyncSender<DeviceEvent>, stop_rx: mpsc::Receiver<()>) {
    monitor_internal(event_tx, stop_rx, true, true, "monitor");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_activity_stays_disabled_after_teardown() {
        let active = AtomicBool::new(true);
        assert!(callbacks_active(&active));

        deactivate_callbacks(&active);

        assert!(!callbacks_active(&active));
    }

    #[test]
    fn repeated_update_requests_are_coalesced() {
        let pending = AtomicBool::new(false);
        request_update(&pending);
        request_update(&pending);

        assert!(take_update(&pending));
        assert!(!take_update(&pending));
    }

    #[test]
    fn bounded_switch_sender_treats_full_as_coalesced_and_disconnect_as_inactive() {
        let (tx, rx) = mpsc::sync_channel::<DeviceSwitch>(1);
        assert!(tx.send_switch(DeviceSwitch::DefaultInputChanged));
        assert!(tx.send_switch(DeviceSwitch::DeviceListChanged));
        drop(rx);
        assert!(!tx.send_switch(DeviceSwitch::DefaultInputChanged));
    }
}
