use windows::Win32::{
    System::{Com::*, Ole::*},
    UI::{Accessibility::*, Input::KeyboardAndMouse::*},
};

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

pub async fn insert(target: String, text: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN].iter().any(|key| unsafe { GetAsyncKeyState(key.0 as i32) } < 0) {
            if std::time::Instant::now() >= deadline {
                return Err("Release the shortcut keys before inserting text. Copy your last dictation from Settings > Dictation.".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        with_focused(|element| {
        if identity(&element)? != target {
            return Err("The focused text field changed. Copy your last dictation from Settings > Dictation.".into());
        }
        let inputs: Vec<INPUT> = text.encode_utf16().flat_map(|unit| {
            [KEYEVENTF_UNICODE, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP].map(|flags| INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 { ki: KEYBDINPUT { wScan: unit, dwFlags: flags, ..Default::default() } },
            })
        }).collect();
        if unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) } != inputs.len() as u32 {
            return Err("Windows could not insert the entire dictation. Check the field before copying your last dictation. Elevated apps may block text insertion.".into());
        }
        Ok(())
        })
    }).await.map_err(|e| e.to_string())?
}
