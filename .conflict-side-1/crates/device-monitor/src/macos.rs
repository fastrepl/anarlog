use cidre::{core_audio as ca, ns, os};
use std::sync::mpsc;

use crate::{DeviceEvent, DeviceSwitch, DeviceUpdate};
use hypr_audio_device::macos::is_headphone_from_default_output_device;

type ListenerFn = extern "C-unwind" fn(ca::Obj, u32, *const ca::PropAddr, *mut ()) -> os::Status;

const SELECTORS: [ca::PropSelector; 3] = [
    ca::PropSelector::HW_DEFAULT_INPUT_DEVICE,
    ca::PropSelector::HW_DEFAULT_OUTPUT_DEVICE,
    ca::PropSelector::HW_DEVICES,
];

trait EventSender: Clone {
    fn send_switch(&self, switch: DeviceSwitch);
    fn send_update(&self, update: DeviceUpdate);
}

impl EventSender for mpsc::Sender<DeviceSwitch> {
    fn send_switch(&self, switch: DeviceSwitch) {
        let _ = self.send(switch);
    }
    fn send_update(&self, _update: DeviceUpdate) {}
}

impl EventSender for mpsc::Sender<DeviceUpdate> {
    fn send_switch(&self, _switch: DeviceSwitch) {}
    fn send_update(&self, update: DeviceUpdate) {
        let _ = self.send(update);
    }
}

impl EventSender for mpsc::Sender<DeviceEvent> {
    fn send_switch(&self, switch: DeviceSwitch) {
        let _ = self.send(DeviceEvent::Switch(switch));
    }
    fn send_update(&self, update: DeviceUpdate) {
        let _ = self.send(DeviceEvent::Update(update));
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
}

impl ListenerBase {
    fn add(&self, addr: &ca::PropAddr) -> bool {
        self.device
            .add_prop_listener(addr, self.listener, self.ptr)
            .is_ok()
    }

    fn remove(&self, addr: &ca::PropAddr) {
        let _ = self
            .device
            .remove_prop_listener(addr, self.listener, self.ptr);
    }
}

struct OutputListeners {
    base: ListenerBase,
    volume_elements: Vec<ca::PropElement>,
}

impl OutputListeners {
    const fn mute_addr() -> ca::PropAddr {
        ca::PropSelector::DEVICE_MUTE.addr(ca::PropScope::OUTPUT, ca::PropElement::MAIN)
    }

    fn volume_addr(element: ca::PropElement) -> ca::PropAddr {
        ca::PropSelector::DEVICE_VOLUME_SCALAR.addr(ca::PropScope::OUTPUT, element)
    }

    fn new(device: ca::Device, listener: ListenerFn, ptr: *mut ()) -> Option<Self> {
        if device.is_unknown() {
            return None;
        }

        let base = ListenerBase {
            device,
            listener,
            ptr,
        };
        let volume_elements = get_volume_elements(&base.device);

        let volume_ok = volume_elements
            .iter()
            .all(|&e| base.add(&Self::volume_addr(e)));
        if !volume_ok {
            return None;
        }
        base.add(&Self::mute_addr());

        Some(Self {
            base,
            volume_elements,
        })
    }

    fn update(&mut self) {
        self.remove_listeners();

        let Ok(device) = ca::System::default_output_device() else {
            return;
        };
        if device.is_unknown() {
            return;
        }

        self.base.device = device;
        self.volume_elements = get_volume_elements(&self.base.device);

        let volume_ok = self
            .volume_elements
            .iter()
            .all(|&e| self.base.add(&Self::volume_addr(e)));
        if volume_ok {
            self.base.add(&Self::mute_addr());
        } else {
            tracing::error!("device_listener_update_failed");
        }
    }

    fn remove_listeners(&self) {
        for &element in &self.volume_elements {
            self.base.remove(&Self::volume_addr(element));
        }
        self.base.remove(&Self::mute_addr());
    }
}

impl Drop for OutputListeners {
    fn drop(&mut self) {
        self.remove_listeners();
    }
}

struct InputMuteListener {
    base: ListenerBase,
}

impl InputMuteListener {
    const fn mute_addr() -> ca::PropAddr {
        ca::PropSelector::DEVICE_MUTE.addr(ca::PropScope::INPUT, ca::PropElement::MAIN)
    }

    fn new(device: ca::Device, listener: ListenerFn, ptr: *mut ()) -> Option<Self> {
        if device.is_unknown() {
            return None;
        }

        let base = ListenerBase {
            device,
            listener,
            ptr,
        };
        base.add(&Self::mute_addr()).then_some(Self { base })
    }

