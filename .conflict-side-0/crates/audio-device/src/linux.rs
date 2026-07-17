use crate::{AudioDevice, AudioDeviceBackend, AudioDirection, DeviceId, Error, TransportType};
use libpulse_binding as pulse;
use pulse::context::{Context, FlagSet as ContextFlagSet};
use pulse::mainloop::threaded::Mainloop;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_millis(2000);
const QUERY_TIMEOUT: Duration = Duration::from_millis(1000);

pub struct LinuxBackend;

struct PulseConnection {
    mainloop: Mainloop,
    context: Context,
}

impl PulseConnection {
    fn new() -> Result<Self, Error> {
        let mainloop = Mainloop::new().ok_or_else(|| {
            Error::AudioSystemError("Failed to create PulseAudio mainloop".into())
        })?;

        let context = Context::new(&mainloop, "hyprnote-audio-device")
            .ok_or_else(|| Error::AudioSystemError("Failed to create PulseAudio context".into()))?;

        Ok(Self { mainloop, context })
    }

    fn connect(&mut self) -> Result<(), Error> {
        self.context
            .connect(None, ContextFlagSet::NOFLAGS, None)
            .map_err(|e| {
                Error::AudioSystemError(format!("Failed to connect to PulseAudio: {:?}", e))
            })?;

        self.mainloop
            .start()
            .map_err(|e| Error::AudioSystemError(format!("Failed to start mainloop: {:?}", e)))?;

        self.wait_for_ready()?;
        Ok(())
    }

    fn wait_for_ready(&mut self) -> Result<(), Error> {
        let start = std::time::Instant::now();

        loop {
            if start.elapsed() > CONNECT_TIMEOUT {
                return Err(Error::AudioSystemError(
                    "PulseAudio connection timeout".into(),
                ));
            }

            self.mainloop.lock();
            let state = self.context.get_state();
            self.mainloop.unlock();

            match state {
                pulse::context::State::Ready => return Ok(()),
                pulse::context::State::Failed | pulse::context::State::Terminated => {
                    return Err(Error::AudioSystemError(
                        "PulseAudio connection failed".into(),
                    ));
                }
                _ => std::thread::sleep(Duration::from_millis(10)),
            }
        }
    }
}

impl Drop for PulseConnection {
    fn drop(&mut self) {
        self.mainloop.lock();
        self.context.disconnect();
        self.mainloop.unlock();
        self.mainloop.stop();
    }
}

