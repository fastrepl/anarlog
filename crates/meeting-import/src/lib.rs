mod error;
pub mod fathom;
pub mod google_meet;
mod json;
pub mod plaud;
pub mod plaud_cli;
pub mod teams;
mod time;
pub mod webex;

pub use anlg_zoom::{TranscriptSegment, parse_vtt};
pub use error::Error;
pub use json::{
    ImportFile, ImportedMeeting, meeting_file, meeting_file_with_scheme, meeting_has_content,
    nonempty, safe_file_component,
};
pub use time::{duration_or_timestamp_to_ms, hhmmss_to_ms, rfc3339_to_ms};