    fn update(&mut self) {
        self.base.remove(&Self::mute_addr());

        if let Ok(device) = ca::System::default_input_device()
            && !device.is_unknown()
        {
            self.base.device = device;
            self.base.add(&Self::mute_addr());
        }
    }
}

impl Drop for InputMuteListener {
    fn drop(&mut self) {
        self.base.remove(&Self::mute_addr());
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
    event_tx: S,
    update_output_tx: mpsc::Sender<()>,
    update_input_tx: mpsc::Sender<()>,
    listen_switch: bool,
    listen_volume_mute: bool,
}

extern "C-unwind" fn system_listener<S: EventSender>(
    _obj_id: ca::Obj,
    number_addresses: u32,
    addresses: *const ca::PropAddr,
    client_data: *mut (),
) -> os::Status {
    let ctx = unsafe { &*(client_data as *const MonitorContext<S>) };
    let addresses = unsafe { std::slice::from_raw_parts(addresses, number_addresses as usize) };

    for addr in addresses {
        match addr.selector {
            ca::PropSelector::HW_DEFAULT_INPUT_DEVICE => {
                if ctx.listen_switch {
                    ctx.event_tx.send_switch(DeviceSwitch::DefaultInputChanged);
                }
                if ctx.listen_volume_mute {
                    let _ = ctx.update_input_tx.send(());
                }
            }
            ca::PropSelector::HW_DEFAULT_OUTPUT_DEVICE => {
                if ctx.listen_switch {
                    let headphone = is_headphone_from_default_output_device();
                    ctx.event_tx
                        .send_switch(DeviceSwitch::DefaultOutputChanged { headphone });
                }
                if ctx.listen_volume_mute {
                    let _ = ctx.update_output_tx.send(());
                }
            }
            ca::PropSelector::HW_DEVICES => {
                if ctx.listen_switch {
                    ctx.event_tx.send_switch(DeviceSwitch::DeviceListChanged);
                }
                if ctx.listen_volume_mute {
                    let _ = ctx.update_output_tx.send(());
                    let _ = ctx.update_input_tx.send(());
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
    let sender = unsafe { &*(client_data as *const S) };
    let addresses = unsafe { std::slice::from_raw_parts(addresses, number_addresses as usize) };

    for addr in addresses {
        handle_volume_mute_event(sender, addr);
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
    let (update_output_tx, update_output_rx) = mpsc::channel();
    let (update_input_tx, update_input_rx) = mpsc::channel();

    let context = MonitorContext {
        event_tx: event_tx.clone(),
        update_output_tx,
        update_input_tx,
        listen_switch,
        listen_volume_mute,
    };
    let context_ptr = as_ptr(&context);
    let event_tx_ptr = as_ptr(&event_tx);

    for selector in SELECTORS {
        if let Err(e) = ca::System::OBJ.add_prop_listener(
            &selector.global_addr(),
            system_listener::<S>,
            context_ptr,
        ) {
            tracing::error!("system_listener_add_failed: {:?}", e);
            return;
        }
    }

    let (mut output_listeners, mut input_listener) = if listen_volume_mute {
        let output = ca::System::default_output_device()
            .ok()
            .and_then(|d| OutputListeners::new(d, device_listener::<S>, event_tx_ptr));
        let input = ca::System::default_input_device()
            .ok()
            .and_then(|d| InputMuteListener::new(d, device_listener::<S>, event_tx_ptr));
        (output, input)
    } else {
        (None, None)
    };

    tracing::info!("{}_started", name);

    run_event_loop(stop_rx, || {
        if listen_volume_mute {
            if update_output_rx.try_recv().is_ok() {
                if let Some(ref mut l) = output_listeners {
                    l.update();
                } else if let Ok(device) = ca::System::default_output_device() {
                    output_listeners =
                        OutputListeners::new(device, device_listener::<S>, event_tx_ptr);
                }
            }
            if update_input_rx.try_recv().is_ok() {
                if let Some(ref mut l) = input_listener {
                    l.update();
                } else if let Ok(device) = ca::System::default_input_device() {
                    input_listener =
                        InputMuteListener::new(device, device_listener::<S>, event_tx_ptr);
                }
            }
        }
        true
    });

    drop(output_listeners);
    drop(input_listener);

    for selector in SELECTORS {
        let _ = ca::System::OBJ.remove_prop_listener(
            &selector.global_addr(),
            system_listener::<S>,
            context_ptr,
        );
    }

    tracing::info!("{}_stopped", name);
}

pub(crate) fn monitor_device_change(
    event_tx: mpsc::Sender<DeviceSwitch>,
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

pub(crate) fn monitor(event_tx: mpsc::Sender<DeviceEvent>, stop_rx: mpsc::Receiver<()>) {
    monitor_internal(event_tx, stop_rx, true, true, "monitor");
}
