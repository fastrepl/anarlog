use std::time::{Duration, Instant};

use windows::Win32::{
    System::{Com::*, Ole::*},
    UI::{Accessibility::*, Input::KeyboardAndMouse::*},
};

use super::edit::{FieldPort, FieldRead, Paste, Span, Write};

struct Com;
impl Drop for Com {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

fn with_focused<T>(
    work: impl FnOnce(IUIAutomationElement) -> Result<T, String>,
) -> Result<T, String> {
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }.map_err(|e| e.to_string())?;
    let _com = Com;
    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }
            .map_err(|e| e.to_string())?;
    let element = unsafe { automation.GetFocusedElement() }.map_err(|e| e.to_string())?;
    if unsafe { element.CurrentIsPassword() }
        .map_err(|e| e.to_string())?
        .as_bool()
    {
        return Err("Dictation is unavailable in password fields.".into());
    }
    let control = unsafe { element.CurrentControlType() }.map_err(|e| e.to_string())?;
    if ![
        UIA_EditControlTypeId,
        UIA_DocumentControlTypeId,
        UIA_ComboBoxControlTypeId,
    ]
    .contains(&control)
        || !unsafe { element.CurrentIsEnabled() }
            .map_err(|e| e.to_string())?
            .as_bool()
    {
        return Err("Focus an editable text field before dictating.".into());
    }
    let read_only = unsafe {
        if let Ok(value) = element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) {
            value.CurrentIsReadOnly().map(|value| value.as_bool())
        } else {
            element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId)
                .and_then(|text| text.DocumentRange())
                .and_then(|range| range.GetAttributeValue(UIA_IsReadOnlyAttributeId))
                .and_then(|value| windows::Win32::System::Variant::VariantToBoolean(&value))
                .map(|value| value.as_bool())
        }
    }.map_err(|_| "This field does not expose whether it is editable. Copy your last dictation from Settings > Dictation.".to_string())?;
    if read_only {
        return Err("Focus an editable text field before dictating.".into());
    }
    work(element)
}

fn identity(element: &IUIAutomationElement) -> Result<String, String> {
    unsafe {
        let array = element.GetRuntimeId().map_err(|e| e.to_string())?;
        let result = (|| -> windows::core::Result<Vec<i32>> {
            let lower = SafeArrayGetLBound(array, 1)?;
            let upper = SafeArrayGetUBound(array, 1)?;
            let mut values = Vec::new();
            for index in lower..=upper {
                let mut value = 0_i32;
                SafeArrayGetElement(array, &index, (&mut value as *mut i32).cast())?;
                values.push(value);
            }
            Ok(values)
        })();
        let _ = SafeArrayDestroy(array);
        let values = result.map_err(|e| e.to_string())?;
        if values.is_empty() {
            return Err("This text field does not expose an accessibility identity.".into());
        }
        serde_json::to_string(&values).map_err(|e| e.to_string())
    }
}

pub async fn capture() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(|| with_focused(|element| identity(&element)))
        .await
        .map_err(|e| e.to_string())?
}

/// How long typing waits for the shortcut's modifier keys to be released.
const KEYS_RELEASE_WAIT: Duration = Duration::from_secs(2);

/// The captured field through UI Automation. Its text is not read, so it takes plain dictation
/// typed as Unicode key presses, reported as applied but unverified. No clipboard paste, voice
/// edit, raw restoration or context.
pub struct Field {
    target: String,
}

pub fn field(target: &str) -> Option<Field> {
    Some(Field {
        target: target.to_owned(),
    })
}

fn modifiers_held() -> bool {
    [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN]
        .iter()
        .any(|key| unsafe { GetAsyncKeyState(key.0 as i32) } < 0)
}

impl FieldPort for Field {
    fn focused(&self) -> bool {
        with_focused(|element| identity(&element)).is_ok_and(|focused| focused == self.target)
    }

    fn read(&self) -> Option<FieldRead> {
        None
    }

    fn select(&self, _span: Span) -> bool {
        false
    }

    fn write(&self, text: &str) -> Write {
        // Typed keys would combine with a held modifier, and focus may have moved meanwhile.
        if modifiers_held() || !self.focused() {
            return Write::Rejected;
        }
        let inputs: Vec<INPUT> = text
            .encode_utf16()
            .flat_map(|unit| {
                [KEYEVENTF_UNICODE, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP].map(|flags| INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wScan: unit,
                            dwFlags: flags,
                            ..Default::default()
                        },
                    },
                })
            })
            .collect();
        // Elevated apps may block synthesized input entirely.
        match unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) } as usize {
            0 => Write::Rejected,
            sent if sent == inputs.len() => Write::Accepted,
            _ => Write::Partial,
        }
    }

    fn paste(&self, _text: &str) -> Paste {
        Paste::NotSent
    }

    fn settle(&self) {}

    fn keys_released(&self) -> bool {
        let deadline = Instant::now() + KEYS_RELEASE_WAIT;
        while modifiers_held() {
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        true
    }
}

pub fn facts(_target: &str) -> Option<dictation_core::context::WindowFacts> {
    None
}

pub fn window_text(_target: &str, _max_chars: usize) -> Option<String> {
    None
}

pub fn release(_target: &str) {}

/// The default render endpoint's master volume. Each call opens its own COM apartment, since the
/// ducker calls from whichever thread ends a dictation.
struct EndpointVolume;

fn default_endpoint() -> Option<(String, windows::Win32::Media::Audio::IMMDevice)> {
    use windows::Win32::Media::Audio::{
        IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender,
    };
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
        let id = device.GetId().ok()?;
        let name = id.to_string().ok();
        CoTaskMemFree(Some(id.0 as *const _));
        Some((name?, device))
    }
}

fn endpoint_volume(
    device: &windows::Win32::Media::Audio::IMMDevice,
) -> Option<windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume> {
    unsafe { device.Activate(CLSCTX_ALL, None).ok() }
}

fn with_com<T>(work: impl FnOnce() -> Option<T>) -> Option<T> {
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }.ok()?;
    let _com = Com;
    work()
}

impl super::volume::SystemVolume for EndpointVolume {
    fn output(&self) -> Option<super::volume::Output> {
        with_com(|| {
            default_endpoint().map(|(device, _)| super::volume::Output {
                device,
                profile: None,
            })
        })
    }

    fn get(&self) -> Option<f32> {
        with_com(|| {
            let volume = endpoint_volume(&default_endpoint()?.1)?;
            unsafe {
                if volume.GetMute().ok()?.as_bool() {
                    return None;
                }
                volume.GetMasterVolumeLevelScalar().ok()
            }
        })
    }

    fn set(&self, device: &str, level: f32) -> bool {
        with_com(|| {
            let (current, endpoint) = default_endpoint()?;
            if current != device {
                return None;
            }
            let volume = endpoint_volume(&endpoint)?;
            unsafe { volume.SetMasterVolumeLevelScalar(level.clamp(0.0, 1.0), std::ptr::null()) }
                .ok()
        })
        .is_some()
    }
}

pub fn system_volume() -> Option<Box<dyn super::volume::SystemVolume>> {
    Some(Box::new(EndpointVolume))
}
