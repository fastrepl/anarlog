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

/// When the default input is a Bluetooth device, returns a wired microphone to use instead.
///
/// Opening a Bluetooth headset's mic forces it into the HFP call profile: the headset gates the
/// mic to silence between words and the wearer's audio drops to 8–16 kHz. Any wired input avoids
/// both, so built-in mics are preferred, then USB. Linux reports onboard mics as PCI rather than
/// built-in, so PCI ranks with built-in. Line-in jacks and output loopbacks are capture endpoints
/// too but never microphones, so they are skipped.
pub fn wired_input_replacing_bluetooth_default() -> Option<AudioDevice> {
    let backend = backend();
    let default = backend.get_default_input_device().ok().flatten()?;
    let inputs = backend.list_input_devices().unwrap_or_default();
    resolve_wired_input_replacement(&default, inputs)
}

fn resolve_wired_input_replacement(
    default: &AudioDevice,
    inputs: Vec<AudioDevice>,
) -> Option<AudioDevice> {
    if default.transport_type != TransportType::Bluetooth {
        return None;
    }

    inputs
        .into_iter()
        .filter(|device| device.direction == AudioDirection::Input && device.id != default.id)
        .filter(|device| !name_suggests_non_microphone(&device.name))
        .filter_map(|device| wired_input_rank(device.transport_type).map(|rank| (rank, device)))
        // Within a transport, a device that calls itself a microphone beats one that does not;
        // `min_by_key` keeps list order for full ties.
        .min_by_key(|(rank, device)| (*rank, !name_suggests_microphone(&device.name)))
        .map(|(_, device)| device)
}

fn wired_input_rank(transport: TransportType) -> Option<u8> {
    match transport {
        TransportType::BuiltIn | TransportType::Pci => Some(0),
        TransportType::Usb => Some(1),
        TransportType::Hdmi => Some(2),
        TransportType::Unknown => Some(3),
        TransportType::Bluetooth | TransportType::Virtual => None,
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

    fn input(id: &str, transport: TransportType) -> AudioDevice {
        AudioDevice::new(id, id, AudioDirection::Input, transport)
    }

    #[test]
    fn wired_default_input_needs_no_replacement() {
        let result = resolve_wired_input_replacement(
            &input("builtin", TransportType::BuiltIn),
            vec![
                input("builtin", TransportType::BuiltIn),
                input("usb", TransportType::Usb),
            ],
        );
        assert!(result.is_none());
    }

    #[test]
    fn bluetooth_default_input_prefers_built_in_then_usb() {
        let inputs = vec![
            input("headset", TransportType::Bluetooth),
            input("usb", TransportType::Usb),
            input("builtin", TransportType::BuiltIn),
        ];
        let result =
            resolve_wired_input_replacement(&input("headset", TransportType::Bluetooth), inputs);
        assert_eq!(result.map(|d| d.id.0), Some("builtin".to_string()));

        let inputs = vec![
            input("headset", TransportType::Bluetooth),
            input("usb", TransportType::Usb),
        ];
        let result =
            resolve_wired_input_replacement(&input("headset", TransportType::Bluetooth), inputs);
        assert_eq!(result.map(|d| d.id.0), Some("usb".to_string()));
    }

    #[test]
    fn pci_onboard_input_ranks_with_built_in_over_usb() {
        let inputs = vec![
            input("headset", TransportType::Bluetooth),
            input("usb", TransportType::Usb),
            input("onboard", TransportType::Pci),
        ];
        let result =
            resolve_wired_input_replacement(&input("headset", TransportType::Bluetooth), inputs);
        assert_eq!(result.map(|d| d.id.0), Some("onboard".to_string()));
    }

    fn named_input(id: &str, name: &str, transport: TransportType) -> AudioDevice {
        AudioDevice::new(id, name, AudioDirection::Input, transport)
    }

    #[test]
    fn line_in_and_loopback_inputs_never_replace_bluetooth() {
        let headset = input("headset", TransportType::Bluetooth);
        let inputs = vec![
            headset.clone(),
            named_input(
                "mix",
                "Stereo Mix (Realtek(R) Audio)",
                TransportType::BuiltIn,
            ),
            named_input("line", "Line In (Realtek(R) Audio)", TransportType::BuiltIn),
            named_input(
                "array",
                "Microphone Array (Realtek(R) Audio)",
                TransportType::BuiltIn,
            ),
        ];
        let result = resolve_wired_input_replacement(&headset, inputs);
        assert_eq!(result.map(|d| d.id.0), Some("array".to_string()));

        let inputs = vec![
            headset.clone(),
            named_input(
                "mix",
                "Stereo Mix (Realtek(R) Audio)",
                TransportType::BuiltIn,
            ),
            named_input("line", "Line In (Realtek(R) Audio)", TransportType::BuiltIn),
        ];
        assert!(resolve_wired_input_replacement(&headset, inputs).is_none());
    }

    #[test]
    fn microphone_wins_ties_within_a_transport_but_not_across() {
        let headset = input("headset", TransportType::Bluetooth);
        let inputs = vec![
            headset.clone(),
            named_input("jack", "Built-in Input", TransportType::BuiltIn),
            named_input("mic", "Built-in Microphone", TransportType::BuiltIn),
        ];
        let result = resolve_wired_input_replacement(&headset, inputs);
        assert_eq!(result.map(|d| d.id.0), Some("mic".to_string()));

        let inputs = vec![
            headset.clone(),
            named_input("yeti", "Yeti Stereo Microphone", TransportType::Usb),
            named_input("jack", "Built-in Input", TransportType::BuiltIn),
        ];
        let result = resolve_wired_input_replacement(&headset, inputs);
        assert_eq!(result.map(|d| d.id.0), Some("jack".to_string()));
    }

    #[test]
    fn bluetooth_default_input_never_falls_back_to_bluetooth_or_virtual() {
        let inputs = vec![
            input("headset", TransportType::Bluetooth),
            input("earbuds", TransportType::Bluetooth),
            input("aggregate", TransportType::Virtual),
            AudioDevice::new(
                "speakers",
                "speakers",
                AudioDirection::Output,
                TransportType::BuiltIn,
            ),
        ];
        let result =
            resolve_wired_input_replacement(&input("headset", TransportType::Bluetooth), inputs);
        assert!(result.is_none());
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
