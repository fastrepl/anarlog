mod device;
mod error;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "windows")]
pub mod windows;

pub use device::*;
pub use error::*;

pub fn backend() -> impl AudioDeviceBackend {
    #[cfg(target_os = "macos")]
    {
        macos::MacOSBackend
    }

    #[cfg(target_os = "linux")]
    {
        linux::LinuxBackend
    }

    #[cfg(target_os = "windows")]
    {
        windows::WindowsBackend
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        compile_error!("Unsupported platform for audio-device crate")
    }
}

/// Returns a headphone when every output device that is currently playing audio is one.
///
/// Meeting apps can play through a device other than the system default, so the default alone
/// does not tell us whether speaker output can reach the mic. When nothing is playing yet, the
/// default output decides. Backends without running-device support fall back to the default.
pub fn headphone_only_output() -> Option<AudioDevice> {
    let backend = backend();
    let default = backend.get_default_output_device().ok().flatten();
    let running = backend.running_output_devices().unwrap_or_default();
    resolve_headphone_only_output(default, running, |device| backend.is_headphone(device))
}

fn resolve_headphone_only_output(
    default: Option<AudioDevice>,
    running: Vec<AudioDevice>,
    is_headphone: impl Fn(&AudioDevice) -> bool,
) -> Option<AudioDevice> {
    if running.is_empty() {
        return default.filter(|device| is_headphone(device));
    }

    if !running.iter().all(&is_headphone) {
        return None;
    }

    let default_is_running = default
        .as_ref()
        .is_some_and(|default| running.iter().any(|device| device.id == default.id));

    if default_is_running {
        default
    } else {
        running.into_iter().next()
    }
}

pub trait AudioDeviceBackend {
    fn list_devices(&self) -> Result<Vec<AudioDevice>, Error>;

    /// Output devices that some process is actively playing through right now.
    fn running_output_devices(&self) -> Result<Vec<AudioDevice>, Error> {
        Ok(Vec::new())
    }

    fn list_input_devices(&self) -> Result<Vec<AudioDevice>, Error> {
        Ok(self
            .list_devices()?
            .into_iter()
            .filter(|d| d.direction == AudioDirection::Input)
            .collect())
    }

    fn list_output_devices(&self) -> Result<Vec<AudioDevice>, Error> {
        Ok(self
            .list_devices()?
            .into_iter()
            .filter(|d| d.direction == AudioDirection::Output)
            .collect())
    }

    fn get_default_input_device(&self) -> Result<Option<AudioDevice>, Error>;

    fn get_default_output_device(&self) -> Result<Option<AudioDevice>, Error>;

    fn set_default_input_device(&self, device_id: &DeviceId) -> Result<(), Error>;

    fn set_default_output_device(&self, device_id: &DeviceId) -> Result<(), Error>;

    fn is_headphone(&self, device: &AudioDevice) -> bool;

    fn get_device_volume(&self, device_id: &DeviceId) -> Result<f32, Error>;

    fn set_device_volume(&self, device_id: &DeviceId, volume: f32) -> Result<(), Error>;

    fn is_device_muted(&self, device_id: &DeviceId) -> Result<bool, Error>;

    fn set_device_mute(&self, device_id: &DeviceId, muted: bool) -> Result<(), Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(id: &str, headphone: bool) -> AudioDevice {
        let name = if headphone { "Headphones" } else { "Speakers" };
        AudioDevice::new(id, name, AudioDirection::Output, TransportType::Unknown)
    }

    fn is_headphone(device: &AudioDevice) -> bool {
        device.name == "Headphones"
    }

    #[test]
    fn falls_back_to_default_output_when_nothing_is_playing() {
        let result =
            resolve_headphone_only_output(Some(output("hp", true)), Vec::new(), is_headphone);
        assert_eq!(result.map(|d| d.id.0), Some("hp".to_string()));

        let result =
            resolve_headphone_only_output(Some(output("spk", false)), Vec::new(), is_headphone);
        assert!(result.is_none());
    }

    #[test]
    fn any_running_speaker_disqualifies_headphone_default() {
        let result = resolve_headphone_only_output(
            Some(output("hp", true)),
            vec![output("hp", true), output("spk", false)],
            is_headphone,
        );
        assert!(result.is_none());
    }

    #[test]
    fn running_headphones_win_over_speaker_default() {
        let result = resolve_headphone_only_output(
            Some(output("spk", false)),
            vec![output("hp", true)],
            is_headphone,
        );
        assert_eq!(result.map(|d| d.id.0), Some("hp".to_string()));
    }

    #[test]
    fn prefers_default_among_running_headphones() {
        let result = resolve_headphone_only_output(
            Some(output("hp-default", true)),
            vec![output("hp-other", true), output("hp-default", true)],
            is_headphone,
        );
        assert_eq!(result.map(|d| d.id.0), Some("hp-default".to_string()));
    }

    #[test]
    fn test_headphone_only_output() {
        let backend = backend();
        println!(
            "running outputs: {:?}",
            backend
                .running_output_devices()
                .map(|devices| devices.into_iter().map(|d| d.name).collect::<Vec<_>>())
        );
        println!(
            "headphone_only_output: {:?}",
            headphone_only_output().map(|d| d.name)
        );
    }

    #[test]
    fn test_list_devices() {
        let backend = backend();
        match backend.list_devices() {
            Ok(devices) => {
                println!("Found {} devices:", devices.len());
                for device in &devices {
                    println!(
                        "  - {} ({:?}, {:?}, uid={})",
                        device.name, device.direction, device.transport_type, device.id.0
                    );
                }
            }
            Err(e) => {
                println!("Error listing devices: {}", e);
            }
        }
    }

    #[test]
    fn test_get_default_devices() {
        let backend = backend();

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
}
