use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

mod debounce;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[derive(Debug, Clone)]
pub enum DeviceSwitch {
    DefaultInputChanged,
    DefaultOutputChanged { headphone: Option<bool> },
    DeviceListChanged,
}

#[derive(Debug, Clone)]
pub enum DeviceUpdate {
    VolumeChanged { device_uid: String, volume: f32 },
    MuteChanged { device_uid: String, is_muted: bool },
}

#[derive(Debug, Clone)]
pub enum DeviceEvent {
    Switch(DeviceSwitch),
    Update(DeviceUpdate),
}

pub(crate) enum EventOutput<T> {
    Unbounded(mpsc::Sender<T>),
    Bounded(mpsc::SyncSender<T>),
}

impl<T> EventOutput<T> {
    fn send(&self, event: T) -> Result<(), ()> {
        match self {
            Self::Unbounded(tx) => tx.send(event).map_err(|_| ()),
            Self::Bounded(tx) => tx.send(event).map_err(|_| ()),
        }
    }
}

impl<T> From<mpsc::Sender<T>> for EventOutput<T> {
    fn from(tx: mpsc::Sender<T>) -> Self {
        Self::Unbounded(tx)
    }
}

impl<T> From<mpsc::SyncSender<T>> for EventOutput<T> {
    fn from(tx: mpsc::SyncSender<T>) -> Self {
        Self::Bounded(tx)
    }
}

pub struct DeviceMonitorHandle {
    stop_tx: Option<mpsc::Sender<()>>,
    thread_handle: Option<JoinHandle<()>>,
}

impl DeviceMonitorHandle {
    pub fn stop(mut self) {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for DeviceMonitorHandle {
    fn drop(&mut self) {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

pub const DEFAULT_DEBOUNCE_DELAY: Duration = Duration::from_millis(1000);
const MAX_RAW_SWITCH_EVENTS: usize = 3;
const MAX_RAW_DEVICE_EVENTS: usize = 64;

pub struct DeviceSwitchMonitor;

impl DeviceSwitchMonitor {
    pub fn spawn(event_tx: mpsc::Sender<DeviceSwitch>) -> DeviceMonitorHandle {
        Self::spawn_with_debounce(event_tx, None)
    }

    pub fn spawn_debounced(event_tx: mpsc::Sender<DeviceSwitch>) -> DeviceMonitorHandle {
        Self::spawn_with_debounce(event_tx, Some(DEFAULT_DEBOUNCE_DELAY))
    }

    pub fn spawn_debounced_bounded(
        event_tx: mpsc::SyncSender<DeviceSwitch>,
    ) -> DeviceMonitorHandle {
        Self::spawn_with_output(event_tx.into(), Some(DEFAULT_DEBOUNCE_DELAY))
    }

    pub fn spawn_with_debounce(
        event_tx: mpsc::Sender<DeviceSwitch>,
        debounce_delay: Option<Duration>,
    ) -> DeviceMonitorHandle {
        Self::spawn_with_output(event_tx.into(), debounce_delay)
    }

    fn spawn_with_output(
        event_tx: EventOutput<DeviceSwitch>,
        debounce_delay: Option<Duration>,
    ) -> DeviceMonitorHandle {
        let (stop_tx, stop_rx) = mpsc::channel();
        let (raw_tx, raw_rx) = mpsc::sync_channel(MAX_RAW_SWITCH_EVENTS);

        match debounce_delay {
            Some(delay) => {
                debounce::spawn_debounced_by_key(delay, raw_rx, event_tx, |switch| match switch {
                    DeviceSwitch::DefaultInputChanged => 0u8,
                    DeviceSwitch::DefaultOutputChanged { .. } => 1u8,
                    DeviceSwitch::DeviceListChanged => 2u8,
                });
            }
            None => debounce::spawn_forwarder(raw_rx, event_tx),
        }

        let thread_handle = std::thread::spawn(move || {
            #[cfg(target_os = "macos")]
            {
                macos::monitor_device_change(raw_tx, stop_rx);
            }

            #[cfg(target_os = "linux")]
            {
                linux::monitor_device_change(raw_tx, stop_rx);
            }

            #[cfg(target_os = "windows")]
            {
                windows::monitor_device_change(raw_tx, stop_rx);
            }

            #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
            {
                let _ = raw_tx;
                tracing::warn!("device_monitoring_unsupported");
                let _ = stop_rx.recv();
            }
        });

        DeviceMonitorHandle {
            stop_tx: Some(stop_tx),
            thread_handle: Some(thread_handle),
        }
    }
}

pub struct DeviceUpdateMonitor;

impl DeviceUpdateMonitor {
    pub fn spawn(event_tx: mpsc::Sender<DeviceUpdate>) -> DeviceMonitorHandle {
        let (stop_tx, stop_rx) = mpsc::channel();

        let thread_handle = std::thread::spawn(move || {
            #[cfg(target_os = "macos")]
            {
                macos::monitor_volume_mute(event_tx, stop_rx);
            }

            #[cfg(target_os = "linux")]
            {
                linux::monitor_volume_mute(event_tx, stop_rx);
            }

            #[cfg(target_os = "windows")]
            {
                windows::monitor_volume_mute(event_tx, stop_rx);
            }

            #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
            {
                let _ = event_tx;
                tracing::warn!("device_monitoring_unsupported");
                let _ = stop_rx.recv();
            }
        });

        DeviceMonitorHandle {
            stop_tx: Some(stop_tx),
            thread_handle: Some(thread_handle),
        }
    }
}

pub struct DeviceMonitor;

impl DeviceMonitor {
    pub fn spawn(event_tx: mpsc::Sender<DeviceEvent>) -> DeviceMonitorHandle {
        Self::spawn_with_debounce(event_tx, None)
    }

    pub fn spawn_debounced(event_tx: mpsc::Sender<DeviceEvent>) -> DeviceMonitorHandle {
        Self::spawn_with_debounce(event_tx, Some(DEFAULT_DEBOUNCE_DELAY))
    }

    pub fn spawn_with_debounce(
        event_tx: mpsc::Sender<DeviceEvent>,
        debounce_delay: Option<Duration>,
    ) -> DeviceMonitorHandle {
        let (stop_tx, stop_rx) = mpsc::channel();
        let (raw_tx, raw_rx) = mpsc::sync_channel(MAX_RAW_DEVICE_EVENTS);
        let event_tx = EventOutput::Unbounded(event_tx);

        match debounce_delay {
            Some(delay) => {
                debounce::spawn_device_event_debouncer(delay, raw_rx, event_tx);
            }
            None => debounce::spawn_forwarder(raw_rx, event_tx),
        }

        let thread_handle = std::thread::spawn(move || {
            #[cfg(target_os = "macos")]
            {
                macos::monitor(raw_tx, stop_rx);
            }

            #[cfg(target_os = "linux")]
            {
                linux::monitor(raw_tx, stop_rx);
            }

            #[cfg(target_os = "windows")]
            {
                windows::monitor(raw_tx, stop_rx);
            }

            #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
            {
                let _ = raw_tx;
                tracing::warn!("device_monitoring_unsupported");
                let _ = stop_rx.recv();
            }
        });

        DeviceMonitorHandle {
            stop_tx: Some(stop_tx),
            thread_handle: Some(thread_handle),
        }
    }
}
