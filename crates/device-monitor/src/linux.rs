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
type DeviceSwitchEmit = Rc<dyn Fn(DeviceSwitch)>;

#[derive(Debug, Default, PartialEq, Eq)]
struct DefaultDeviceChanges {
    source_changed: bool,
    sink_changed: bool,
}

#[derive(Debug, Default)]
struct DefaultDevices {
    source: Option<String>,
    sink: Option<String>,
}

impl DefaultDevices {
    fn observe(&mut self, source: Option<&str>, sink: Option<&str>) -> DefaultDeviceChanges {
        DefaultDeviceChanges {
            source_changed: Self::observe_name(&mut self.source, source),
            sink_changed: Self::observe_name(&mut self.sink, sink),
        }
    }

    fn observe_name(stored: &mut Option<String>, incoming: Option<&str>) -> bool {
        let Some(incoming) = incoming else {
            return false;
        };
        match stored.as_deref() {
            Some(previous) if previous != incoming => {
                *stored = Some(incoming.to_owned());
                true
            }
            None => {
                *stored = Some(incoming.to_owned());
                false
            }
            Some(_) => false,
        }
    }
}

fn is_headphone_from_default_output_device() -> Option<bool> {
    anlg_audio_device::linux::is_headphone_from_default_output_device()
}

