use crate::{AudioDevice, AudioDeviceBackend, AudioDirection, DeviceId, Error, TransportType};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    DEVICE_STATE_ACTIVE, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, eAll, eCapture,
    eConsole, eRender,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize, STGM_READ,
};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::core::{GUID, Interface, PCWSTR, PWSTR};

pub struct WindowsBackend;

const CLSID_POLICY_CONFIG: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
const IID_IPOLICY_CONFIG: GUID = GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);

#[windows::core::interface("f8679f50-850a-41cf-9c72-430f290290c8")]
unsafe trait IPolicyConfig: windows::core::IUnknown {
    unsafe fn GetMixFormat(
        &self,
        device_id: PCWSTR,
        format: *mut *mut std::ffi::c_void,
    ) -> windows::core::HRESULT;
    unsafe fn GetDeviceFormat(
        &self,
        device_id: PCWSTR,
        default: i32,
        format: *mut *mut std::ffi::c_void,
    ) -> windows::core::HRESULT;
    unsafe fn ResetDeviceFormat(&self, device_id: PCWSTR) -> windows::core::HRESULT;
    unsafe fn SetDeviceFormat(
        &self,
        device_id: PCWSTR,
        format: *const std::ffi::c_void,
        mix_format: *const std::ffi::c_void,
    ) -> windows::core::HRESULT;
    unsafe fn GetProcessingPeriod(
        &self,
        device_id: PCWSTR,
        default: i32,
        default_period: *mut i64,
        min_period: *mut i64,
    ) -> windows::core::HRESULT;
    unsafe fn SetProcessingPeriod(
        &self,
        device_id: PCWSTR,
        period: *const i64,
    ) -> windows::core::HRESULT;
    unsafe fn GetShareMode(&self, device_id: PCWSTR, mode: *mut i32) -> windows::core::HRESULT;
    unsafe fn SetShareMode(&self, device_id: PCWSTR, mode: i32) -> windows::core::HRESULT;
    unsafe fn GetPropertyValue(
        &self,
        device_id: PCWSTR,
        store_type: i32,
        key: *const std::ffi::c_void,
        value: *mut std::ffi::c_void,
    ) -> windows::core::HRESULT;
    unsafe fn SetPropertyValue(
        &self,
        device_id: PCWSTR,
        store_type: i32,
        key: *const std::ffi::c_void,
        value: *const std::ffi::c_void,
    ) -> windows::core::HRESULT;
    unsafe fn SetDefaultEndpoint(&self, device_id: PCWSTR, role: u32) -> windows::core::HRESULT;
    unsafe fn SetEndpointVisibility(
        &self,
        device_id: PCWSTR,
        visible: i32,
    ) -> windows::core::HRESULT;
}

struct ComGuard;

impl ComGuard {
    fn new() -> Result<Self, Error> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED)
                .ok()
                .map_err(|e| {
                    Error::AudioSystemError(format!("COM initialization failed: {}", e))
                })?;
        }
        Ok(Self)
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

fn get_device_id(device: &IMMDevice) -> Result<String, Error> {
    unsafe {
        let id: PWSTR = device
            .GetId()
            .map_err(|e| Error::EnumerationFailed(format!("Failed to get device ID: {}", e)))?;

        let len = (0..).take_while(|&i| *id.0.add(i) != 0).count();
        let slice = std::slice::from_raw_parts(id.0, len);
        let os_string = OsString::from_wide(slice);

        windows::Win32::System::Com::CoTaskMemFree(Some(id.0 as *const _));

        os_string
            .into_string()
            .map_err(|_| Error::EnumerationFailed("Invalid device ID encoding".into()))
    }
}

fn get_device_name(device: &IMMDevice) -> Result<String, Error> {
    unsafe {
        let store: IPropertyStore = device.OpenPropertyStore(STGM_READ).map_err(|e| {
            Error::EnumerationFailed(format!("Failed to open property store: {}", e))
        })?;

        let prop = store
            .GetValue(&PKEY_Device_FriendlyName)
            .map_err(|e| Error::EnumerationFailed(format!("Failed to get device name: {}", e)))?;

        let name = prop.Anonymous.Anonymous.Anonymous.pwszVal;
        if name.is_null() {
            return Err(Error::EnumerationFailed("Device name is null".into()));
        }

        let len = (0..).take_while(|&i| *name.0.add(i) != 0).count();
        let slice = std::slice::from_raw_parts(name.0, len);
        let os_string = OsString::from_wide(slice);

        os_string
            .into_string()
            .map_err(|_| Error::EnumerationFailed("Invalid device name encoding".into()))
    }
}

fn get_transport_type_from_name(name: &str) -> TransportType {
    let name_lower = name.to_lowercase();
    if name_lower.contains("bluetooth") {
        TransportType::Bluetooth
    } else if name_lower.contains("usb") {
        TransportType::Usb
    } else if name_lower.contains("hdmi") || name_lower.contains("displayport") {
        TransportType::Hdmi
    } else if name_lower.contains("realtek") || name_lower.contains("high definition") {
        TransportType::BuiltIn
    } else {
        TransportType::Unknown
    }
}