fn wait_for_done(done: &AtomicBool, timeout: Duration) {
    let start = std::time::Instant::now();
    while !done.load(Ordering::Acquire) && start.elapsed() < timeout {
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_context(mainloop: &mut Mainloop, context: &Context, timeout: Duration) -> bool {
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() > timeout {
            tracing::debug!("pulseaudio_connect_timeout");
            return false;
        }

        mainloop.lock();
        let state = context.get_state();
        mainloop.unlock();

        match state {
            pulse::context::State::Ready => return true,
            pulse::context::State::Failed | pulse::context::State::Terminated => {
                tracing::debug!("pulseaudio_context_connection_failed");
                return false;
            }
            _ => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

fn transport_type_from_bus(bus: Option<&str>) -> TransportType {
    match bus {
        Some(b) if b.contains("usb") => TransportType::Usb,
        Some(b) if b.contains("bluetooth") => TransportType::Bluetooth,
        Some(b) if b.contains("pci") => TransportType::Pci,
        Some(b) if b.contains("hdmi") => TransportType::Hdmi,
        _ => TransportType::Unknown,
    }
}

impl AudioDeviceBackend for LinuxBackend {
    fn list_devices(&self) -> Result<Vec<AudioDevice>, Error> {
        let mut conn = PulseConnection::new()?;
        conn.connect()?;

        let devices = Arc::new(Mutex::new(Vec::new()));
        let default_sink = Arc::new(Mutex::new(None::<String>));
        let default_source = Arc::new(Mutex::new(None::<String>));

        {
            let default_sink = default_sink.clone();
            let default_source = default_source.clone();
            let done = Arc::new(AtomicBool::new(false));
            let done_clone = done.clone();

            conn.mainloop.lock();
            let introspector = conn.context.introspect();
            introspector.get_server_info(move |info| {
                if let Some(name) = &info.default_sink_name {
                    if let Ok(mut ds) = default_sink.lock() {
                        *ds = Some(name.to_string());
                    }
                }
                if let Some(name) = &info.default_source_name {
                    if let Ok(mut ds) = default_source.lock() {
                        *ds = Some(name.to_string());
                    }
                }
                done_clone.store(true, Ordering::Release);
            });
            conn.mainloop.unlock();

            wait_for_done(&done, QUERY_TIMEOUT);
        }

        let default_sink_name = default_sink.lock().ok().and_then(|d| d.clone());
        let default_source_name = default_source.lock().ok().and_then(|d| d.clone());

        {
            let devices = devices.clone();
            let default_sink_name = default_sink_name.clone();
            let done = Arc::new(AtomicBool::new(false));
            let done_clone = done.clone();

            conn.mainloop.lock();
            let introspector = conn.context.introspect();
            introspector.get_sink_info_list(move |list_result| {
                if let pulse::callbacks::ListResult::Item(sink_info) = list_result {
                    if let Some(name) = sink_info.name.as_ref() {
                        let description = sink_info
                            .description
                            .as_ref()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| name.to_string());

                        let bus = sink_info.proplist.get_str("device.bus");
                        let transport_type = transport_type_from_bus(bus.as_deref());

                        let is_default = default_sink_name
                            .as_ref()
                            .map(|d| d == name.as_ref())
                            .unwrap_or(false);

                        let volume = sink_info.volume.avg().0 as f32
                            / pulse::volume::Volume::NORMAL.0 as f32;
                        let is_muted = sink_info.mute;

                        let device = AudioDevice {
                            id: DeviceId::new(name.to_string()),
                            name: description,
                            direction: AudioDirection::Output,
                            transport_type,
                            is_default,
                            volume: Some(volume),
                            is_muted: Some(is_muted),
                        };

                        if let Ok(mut devs) = devices.lock() {
                            devs.push(device);
                        }
                    }
                }
                if let pulse::callbacks::ListResult::End = list_result {
                    done_clone.store(true, Ordering::Release);
                }
            });
            conn.mainloop.unlock();

            wait_for_done(&done, QUERY_TIMEOUT);
        }

        {
            let devices = devices.clone();
            let default_source_name = default_source_name.clone();
            let done = Arc::new(AtomicBool::new(false));
            let done_clone = done.clone();

            conn.mainloop.lock();
            let introspector = conn.context.introspect();
            introspector.get_source_info_list(move |list_result| {
                if let pulse::callbacks::ListResult::Item(source_info) = list_result {
                    if let Some(name) = source_info.name.as_ref() {
                        if name.ends_with(".monitor") {
                            return;
                        }

                        let description = source_info
                            .description
                            .as_ref()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| name.to_string());

                        let bus = source_info.proplist.get_str("device.bus");
                        let transport_type = transport_type_from_bus(bus.as_deref());

                        let is_default = default_source_name
                            .as_ref()
                            .map(|d| d == name.as_ref())
                            .unwrap_or(false);

                        let device = AudioDevice {
                            id: DeviceId::new(name.to_string()),
                            name: description,
                            direction: AudioDirection::Input,
                            transport_type,
                            is_default,
                            volume: None,
                            is_muted: None,
                        };

                        if let Ok(mut devs) = devices.lock() {
                            devs.push(device);
                        }
                    }
                }
                if let pulse::callbacks::ListResult::End = list_result {
                    done_clone.store(true, Ordering::Release);
                }
            });
            conn.mainloop.unlock();

            wait_for_done(&done, QUERY_TIMEOUT);
        }

        let result = devices
            .lock()
            .map_err(|_| Error::AudioSystemError("Failed to acquire device list lock".into()))?;

        Ok(result.clone())
    }

    fn get_default_input_device(&self) -> Result<Option<AudioDevice>, Error> {
        let devices = self.list_devices()?;
        Ok(devices
            .into_iter()
            .find(|d| d.direction == AudioDirection::Input && d.is_default))
    }

    fn get_default_output_device(&self) -> Result<Option<AudioDevice>, Error> {
        let devices = self.list_devices()?;
        Ok(devices
            .into_iter()
            .find(|d| d.direction == AudioDirection::Output && d.is_default))
    }

    fn set_default_input_device(&self, device_id: &DeviceId) -> Result<(), Error> {
        let mut conn = PulseConnection::new()?;
        conn.connect()?;

        let success = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));

        let success_clone = success.clone();
        let done_clone = done.clone();

        conn.mainloop.lock();
        conn.context
            .set_default_source(&device_id.0, move |result| {
                success_clone.store(result, Ordering::Release);
                done_clone.store(true, Ordering::Release);
            });
        conn.mainloop.unlock();

        wait_for_done(&done, QUERY_TIMEOUT);

        if success.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(Error::SetDefaultFailed(format!(
                "Failed to set default source: {}",
                device_id.0
            )))
        }
    }

    fn set_default_output_device(&self, device_id: &DeviceId) -> Result<(), Error> {
        let mut conn = PulseConnection::new()?;
        conn.connect()?;

        let success = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));

        let success_clone = success.clone();
        let done_clone = done.clone();

        conn.mainloop.lock();
        conn.context.set_default_sink(&device_id.0, move |result| {
            success_clone.store(result, Ordering::Release);
            done_clone.store(true, Ordering::Release);
        });
        conn.mainloop.unlock();

        wait_for_done(&done, QUERY_TIMEOUT);

        if success.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(Error::SetDefaultFailed(format!(
                "Failed to set default sink: {}",
                device_id.0
            )))
        }
    }

    fn is_headphone(&self, device: &AudioDevice) -> bool {
        if device.direction != AudioDirection::Output {
            return false;
        }

        let mut conn = match PulseConnection::new() {
            Ok(c) => c,
            Err(_) => return false,
        };

        if conn.connect().is_err() {
            return false;
        }

        let result = Arc::new(Mutex::new(false));
        let done = Arc::new(AtomicBool::new(false));

        let result_clone = result.clone();
        let done_clone = done.clone();
        let device_name = device.id.0.clone();

        conn.mainloop.lock();
        let introspector = conn.context.introspect();
        introspector.get_sink_info_by_name(&device_name, move |list_result| {
            if let pulse::callbacks::ListResult::Item(sink_info) = list_result {
                if let Some(active_port) = &sink_info.active_port {
                    if let Some(name) = active_port.name.as_ref() {
                        let name_lower = name.to_lowercase();
                        let is_headphone =
                            name_lower.contains("headphone") || name_lower.contains("headset");
                        if let Ok(mut r) = result_clone.lock() {
                            *r = is_headphone;
                        }
                    }
                }
            }
            done_clone.store(true, Ordering::Release);
        });
        conn.mainloop.unlock();

        wait_for_done(&done, QUERY_TIMEOUT);

        result.lock().map(|r| *r).unwrap_or(false)
    }

    fn get_device_volume(&self, device_id: &DeviceId) -> Result<f32, Error> {
        let mut conn = PulseConnection::new()?;
        conn.connect()?;

        let result = Arc::new(Mutex::new(None::<f32>));
        let done = Arc::new(AtomicBool::new(false));

        let result_clone = result.clone();
        let done_clone = done.clone();
        let device_name = device_id.0.clone();

        conn.mainloop.lock();
        let introspector = conn.context.introspect();
        introspector.get_sink_info_by_name(&device_name, move |list_result| {
            if let pulse::callbacks::ListResult::Item(sink_info) = list_result {
                let avg_volume =
                    sink_info.volume.avg().0 as f32 / pulse::volume::Volume::NORMAL.0 as f32;
                if let Ok(mut r) = result_clone.lock() {
                    *r = Some(avg_volume);
                }
            }
            done_clone.store(true, Ordering::Release);
        });
        conn.mainloop.unlock();

        wait_for_done(&done, QUERY_TIMEOUT);

        let vol = result
            .lock()
            .map_err(|_| Error::AudioSystemError("Failed to acquire volume lock".into()))?
            .ok_or_else(|| Error::DeviceNotFound(device_id.to_string()))?;

        Ok(vol)
    }

    fn set_device_volume(&self, device_id: &DeviceId, volume: f32) -> Result<(), Error> {
        let volume = volume.clamp(0.0, 1.0);

        let mut conn = PulseConnection::new()?;
        conn.connect()?;

        let volume_value = (volume * pulse::volume::Volume::NORMAL.0 as f32) as u32;

        let volume_info = Arc::new(Mutex::new(None::<pulse::volume::ChannelVolumes>));
        let info_done = Arc::new(AtomicBool::new(false));

        let volume_info_clone = volume_info.clone();
        let info_done_clone = info_done.clone();
        let device_name = device_id.0.clone();

        conn.mainloop.lock();
        let introspector = conn.context.introspect();
        introspector.get_sink_info_by_name(&device_name, move |list_result| {
            if let pulse::callbacks::ListResult::Item(sink_info) = list_result {
                let mut new_volume = sink_info.volume;
                new_volume.set(new_volume.len(), pulse::volume::Volume(volume_value));
                if let Ok(mut v) = volume_info_clone.lock() {
                    *v = Some(new_volume);
                }
            }
            info_done_clone.store(true, Ordering::Release);
        });
        conn.mainloop.unlock();

        wait_for_done(&info_done, QUERY_TIMEOUT);

        let new_volume = volume_info
            .lock()
            .map_err(|_| Error::AudioSystemError("Failed to acquire volume info lock".into()))?
            .ok_or_else(|| Error::DeviceNotFound(device_id.to_string()))?;

        let success = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));

        let success_clone = success.clone();
        let done_clone = done.clone();

        conn.mainloop.lock();
        let mut introspector = conn.context.introspect();
        introspector.set_sink_volume_by_name(
            &device_id.0,
            &new_volume,
            Some(Box::new(move |result| {
                success_clone.store(result, Ordering::Release);
                done_clone.store(true, Ordering::Release);
            })),
        );
        conn.mainloop.unlock();

        wait_for_done(&done, QUERY_TIMEOUT);

        if success.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(Error::AudioSystemError(format!(
                "Failed to set volume for device: {}",
                device_id.0
            )))
        }
    }

    fn is_device_muted(&self, device_id: &DeviceId) -> Result<bool, Error> {
        let mut conn = PulseConnection::new()?;
        conn.connect()?;

        let result = Arc::new(Mutex::new(None::<bool>));
        let done = Arc::new(AtomicBool::new(false));

        let result_clone = result.clone();
        let done_clone = done.clone();
        let device_name = device_id.0.clone();

        conn.mainloop.lock();
        let introspector = conn.context.introspect();
        introspector.get_sink_info_by_name(&device_name, move |list_result| {
            if let pulse::callbacks::ListResult::Item(sink_info) = list_result {
                if let Ok(mut r) = result_clone.lock() {
                    *r = Some(sink_info.mute);
                }
            }
            done_clone.store(true, Ordering::Release);
        });
        conn.mainloop.unlock();

        wait_for_done(&done, QUERY_TIMEOUT);

        let muted = result
            .lock()
            .map_err(|_| Error::AudioSystemError("Failed to acquire mute lock".into()))?
            .ok_or_else(|| Error::DeviceNotFound(device_id.to_string()))?;

        Ok(muted)
    }

    fn set_device_mute(&self, device_id: &DeviceId, muted: bool) -> Result<(), Error> {
        let mut conn = PulseConnection::new()?;
        conn.connect()?;

        let success = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));

        let success_clone = success.clone();
        let done_clone = done.clone();

        conn.mainloop.lock();
        let mut introspector = conn.context.introspect();
        introspector.set_sink_mute_by_name(
            &device_id.0,
            muted,
            Some(Box::new(move |result| {
                success_clone.store(result, Ordering::Release);
                done_clone.store(true, Ordering::Release);
            })),
        );
        conn.mainloop.unlock();

        wait_for_done(&done, QUERY_TIMEOUT);

        if success.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(Error::AudioSystemError(format!(
                "Failed to set mute for device: {}",
                device_id.0
            )))
        }
    }
}