fn setup_pulseaudio(stop_rx: &mpsc::Receiver<()>) -> Option<PulseAudioHandles> {
    let mut proplist = match Proplist::new() {
        Some(p) => p,
        None => {
            tracing::warn!("Failed to create PulseAudio proplist");
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
        tracing::warn!("Failed to set PulseAudio application name");
        let _ = stop_rx.recv();
        return None;
    }

    let mainloop = match Mainloop::new() {
        Some(m) => Rc::new(RefCell::new(m)),
        None => {
            tracing::warn!("Failed to create PulseAudio mainloop");
            let _ = stop_rx.recv();
            return None;
        }
    };

    let context = match Context::new_with_proplist(&*mainloop.borrow(), "AnarlogContext", &proplist)
    {
        Some(c) => Rc::new(RefCell::new(c)),
        None => {
            tracing::warn!("Failed to create PulseAudio context");
            let _ = stop_rx.recv();
            return None;
        }
    };

    if let Err(e) = context
        .borrow_mut()
        .connect(None, ContextFlagSet::NOFLAGS, None)
    {
        tracing::warn!("Failed to connect to PulseAudio: {:?}", e);
        let _ = stop_rx.recv();
        return None;
    }

    mainloop.borrow_mut().lock();

    if let Err(e) = mainloop.borrow_mut().start() {
        tracing::warn!("Failed to start PulseAudio mainloop: {:?}", e);
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
                tracing::warn!("PulseAudio context failed");
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

fn refresh_default_devices(
    context: &Rc<RefCell<Context>>,
    tracker: &Rc<RefCell<DefaultDevices>>,
    emit: &DeviceSwitchEmit,
) {
    let Ok(ctx) = context.try_borrow() else {
        return;
    };
    let tracker = Rc::clone(tracker);
    let emit = Rc::clone(emit);
    ctx.introspect().get_server_info(move |info| {
        let source = info.default_source_name.as_deref();
        let sink = info.default_sink_name.as_deref();
        let changes = tracker.borrow_mut().observe(source, sink);
        if changes.source_changed {
            tracing::info!(
                anarlog.audio.default_source = source.unwrap_or(""),
                "default_source_changed"
            );
            emit(DeviceSwitch::DefaultInputChanged);
        }
        if changes.sink_changed {
            tracing::info!(
                anarlog.audio.default_sink = sink.unwrap_or(""),
                "default_sink_changed"
            );
            emit(DeviceSwitch::DefaultOutputChanged {
                headphone: is_headphone_from_default_output_device(),
            });
        }
    });
}

fn subscribe_pulse_device_events(context: &Rc<RefCell<Context>>, emit: DeviceSwitchEmit) {
    let tracker = Rc::new(RefCell::new(DefaultDevices::default()));
    let context_for_callback = Rc::clone(context);
    let emit_for_callback = Rc::clone(&emit);
    let tracker_for_callback = Rc::clone(&tracker);

    context.borrow_mut().subscribe(
        InterestMaskSet::SINK | InterestMaskSet::SOURCE | InterestMaskSet::SERVER,
        |success| {
            if !success {
                tracing::error!("Failed to subscribe to PulseAudio events");
            }
        },
    );

    context.borrow_mut().set_subscribe_callback(Some(Box::new(
        move |facility, operation, _index| match (facility, operation) {
            (Some(Facility::Sink), Some(Operation::New | Operation::Removed))
            | (Some(Facility::Source), Some(Operation::New | Operation::Removed)) => {
                emit_for_callback(DeviceSwitch::DeviceListChanged);
                refresh_default_devices(
                    &context_for_callback,
                    &tracker_for_callback,
                    &emit_for_callback,
                );
            }
            (Some(Facility::Server), Some(Operation::Changed))
            | (Some(Facility::Sink), Some(Operation::Changed))
            | (Some(Facility::Source), Some(Operation::Changed)) => {
                refresh_default_devices(
                    &context_for_callback,
                    &tracker_for_callback,
                    &emit_for_callback,
                );
            }
            _ => {}
        },
    )));

    refresh_default_devices(context, &tracker, &emit);
}

pub(crate) fn monitor_device_change(
    event_tx: mpsc::SyncSender<DeviceSwitch>,
    stop_rx: mpsc::Receiver<()>,
) {
    let Some((mainloop, context)) = setup_pulseaudio(&stop_rx) else {
        return;
    };

    let emit: DeviceSwitchEmit = Rc::new(move |switch| {
        let _ = event_tx.try_send(switch);
    });
    subscribe_pulse_device_events(&context, emit);

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

pub(crate) fn monitor(event_tx: mpsc::SyncSender<DeviceEvent>, stop_rx: mpsc::Receiver<()>) {
    let Some((mainloop, context)) = setup_pulseaudio(&stop_rx) else {
        return;
    };

    let emit: DeviceSwitchEmit = Rc::new(move |switch| {
        let _ = event_tx.try_send(DeviceEvent::Switch(switch));
    });
    subscribe_pulse_device_events(&context, emit);

    mainloop.borrow_mut().unlock();

    tracing::info!("monitor_started");

    let _ = stop_rx.recv();

    cleanup_pulseaudio(mainloop, context);

    tracing::info!("monitor_stopped");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_observe_establishes_baseline_without_emitting() {
        let mut devices = DefaultDevices::default();

        assert_eq!(
            devices.observe(Some("qa_mic_bus.monitor"), Some("qa_system")),
            DefaultDeviceChanges::default()
        );
    }

    #[test]
    fn source_property_change_does_not_count_as_default_input_change() {
        let mut devices = DefaultDevices::default();
        devices.observe(Some("qa_mic_bus.monitor"), Some("qa_system"));

        assert_eq!(
            devices.observe(Some("qa_mic_bus.monitor"), Some("qa_system")),
            DefaultDeviceChanges::default()
        );
    }

    #[test]
    fn default_source_change_emits_only_source() {
        let mut devices = DefaultDevices::default();
        devices.observe(Some("qa_mic_bus.monitor"), Some("qa_system"));

        assert_eq!(
            devices.observe(Some("alsa_input.usb"), Some("qa_system")),
            DefaultDeviceChanges {
                source_changed: true,
                sink_changed: false,
            }
        );
    }

    #[test]
    fn default_sink_change_emits_only_sink() {
        let mut devices = DefaultDevices::default();
        devices.observe(Some("qa_mic_bus.monitor"), Some("qa_system"));

        assert_eq!(
            devices.observe(Some("qa_mic_bus.monitor"), Some("alsa_output.usb")),
            DefaultDeviceChanges {
                source_changed: false,
                sink_changed: true,
            }
        );
    }

    #[test]
    fn missing_defaults_after_baseline_do_not_emit() {
        let mut devices = DefaultDevices::default();
        devices.observe(Some("qa_mic_bus.monitor"), Some("qa_system"));

        assert_eq!(devices.observe(None, None), DefaultDeviceChanges::default());
    }

    #[test]
    fn null_defaults_do_not_clear_baseline() {
        let mut devices = DefaultDevices::default();
        devices.observe(Some("qa_mic_bus.monitor"), Some("qa_system"));
        devices.observe(None, None);

        assert_eq!(
            devices.observe(Some("qa_mic_bus.monitor"), Some("qa_system")),
            DefaultDeviceChanges::default()
        );
    }

    #[test]
    fn first_real_names_after_null_observation_are_baseline() {
        let mut devices = DefaultDevices::default();
        devices.observe(None, None);

        assert_eq!(
            devices.observe(Some("qa_mic_bus.monitor"), Some("qa_system")),
            DefaultDeviceChanges::default()
        );
    }

    #[test]
    fn default_change_after_null_refresh_still_emits() {
        let mut devices = DefaultDevices::default();
        devices.observe(Some("qa_mic_bus.monitor"), Some("qa_system"));
        devices.observe(None, None);

        assert_eq!(
            devices.observe(Some("alsa_input.usb"), Some("qa_system")),
            DefaultDeviceChanges {
                source_changed: true,
                sink_changed: false,
            }
        );
    }
}
