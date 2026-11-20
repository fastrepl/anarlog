//! Switches the camera a call uses in a Mac meeting app (Zoom, Microsoft Teams, Webex) to a named
//! camera, such as Anarlog Camera, through Accessibility: find the call's window, open the camera
//! picker beside its camera button, choose the camera by its exact name, and check the choice
//! took. It only changes which camera is used; a camera that is off stays off, and the camera
//! button itself is never pressed. Google Meet is the browser extension's job, and Slack huddles
//! aren't handled.
//!
//! A meeting app's own background or blur cuts out everything but the person, Anarlog Camera's
//! badge included. [`read_background`] tells which effect the call shows, so Anarlog Camera can put
//! its own background behind the person first, and only then [`turn_off_background`] turns the
//! app's off, so the room never shows in between. Both work in Zoom and Teams. Webex keeps its
//! backgrounds in tiles Accessibility can't choose and shows no choice, so it answers
//! `Unsupported` and stays manual.
//!
//! Every action takes the one meeting-app automation lock the chat notice also takes, so two
//! automations never fight over the front app or keys. `cancelled` is asked before each step that
//! changes the app; once it says yes, nothing more is pressed.
//!
//! Ported from Char `crates/desktop-agent-automation/src/meeting_camera`; see
//! `tools/camera-watermark/PROVENANCE.md`.

use std::time::Duration;

pub use crate::meeting_automation::NativeMeetingApp;
pub use matching::BackgroundEffect;

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod matching;

#[cfg(target_os = "macos")]
mod macos;

/// What happened to the camera. Only `Switched` means the app now uses the camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Switched,
    /// The app shows no call.
    NoCall,
    /// The call has no camera picker Anarlog could find or open (a layout it doesn't know, or
    /// meeting controls hidden).
    PickerUnavailable,
    /// The picker lists no camera by that name.
    CameraMissing,
    /// Anarlog chose the camera but couldn't confirm the app took it.
    Unconfirmed,
    /// The caller cancelled before the camera was chosen; the app's camera is as it was.
    Cancelled,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Switched => "switched",
            Outcome::NoCall => "no_call",
            Outcome::PickerUnavailable => "picker_unavailable",
            Outcome::CameraMissing => "camera_missing",
            Outcome::Unconfirmed => "unconfirmed",
            Outcome::Cancelled => "cancelled",
        }
    }
}

/// What happened to the app's own background or blur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundOutcome {
    /// The app showed no background effect.
    AlreadyOff,
    /// Anarlog turned this effect off and read the app's choice back as none.
    TurnedOff(BackgroundEffect),
    /// Anarlog started turning this effect off but couldn't read the app's choice back: it may be
    /// off or on.
    Unconfirmed(BackgroundEffect),
    /// The app shows no call.
    NoCall,
    /// Anarlog couldn't find or read the app's background controls; nothing was changed.
    Unavailable,
    /// The app's background controls can't be read or chosen through Accessibility (Webex).
    Unsupported,
    /// The caller cancelled before anything in the app was changed.
    Cancelled,
}

/// What the call's own background control shows, before Anarlog changes anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundReading {
    /// The app shows no background effect.
    Off,
    /// The app draws this effect behind the person.
    On(BackgroundEffect),
    /// The app shows no call.
    NoCall,
    /// Anarlog couldn't find the controls, or they didn't show exactly one choice in use.
    Unavailable,
    /// The app's background controls can't be read through Accessibility (Webex).
    Unsupported,
}

impl BackgroundReading {
    pub fn as_str(self) -> &'static str {
        match self {
            BackgroundReading::Off => "off",
            BackgroundReading::On(_) => "on",
            BackgroundReading::NoCall => "no_call",
            BackgroundReading::Unavailable => "unavailable",
            BackgroundReading::Unsupported => "unsupported",
        }
    }

    pub fn effect(self) -> Option<BackgroundEffect> {
        match self {
            BackgroundReading::On(effect) => Some(effect),
            _ => None,
        }
    }
}

