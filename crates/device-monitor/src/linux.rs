use crate::{DeviceEvent, DeviceSwitch, DeviceUpdate};
use libpulse_binding::{
    context::{
        Context, FlagSet as ContextFlagSet,
        subscribe::{Facility, InterestMaskSet, Operation},
    },
    mainloop::threaded::Mainloop,
    proplist::Proplist,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

type PulseAudioHandles = (Rc<RefCell<Mainloop>>, Rc<RefCell<Context>>);

fn is_headphone_from_default_output_device() -> Option<bool> {
    hypr_audio_device::linux::is_headphone_from_default_output_device()
}

fn setup_pulseaudio(stop_rx: &mpsc::Receiver<()>) -> Option<PulseAudioHandles> {
    let mut proplist = match Proplist::new() {
        Some(p) => p,
        None => {
            tracing::error!("Failed to create PulseAudio proplist");
            let _ = stop_rx.recv();
            return None;
        }
    };

    if proplist
        .set_str(
            libpulse_binding::proplist::properties::APPLICATION_NAME,
            "Char Device Monitor",
        )
        .is_err()
    {
        tracing::error!("Failed to set PulseAudio application name");
        let _ = stop_rx.recv();
        return None;
    }

    let mainloop = match Mainloop::new() {
        Some(m) => Rc::new(RefCell::new(m)),
        None => {
            tracing::error!("Failed to create PulseAudio mainloop");
            let _ = stop_rx.recv();
            return None;
        }
    };

    let context =
        match Context::new_with_proplist(&*mainloop.borrow(), "CharContext", &proplist) {
            Some(c) => Rc::new(RefCell::new(c)),
            None => {
                tracing::error!("Failed to create PulseAudio context");
                let _ = stop_rx.recv();
                return None;
            }
        };

    if let Err(e) = context
        .borrow_mut()
        .connect(None, ContextFlagSet::NOFLAGS, None)
    {
        tracing::error!("Failed to connect to PulseAudio: {:?}", e);
        let _ = stop_rx.recv();
        return None;
    }

    mainloop.borrow_mut().lock();

    if let Err(e) = mainloop.borrow_mut().start() {
        tracing::error!("Failed to start PulseAudio mainloop: {:?}", e);
        mainloop.borrow_mut().unlock();
        let _ = stop_rx.recv();
        return None;
    }

    loop {
        match context.borrow().get_state() {
            libpulse_binding::context::State::Ready => {
                tracing::info!("PulseAudio context ready");
                break;
            }
            libpulse_binding::context::State::Failed
            | libpulse_binding::context::State::Terminated => {
                tracing::error!("PulseAudio context failed");
                mainloop.borrow_mut().unlock();
                return None;
            }
            _ => {
                mainloop.borrow_mut().unlock();
                std::thread::sleep(std::time::Duration::from_millis(50));
                mainloop.borrow_mut().lock();
            }
        }
    }

    Some((mainloop, context))
}

fn cleanup_pulseaudio(mainloop: Rc<RefCell<Mainloop>>, context: Rc<RefCell<Context>>) {
    mainloop.borrow_mut().lock();
    context.borrow_mut().disconnect();
    mainloop.borrow_mut().unlock();
    mainloop.borrow_mut().stop();
}

pub(crate) fn monitor_device_change(
    event_tx: mpsc::Sender<DeviceSwitch>,
    stop_rx: mpsc::Receiver<()>,
) {
    let Some((mainloop, context)) = setup_pulseaudio(&stop_rx) else {
        return;
    };

    context.borrow_mut().subscribe(
        InterestMaskSet::SINK | InterestMaskSet::SOURCE | InterestMaskSet::SERVER,
        |success| {
            if !success {
                tracing::error!("Failed to subscribe to PulseAudio events");
            }
        },
    );

    let event_tx_for_callback = event_tx.clone();
    context.borrow_mut().set_subscribe_callback(Some(Box::new(
        move |facility, operation, _index| match (facility, operation) {
            (Some(Facility::Server), Some(Operation::Changed)) => {
                let _ = event_tx_for_callback.send(DeviceSwitch::DefaultInputChanged);
                let _ = event_tx_for_callback.send(DeviceSwitch::DefaultOutputChanged {
                    headphone: is_headphone_from_default_output_device(),
                });
            }
            (Some(Facility::Sink), Some(Operation::Changed)) => {
                let _ = event_tx_for_callback.send(DeviceSwitch::DefaultOutputChanged {
                    headphone: is_headphone_from_default_output_device(),
                });
            }
            (Some(Facility::Sink), Some(Operation::New | Operation::Removed)) => {
                let _ = event_tx_for_callback.send(DeviceSwitch::DeviceListChanged);
            }
            (Some(Facility::Source), Some(Operation::Changed)) => {
                let _ = event_tx_for_callback.send(DeviceSwitch::DefaultInputChanged);
            }
            (Some(Facility::Source), Some(Operation::New | Operation::Removed)) => {
                let _ = event_tx_for_callback.send(DeviceSwitch::DeviceListChanged);
            }
            _ => {}
        },
    )));

    mainloop.borrow_mut().unlock();

    tracing::info!("monitor_device_change_started");

    let _ = stop_rx.recv();

    cleanup_pulseaudio(mainloop, context);

    tracing::info!("monitor_device_change_stopped");
}

pub(crate) fn monitor_volume_mute(
    _event_tx: mpsc::Sender<DeviceUpdate>,
    stop_rx: mpsc::Receiver<()>,
) {
    tracing::warn!("volume_mute_monitoring_unsupported_on_linux");
    let _ = stop_rx.recv();
}

pub(crate) fn monitor(event_tx: mpsc::Sender<DeviceEvent>, stop_rx: mpsc::Receiver<()>) {
    let Some((mainloop, context)) = setup_pulseaudio(&stop_rx) else {
        return;
    };

    context.borrow_mut().subscribe(
        InterestMaskSet::SINK | InterestMaskSet::SOURCE | InterestMaskSet::SERVER,
        |success| {
            if !success {
                tracing::error!("Failed to subscribe to PulseAudio events");
            }
        },
    );

    let event_tx_for_callback = event_tx.clone();
    context.borrow_mut().set_subscribe_callback(Some(Box::new(
        move |facility, operation, _index| match (facility, operation) {
            (Some(Facility::Server), Some(Operation::Changed)) => {
                let _ = event_tx_for_callback
                    .send(DeviceEvent::Switch(DeviceSwitch::DefaultInputChanged));
                let _ = event_tx_for_callback.send(DeviceEvent::Switch(
                    DeviceSwitch::DefaultOutputChanged {
                        headphone: is_headphone_from_default_output_device(),
                    },
                ));
            }
            (Some(Facility::Sink), Some(Operation::Changed)) => {
                let _ = event_tx_for_callback.send(DeviceEvent::Switch(
                    DeviceSwitch::DefaultOutputChanged {
                        headphone: is_headphone_from_default_output_device(),
                    },
                ));
            }
            (Some(Facility::Sink), Some(Operation::New | Operation::Removed)) => {
                let _ = event_tx_for_callback
                    .send(DeviceEvent::Switch(DeviceSwitch::DeviceListChanged));
            }
            (Some(Facility::Source), Some(Operation::Changed)) => {
                let _ = event_tx_for_callback
                    .send(DeviceEvent::Switch(DeviceSwitch::DefaultInputChanged));
            }
            (Some(Facility::Source), Some(Operation::New | Operation::Removed)) => {
                let _ = event_tx_for_callback
                    .send(DeviceEvent::Switch(DeviceSwitch::DeviceListChanged));
            }
            _ => {}
        },
    )));

    mainloop.borrow_mut().unlock();

    tracing::info!("monitor_started");

    let _ = stop_rx.recv();

    cleanup_pulseaudio(mainloop, context);

    tracing::info!("monitor_stopped");
}
