//! Posts one message into the chat of a call running in a Mac app (Zoom, Microsoft Teams, Webex,
//! Slack huddles), through Accessibility: find the call's chat input, open the chat if it is
//! closed, put the text in, check it is exactly the message, press Return, and check it left the
//! input. Google Meet in a browser is not posted from here. Provenance: crates/detect/PROVENANCE.md.
//!
//! Return is pressed at most once per call to `post_notice`. Outcomes tell a message that can't
//! have gone out from one that might have (`Sent`, `Unconfirmed`), so a caller never posts again
//! after a Return.

use std::time::Duration;

pub use crate::meeting_automation::NativeMeetingApp;

#[cfg(any(target_os = "macos", test))]
pub(crate) mod matching;

#[cfg(any(target_os = "macos", test))]
mod send;

#[cfg(target_os = "macos")]
mod macos;

/// The longest notice Anarlog posts.
pub const MAX_NOTICE_CHARS: usize = 500;

/// What happened to the message. Only `Sent` means it left the input; only `Sent` and
/// `Unconfirmed` follow a Return press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Sent,
    /// The process shows no call, or it is no longer the app the call was confirmed in.
    NoCall,
    /// The call has no chat input Anarlog could find or open (chat turned off, or an unknown
    /// layout).
    ChatUnavailable,
    /// Someone is typing in the chat; their draft is left alone.
    DraftInProgress,
    /// Anarlog stopped before pressing Return (the app didn't come to the front, take the text,
    /// or keep the input focused) and took its text back out: nothing was sent.
    NotSent,
    /// The caller cancelled before Return was pressed; Anarlog's text, if it went in, was taken
    /// out.
    Cancelled,
    /// Return was pressed, but the message didn't leave the input in time: it may have been sent.
    Unconfirmed,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Sent => "sent",
            Outcome::NoCall => "no_call",
            Outcome::ChatUnavailable => "chat_unavailable",
            Outcome::DraftInProgress => "draft_in_progress",
            Outcome::NotSent => "not_sent",
            Outcome::Cancelled => "cancelled",
            Outcome::Unconfirmed => "unconfirmed",
        }
    }

    /// The message may be in the call's chat, so it is never posted there again.
    pub fn may_have_sent(self) -> bool {
        matches!(self, Outcome::Sent | Outcome::Unconfirmed)
    }
}

/// Every error is raised before Return could be pressed: the send step itself is infallible and
/// reports through `Outcome`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Accessibility permission is required to post in the meeting chat")]
    PermissionRequired,
    #[error("the notice must be 1 to {MAX_NOTICE_CHARS} characters")]
    InvalidMessage,
    #[cfg(not(target_os = "macos"))]
    #[error("Posting in a meeting chat is unsupported on this platform")]
    Unsupported,
    #[error("Meeting chat accessibility action failed: {0}")]
    Accessibility(String),
}

#[cfg(target_os = "macos")]
impl From<crate::meeting_automation::macos::AxFailure> for Error {
    fn from(failure: crate::meeting_automation::macos::AxFailure) -> Self {
        Error::Accessibility(failure.0)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// The notice Anarlog posts, trimmed, or an error when it is empty or too long.
pub fn validate_notice(message: &str) -> Result<&str> {
    let message = message.trim();
    if message.is_empty() || message.chars().count() > MAX_NOTICE_CHARS {
        return Err(Error::InvalidMessage);
    }
    Ok(message)
}

/// Posts `message` in the chat of the call that process `pid` of `app` shows. `pid` must be the
/// process the call was confirmed in; nothing else is tried. `cancelled` is asked before each step
/// up to Return; once it says yes, nothing more is pressed.
#[cfg(target_os = "macos")]
pub fn post_notice(
    app: NativeMeetingApp,
    pid: i32,
    message: &str,
    timeout: Duration,
    cancelled: &dyn Fn() -> bool,
) -> Result<Outcome> {
    let message = validate_notice(message)?;
    macos::post_notice(app, pid, message, timeout, cancelled)
}

#[cfg(not(target_os = "macos"))]
pub fn post_notice(
    _app: NativeMeetingApp,
    _pid: i32,
    message: &str,
    _timeout: Duration,
    _cancelled: &dyn Fn() -> bool,
) -> Result<Outcome> {
    validate_notice(message)?;
    Err(Error::Unsupported)
}

/// The bundle identifier of a running process, for telling which app a call is in.
#[cfg(target_os = "macos")]
pub fn bundle_id_of(pid: i32) -> Option<String> {
    crate::meeting_automation::macos::applications::bundle_id_of(pid)
}

#[cfg(not(target_os = "macos"))]
pub fn bundle_id_of(_pid: i32) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notices_are_one_to_five_hundred_characters() {
        assert_eq!(validate_notice("  hello  ").unwrap(), "hello");
        assert!(validate_notice("   ").is_err());
        assert!(validate_notice(&"a".repeat(MAX_NOTICE_CHARS)).is_ok());
        assert!(validate_notice(&"é".repeat(MAX_NOTICE_CHARS + 1)).is_err());
    }

    #[test]
    fn only_sent_and_unconfirmed_may_have_sent() {
        let all = [
            Outcome::Sent,
            Outcome::NoCall,
            Outcome::ChatUnavailable,
            Outcome::DraftInProgress,
            Outcome::NotSent,
            Outcome::Cancelled,
            Outcome::Unconfirmed,
        ];
        let may: Vec<_> = all.into_iter().filter(|o| o.may_have_sent()).collect();
        assert_eq!(may, vec![Outcome::Sent, Outcome::Unconfirmed]);
    }
}
