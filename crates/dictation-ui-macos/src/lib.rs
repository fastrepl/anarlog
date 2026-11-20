use serde::Deserialize;
use swift_rs::{Bool, Float, Int, SRString, swift};

swift!(fn _capture_dictation_target() -> SRString);
swift!(fn _insert_dictation_text(target: &SRString, text: &SRString) -> SRString);
swift!(fn _dictation_target_focused(target: &SRString) -> Bool);
swift!(fn _dictation_target_read(target: &SRString, window: Int) -> SRString);
swift!(fn _dictation_target_select(target: &SRString, start: Int, length: Int) -> Bool);
swift!(fn _dictation_target_write(target: &SRString, text: &SRString) -> Int);
swift!(fn _dictation_target_paste(target: &SRString, text: &SRString) -> Int);
swift!(fn _dictation_paste_state(id: Int) -> Int);
swift!(fn _dictation_paste_revoke(id: Int) -> Int);
swift!(fn _dictation_paste_finish(id: Int) -> Bool);
swift!(fn _dictation_target_facts(target: &SRString) -> SRString);
swift!(fn _dictation_target_window_text(target: &SRString, max: Int) -> SRString);
swift!(fn _dictation_target_release(target: &SRString) -> Bool);
swift!(fn _dictation_output_level() -> SRString);
swift!(fn _dictation_set_output_level(device: &SRString, volume: Float) -> Bool);

pub fn capture_target() -> Result<String, String> {
    let value = unsafe { _capture_dictation_target() }.to_string();
    value.strip_prefix("ok:").map(str::to_owned).ok_or(value)
}

pub fn insert_text(target: &str, text: &str) -> Result<(), String> {
    let value = unsafe { _insert_dictation_text(&SRString::from(target), &SRString::from(text)) }
        .to_string();
    if value.is_empty() { Ok(()) } else { Err(value) }
}

/// A captured field around its selection. Offsets and lengths are UTF-16 units.
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FieldText {
    pub length: usize,
    pub start: usize,
    pub before: String,
    pub selected: String,
    pub after: String,
}

#[derive(Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TargetFacts {
    pub bundle_id: Option<String>,
    pub app_name: Option<String>,
    pub title: Option<String>,
    pub secure: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteState {
    /// No app read the promised text yet.
    Pending,
    Read,
    /// It can no longer be read by anyone.
    Gone,
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct OutputLevel {
    pub device: String,
    pub profile: Option<String>,
    /// 0–1; `None` while muted or without a settable volume.
    pub volume: Option<f32>,
}

fn sr(text: &str) -> SRString {
    SRString::from(text)
}

fn json<T: for<'de> Deserialize<'de>>(value: SRString) -> Option<T> {
    serde_json::from_str(value.as_str()).ok()
}

/// The captured field itself is still the focused text field of the frontmost app.
pub fn target_focused(target: &str) -> bool {
    unsafe { _dictation_target_focused(&sr(target)) }
}

/// Reads up to `window` UTF-16 units either side of the selection; `None` when unreadable.
pub fn target_read(target: &str, window: usize) -> Option<FieldText> {
    json(unsafe { _dictation_target_read(&sr(target), window as Int) })
}

pub fn target_select(target: &str, start: usize, length: usize) -> bool {
    unsafe { _dictation_target_select(&sr(target), start as Int, length as Int) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetWrite {
    /// Provably not written: the selected text isn't settable or the app refused the write.
    Refused,
    Done,
    /// Sent, but the app never answered (a timeout); it may still land.
    Unconfirmed,
}

/// Writes over the selection through Accessibility.
pub fn target_write(target: &str, text: &str) -> TargetWrite {
    match unsafe { _dictation_target_write(&sr(target), &sr(text)) } {
        0 => TargetWrite::Refused,
        1 => TargetWrite::Done,
        _ => TargetWrite::Unconfirmed,
    }
}

/// Pastes through a promised clipboard item while the field has focus; the paste's id, or
/// `None` when no keystroke was sent.
pub fn target_paste(target: &str, text: &str) -> Option<isize> {
    let id = unsafe { _dictation_target_paste(&sr(target), &sr(text)) };
    (id > 0).then_some(id)
}

fn paste_state(value: Int) -> PasteState {
    match value {
        0 => PasteState::Pending,
        1 => PasteState::Read,
        _ => PasteState::Gone,
    }
}

pub fn paste_status(id: isize) -> PasteState {
    paste_state(unsafe { _dictation_paste_state(id) })
}

/// Withdraws an unread paste so its text can no longer land; `Read` when it already was.
pub fn paste_revoke(id: isize) -> PasteState {
    paste_state(unsafe { _dictation_paste_revoke(id) })
}

/// The app is done with a paste it read: gives the clipboard back now instead of after the
/// bounded hold, unless the person changed it since.
pub fn paste_finish(id: isize) {
    unsafe { _dictation_paste_finish(id) };
}

pub fn target_facts(target: &str) -> Option<TargetFacts> {
    json(unsafe { _dictation_target_facts(&sr(target)) })
}

pub fn target_window_text(target: &str, max_chars: usize) -> Option<String> {
    let text = unsafe { _dictation_target_window_text(&sr(target), max_chars as Int) };
    Some(text.to_string()).filter(|text| !text.is_empty())
}

pub fn target_release(target: &str) {
    unsafe { _dictation_target_release(&sr(target)) };
}

pub fn output_level() -> Option<OutputLevel> {
    json(unsafe { _dictation_output_level() })
}

/// Sets the default output's volume, only while `device` is still the default output.
pub fn set_output_level(device: &str, volume: f32) -> bool {
    unsafe { _dictation_set_output_level(&sr(device), volume) }
}
