use crate::{DeviceEvent, DeviceSwitch, DeviceUpdate};
use std::sync::mpsc;

pub(crate) fn monitor_device_change(
    _event_tx: mpsc::Sender<DeviceSwitch>,
    stop_rx: mpsc::Receiver<()>,
) {
    tracing::warn!("device_monitoring_unsupported_on_windows");
    let _ = stop_rx.recv();
}

pub(crate) fn monitor_volume_mute(
    _event_tx: mpsc::Sender<DeviceUpdate>,
    stop_rx: mpsc::Receiver<()>,
) {
    tracing::warn!("device_monitoring_unsupported_on_windows");
    let _ = stop_rx.recv();
}

pub(crate) fn monitor(_event_tx: mpsc::Sender<DeviceEvent>, stop_rx: mpsc::Receiver<()>) {
    tracing::warn!("device_monitoring_unsupported_on_windows");
    let _ = stop_rx.recv();
}
