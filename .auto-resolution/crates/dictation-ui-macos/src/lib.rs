use serde::Serialize;
use swift_rs::{Bool, SRString, swift};

swift!(fn _show_dictation_overlay() -> Bool);
swift!(fn _hide_dictation_overlay() -> Bool);
swift!(fn _update_dictation_state(json: &SRString) -> Bool);

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
