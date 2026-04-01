mod capture;
mod input;
mod logo;
mod viewport;
pub mod waveform;

pub use capture::{CaptureLayer, TraceBuffer, new_trace_buffer};
pub(crate) use input::InputAction;
pub use viewport::InlineViewport;

pub const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub fn truncate_line(s: &str, max: usize) -> &str {
    let char_count = s.chars().count();
    if char_count <= max {
        return s;
    }
    let skip = char_count - max;
    match s.char_indices().nth(skip) {
        Some((byte_idx, _)) => &s[byte_idx..],
        None => s,
    }
}