impl BackgroundOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            BackgroundOutcome::AlreadyOff => "already_off",
            BackgroundOutcome::TurnedOff(_) => "turned_off",
            BackgroundOutcome::Unconfirmed(_) => "unconfirmed",
            BackgroundOutcome::NoCall => "no_call",
            BackgroundOutcome::Unavailable => "unavailable",
            BackgroundOutcome::Unsupported => "unsupported",
            BackgroundOutcome::Cancelled => "cancelled",
        }
    }

    /// The effect the app showed before Anarlog turned it off.
    pub fn effect(self) -> Option<BackgroundEffect> {
        match self {
            BackgroundOutcome::TurnedOff(effect) | BackgroundOutcome::Unconfirmed(effect) => {
                Some(effect)
            }
            _ => None,
        }
    }

    /// Whether the app's effect may have changed: from here on, a rollback of Anarlog's own
    /// background would risk showing the room.
    pub fn may_have_changed(self) -> bool {
        matches!(
            self,
            BackgroundOutcome::TurnedOff(_) | BackgroundOutcome::Unconfirmed(_)
        )
    }
}

impl BackgroundEffect {
    pub fn as_str(self) -> &'static str {
        match self {
            BackgroundEffect::Blur => "blur",
            BackgroundEffect::Image => "image",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Accessibility permission is required to switch the meeting camera")]
    PermissionRequired,
    #[error("Switching the camera isn't supported for this app")]
    UnsupportedApp,
    #[cfg(not(target_os = "macos"))]
    #[error("Switching the meeting camera is unsupported on this platform")]
    Unsupported,
    #[error("Meeting camera accessibility action failed: {0}")]
    Accessibility(String),
}

#[cfg(target_os = "macos")]
impl From<crate::meeting_automation::macos::AxFailure> for Error {
    fn from(failure: crate::meeting_automation::macos::AxFailure) -> Self {
        Error::Accessibility(failure.0)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Whether this platform can switch meeting cameras at all.
pub fn is_supported() -> bool {
    cfg!(target_os = "macos")
}

/// Switches the call `app` shows in process `pid` to the camera named `camera`. Only a pid the
/// recorder confirmed may be passed: Anarlog never guesses which running copy holds the call.
#[cfg(target_os = "macos")]
pub fn select_camera(
    app: NativeMeetingApp,
    pid: i32,
    camera: &str,
    timeout: Duration,
    cancelled: &dyn Fn() -> bool,
) -> Result<Outcome> {
    macos::select_camera(app, pid, camera, timeout, cancelled)
}

#[cfg(not(target_os = "macos"))]
pub fn select_camera(
    _app: NativeMeetingApp,
    _pid: i32,
    _camera: &str,
    _timeout: Duration,
    _cancelled: &dyn Fn() -> bool,
) -> Result<Outcome> {
    Err(Error::Unsupported)
}

/// Reads the background or blur the call in `app` draws behind the person, changing nothing.
#[cfg(target_os = "macos")]
pub fn read_background(
    app: NativeMeetingApp,
    pid: i32,
    timeout: Duration,
) -> Result<BackgroundReading> {
    macos::read_background(app, pid, timeout)
}

#[cfg(not(target_os = "macos"))]
pub fn read_background(
    _app: NativeMeetingApp,
    _pid: i32,
    _timeout: Duration,
) -> Result<BackgroundReading> {
    Err(Error::Unsupported)
}

/// Turns off the background or blur the call in `app` draws behind the person, if any. `known` is
/// the effect [`read_background`] just read, which spares Zoom opening its Settings again.
#[cfg(target_os = "macos")]
pub fn turn_off_background(
    app: NativeMeetingApp,
    pid: i32,
    known: Option<BackgroundEffect>,
    timeout: Duration,
    cancelled: &dyn Fn() -> bool,
) -> Result<BackgroundOutcome> {
    macos::turn_off_background(app, pid, known, timeout, cancelled)
}

#[cfg(not(target_os = "macos"))]
pub fn turn_off_background(
    _app: NativeMeetingApp,
    _pid: i32,
    _known: Option<BackgroundEffect>,
    _timeout: Duration,
    _cancelled: &dyn Fn() -> bool,
) -> Result<BackgroundOutcome> {
    Err(Error::Unsupported)
}