fn is_headphone_from_name(name: &str) -> bool {
    let name_lower = name.to_lowercase();
    name_lower.contains("headphone")
        || name_lower.contains("headset")
        || name_lower.contains("earphone")
        || name_lower.contains("airpods")
}

impl AudioDeviceBackend for WindowsBackend {
    fn list_devices(&self) -> Result<Vec<AudioDevice>, Error> {
        let _com = ComGuard::new()?;

        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| {
                    Error::EnumerationFailed(format!("Failed to create enumerator: {}", e))
                })?;

            let mut devices = Vec::new();

            let default_input_id = enumerator
                .GetDefaultAudioEndpoint(eCapture, eConsole)
                .ok()
                .and_then(|d| get_device_id(&d).ok());

            let default_output_id = enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .ok()
                .and_then(|d| get_device_id(&d).ok());

            let collection = enumerator
                .EnumAudioEndpoints(eAll, DEVICE_STATE_ACTIVE)
                .map_err(|e| {
                    Error::EnumerationFailed(format!("Failed to enumerate endpoints: {}", e))
                })?;

            let count = collection.GetCount().map_err(|e| {
                Error::EnumerationFailed(format!("Failed to get device count: {}", e))
            })?;

            for i in 0..count {
                let device = match collection.Item(i) {
                    Ok(d) => d,
                    Err(_) => continue,
                };

                let id = match get_device_id(&device) {
                    Ok(id) => id,
                    Err(_) => continue,
                };

                let name = match get_device_name(&device) {
                    Ok(name) => name,
                    Err(_) => continue,
                };

                let endpoint: windows::Win32::Media::Audio::IMMEndpoint = match device.cast() {
                    Ok(e) => e,
                    Err(_) => continue,
                };

                let data_flow = match endpoint.GetDataFlow() {
                    Ok(flow) => flow,
                    Err(_) => continue,
                };

                let direction = match data_flow {
                    eRender => AudioDirection::Output,
                    eCapture => AudioDirection::Input,
                    _ => continue,
                };

                let transport_type = get_transport_type_from_name(&name);

                let is_default = match direction {
                    AudioDirection::Input => {
                        default_input_id.as_ref().map(|d| d == &id).unwrap_or(false)
                    }
                    AudioDirection::Output => default_output_id
                        .as_ref()
                        .map(|d| d == &id)
                        .unwrap_or(false),
                };

                let mut audio_device = AudioDevice {
                    id: DeviceId::new(id),
                    name,
                    direction,
                    transport_type,
                    is_default,
                    volume: None,
                    is_muted: None,
                };

                if direction == AudioDirection::Output {
                    if let Ok(volume_control) =
                        device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
                    {
                        if let Ok(volume) = volume_control.GetMasterVolumeLevelScalar() {
                            audio_device.volume = Some(volume);
                        }
                        if let Ok(muted) = volume_control.GetMute() {
                            audio_device.is_muted = Some(muted.as_bool());
                        }
                    }
                }

                devices.push(audio_device);
            }

