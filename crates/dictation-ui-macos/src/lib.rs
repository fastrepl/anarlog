use serde::Serialize;
use swift_rs::{Bool, SRString, swift};

swift!(fn _show_dictation_overlay() -> Bool);
swift!(fn _hide_dictation_overlay() -> Bool);
swift!(fn _update_dictation_state(json: &SRString) -> Bool);
swift!(fn _capture_dictation_target() -> SRString);
swift!(fn _insert_dictation_text(target: &SRString, text: &SRString) -> SRString);

pub fn capture_target() -> Result<String, String> {
    let value = unsafe { _capture_dictation_target() }.to_string();
    value.strip_prefix("ok:").map(str::to_owned).ok_or(value)
}

pub fn insert_text(target: &str, text: &str) -> Result<(), String> {
    let value = unsafe { _insert_dictation_text(&SRString::from(target), &SRString::from(text)) }
        .to_string();
    if value.is_empty() { Ok(()) } else { Err(value) }
}

#[derive(Serialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Recording,
    Processing,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DictationState {
    pub phase: Phase,
    pub amplitude: f32,
}

pub fn show() {
    unsafe {
        _show_dictation_overlay();
    }
}

pub fn hide() {
    unsafe {
        _hide_dictation_overlay();
    }
}

pub fn update_state(state: &DictationState) {
    let json = serde_json::to_string(state).unwrap();
    let json_str = SRString::from(json.as_str());
    unsafe {
        _update_dictation_state(&json_str);
    }
}