pub fn is_headphone_from_default_output_device() -> Option<bool> {
    let mut mainloop = Mainloop::new()?;

    let mut context = Context::new(&mainloop, "hyprnote-headphone-check")?;

    if context
        .connect(None, ContextFlagSet::NOFLAGS, None)
        .is_err()
    {
        tracing::debug!("failed_to_connect_to_pulseaudio");
        return None;
    }

    if mainloop.start().is_err() {
        tracing::debug!("failed_to_start_mainloop");
        return None;
    }

    if !wait_for_context(&mut mainloop, &context, CONNECT_TIMEOUT) {
        mainloop.stop();
        return None;
    }

    let result = Arc::new(Mutex::new(None));
    let done = Arc::new(AtomicBool::new(false));

    {
        let result = result.clone();
        let done = done.clone();

        mainloop.lock();
        let introspector = context.introspect();
        introspector.get_sink_info_by_name("@DEFAULT_SINK@", move |list_result| {
            if let pulse::callbacks::ListResult::Item(sink_info) = list_result {
                if let Some(active_port) = &sink_info.active_port {
                    if let Some(name) = active_port.name.as_ref() {
                        let name_lower = name.to_lowercase();
                        let is_headphone =
                            name_lower.contains("headphone") || name_lower.contains("headset");
                        if let Ok(mut r) = result.lock() {
                            *r = if is_headphone { Some(true) } else { None };
                        }
                    }
                }
            }
            done.store(true, Ordering::Release);
        });
        mainloop.unlock();
    }

    wait_for_done(&done, QUERY_TIMEOUT);
    mainloop.stop();

    result.lock().ok().and_then(|r| *r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_devices() {
        let backend = LinuxBackend;
        match backend.list_devices() {
            Ok(devices) => {
                println!("Found {} devices", devices.len());
                for device in &devices {
                    println!(
                        "  {} ({:?}, {:?}, id={})",
                        device.name, device.direction, device.transport_type, device.id.0
                    );
                }
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }

    #[test]
    fn test_get_default_devices() {
        let backend = LinuxBackend;

        match backend.get_default_input_device() {
            Ok(Some(device)) => {
                println!("Default input: {} ({})", device.name, device.id.0);
            }
            Ok(None) => {
                println!("No default input device");
            }
            Err(e) => {
                println!("Error getting default input: {}", e);
            }
        }

        match backend.get_default_output_device() {
            Ok(Some(device)) => {
                println!("Default output: {} ({})", device.name, device.id.0);
                println!("Is headphone: {}", backend.is_headphone(&device));
            }
            Ok(None) => {
                println!("No default output device");
            }
            Err(e) => {
                println!("Error getting default output: {}", e);
            }
        }
    }

    #[test]
    fn test_is_headphone_from_default_output_device() {
        let result = is_headphone_from_default_output_device();
        println!("is_headphone_from_default_output_device={:?}", result);
    }
}