            Ok(devices)
        }
    }

    fn get_default_input_device(&self) -> Result<Option<AudioDevice>, Error> {
        let _com = ComGuard::new()?;

        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| {
                    Error::GetDefaultFailed(format!("Failed to create enumerator: {}", e))
                })?;

            let device = match enumerator.GetDefaultAudioEndpoint(eCapture, eConsole) {
                Ok(d) => d,
                Err(_) => return Ok(None),
            };

            let id = get_device_id(&device)?;
            let name = get_device_name(&device)?;
            let transport_type = get_transport_type_from_name(&name);

            Ok(Some(AudioDevice {
                id: DeviceId::new(id),
                name,
                direction: AudioDirection::Input,
                transport_type,
                is_default: true,
                volume: None,
                is_muted: None,
            }))
        }
    }

    fn get_default_output_device(&self) -> Result<Option<AudioDevice>, Error> {
        let _com = ComGuard::new()?;

        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| {
                    Error::GetDefaultFailed(format!("Failed to create enumerator: {}", e))
                })?;

            let device = match enumerator.GetDefaultAudioEndpoint(eRender, eConsole) {
                Ok(d) => d,
                Err(_) => return Ok(None),
            };

            let id = get_device_id(&device)?;
            let name = get_device_name(&device)?;
            let transport_type = get_transport_type_from_name(&name);

            let mut audio_device = AudioDevice {
                id: DeviceId::new(id),
                name,
                direction: AudioDirection::Output,
                transport_type,
                is_default: true,
                volume: None,
                is_muted: None,
            };

            if let Ok(volume_control) = device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) {
                if let Ok(volume) = volume_control.GetMasterVolumeLevelScalar() {
                    audio_device.volume = Some(volume);
                }
                if let Ok(muted) = volume_control.GetMute() {
                    audio_device.is_muted = Some(muted.as_bool());
                }
            }

            Ok(Some(audio_device))
        }
    }

    fn set_default_input_device(&self, device_id: &DeviceId) -> Result<(), Error> {
        let _com = ComGuard::new()?;

        unsafe {
            let policy_config: IPolicyConfig =
                CoCreateInstance(&CLSID_POLICY_CONFIG, None, CLSCTX_ALL).map_err(|e| {
                    Error::SetDefaultFailed(format!("Failed to create PolicyConfig: {}", e))
                })?;

            let device_id_wide: Vec<u16> = device_id
                .0
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            for role in 0..3u32 {
                policy_config
                    .SetDefaultEndpoint(PCWSTR(device_id_wide.as_ptr()), role)
                    .ok()
                    .map_err(|e| {
                        Error::SetDefaultFailed(format!("Failed to set default endpoint: {}", e))
                    })?;
            }

            Ok(())
        }
    }

    fn set_default_output_device(&self, device_id: &DeviceId) -> Result<(), Error> {
        self.set_default_input_device(device_id)
    }

    fn is_headphone(&self, device: &AudioDevice) -> bool {
        if device.direction != AudioDirection::Output {
            return false;
        }

        is_headphone_from_name(&device.name) || device.transport_type == TransportType::Bluetooth
    }

    fn get_device_volume(&self, device_id: &DeviceId) -> Result<f32, Error> {
        let _com = ComGuard::new()?;

        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| {
                    Error::AudioSystemError(format!("Failed to create enumerator: {}", e))
                })?;

            let device_id_wide: Vec<u16> = device_id
                .0
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            let device = enumerator
                .GetDevice(PCWSTR(device_id_wide.as_ptr()))
                .map_err(|e| Error::DeviceNotFound(format!("{}: {}", device_id.0, e)))?;

            let volume_control: IAudioEndpointVolume =
                device.Activate(CLSCTX_ALL, None).map_err(|e| {
                    Error::AudioSystemError(format!("Failed to activate volume control: {}", e))
                })?;

            let level = volume_control
                .GetMasterVolumeLevelScalar()
                .map_err(|e| Error::AudioSystemError(format!("Failed to get volume: {}", e)))?;

            Ok(level)
        }
    }

    fn set_device_volume(&self, device_id: &DeviceId, volume: f32) -> Result<(), Error> {
        let volume = volume.clamp(0.0, 1.0);
        let _com = ComGuard::new()?;

        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| {
                    Error::AudioSystemError(format!("Failed to create enumerator: {}", e))
                })?;

            let device_id_wide: Vec<u16> = device_id
                .0
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            let device = enumerator
                .GetDevice(PCWSTR(device_id_wide.as_ptr()))
                .map_err(|e| Error::DeviceNotFound(format!("{}: {}", device_id.0, e)))?;

            let volume_control: IAudioEndpointVolume =
                device.Activate(CLSCTX_ALL, None).map_err(|e| {
                    Error::AudioSystemError(format!("Failed to activate volume control: {}", e))
                })?;

            volume_control
                .SetMasterVolumeLevelScalar(volume, std::ptr::null())
                .map_err(|e| Error::AudioSystemError(format!("Failed to set volume: {}", e)))?;

            Ok(())
        }
    }

    fn is_device_muted(&self, device_id: &DeviceId) -> Result<bool, Error> {
        let _com = ComGuard::new()?;

        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| {
                    Error::AudioSystemError(format!("Failed to create enumerator: {}", e))
                })?;

            let device_id_wide: Vec<u16> = device_id
                .0
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            let device = enumerator
                .GetDevice(PCWSTR(device_id_wide.as_ptr()))
                .map_err(|e| Error::DeviceNotFound(format!("{}: {}", device_id.0, e)))?;

            let volume_control: IAudioEndpointVolume =
                device.Activate(CLSCTX_ALL, None).map_err(|e| {
                    Error::AudioSystemError(format!("Failed to activate volume control: {}", e))
                })?;

            let muted = volume_control
                .GetMute()
                .map_err(|e| Error::AudioSystemError(format!("Failed to get mute state: {}", e)))?;

            Ok(muted.as_bool())
        }
    }

    fn set_device_mute(&self, device_id: &DeviceId, muted: bool) -> Result<(), Error> {
        let _com = ComGuard::new()?;

        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| {
                    Error::AudioSystemError(format!("Failed to create enumerator: {}", e))
                })?;

            let device_id_wide: Vec<u16> = device_id
                .0
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            let device = enumerator
                .GetDevice(PCWSTR(device_id_wide.as_ptr()))
                .map_err(|e| Error::DeviceNotFound(format!("{}: {}", device_id.0, e)))?;

            let volume_control: IAudioEndpointVolume =
                device.Activate(CLSCTX_ALL, None).map_err(|e| {
                    Error::AudioSystemError(format!("Failed to activate volume control: {}", e))
                })?;

            volume_control
                .SetMute(muted, std::ptr::null())
                .map_err(|e| Error::AudioSystemError(format!("Failed to set mute state: {}", e)))?;

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_devices() {
        let backend = WindowsBackend;
        match backend.list_devices() {
            Ok(devices) => {
                println!("Found {} devices:", devices.len());
                for device in &devices {
                    println!(
                        "  - {} ({:?}, {:?}, id={})",
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
        let backend = WindowsBackend;

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
