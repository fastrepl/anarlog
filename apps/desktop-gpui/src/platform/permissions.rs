use std::sync::Arc;

use desktop_runtime::{Result, ServiceError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    Microphone,
    SystemAudio,
    Accessibility,
    Calendar,
}

#[derive(Clone, Debug)]
pub enum Status {
    Granted,
    Denied(String),
    NotRequired,
}

#[cfg(target_os = "macos")]
const NOT_REQUESTED: &str = "Not requested yet";

#[cfg(not(target_os = "macos"))]
fn probed(result: std::result::Result<(), impl std::fmt::Display>) -> Status {
    match result {
        Ok(()) => Status::Granted,
        Err(error) => Status::Denied(error.to_string()),
    }
}

#[derive(Clone)]
pub struct NativePermissions {
    audio: Arc<dyn anlg_audio::AudioProvider>,
}

impl NativePermissions {
    pub fn new(audio: Arc<dyn anlg_audio::AudioProvider>) -> Self {
        Self { audio }
    }

    /// Reads the current permission without prompting. On macOS this queries the
    /// authorization status like shipping; opening a device here would trigger the OS prompt.
    pub fn check(&self, permission: Permission) -> Status {
        match permission {
            Permission::Microphone => self.microphone(),
            Permission::SystemAudio => self.system_audio(),
            Permission::Accessibility => accessibility(false),
            Permission::Calendar => calendar(false),
        }
    }

    #[cfg(target_os = "macos")]
    fn microphone(&self) -> Status {
        use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};
        let Some(media_type) = (unsafe { AVMediaTypeAudio }) else {
            return Status::Denied("Audio media type unavailable".into());
        };
        match unsafe { AVCaptureDevice::authorizationStatusForMediaType(media_type) } {
            AVAuthorizationStatus::Authorized => Status::Granted,
            AVAuthorizationStatus::NotDetermined => Status::Denied(NOT_REQUESTED.into()),
            _ => Status::Denied("Microphone access denied in System Settings".into()),
        }
    }

    #[cfg(target_os = "macos")]
    fn system_audio(&self) -> Status {
        match anlg_tcc::audio_capture_permission_status() {
            0 => Status::Granted,
            2 => Status::Denied(NOT_REQUESTED.into()),
            _ => Status::Denied("System audio recording denied in System Settings".into()),
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn microphone(&self) -> Status {
        probed(self.audio.probe_mic(None))
    }

    #[cfg(not(target_os = "macos"))]
    fn system_audio(&self) -> Status {
        probed(self.audio.probe_speaker())
    }

    pub fn request(&self, permission: Permission) -> Status {
        if super::os::headless() {
            return self.check(permission);
        }
        match permission {
            #[cfg(target_os = "macos")]
            Permission::Microphone => request_microphone(),
            Permission::SystemAudio => {
                let stop = self.audio.play_silence();
                let status = self.check(permission);
                let _ = stop.send(());
                status
            }
            Permission::Accessibility => accessibility(true),
            Permission::Calendar => calendar(true),
            #[cfg(not(target_os = "macos"))]
            Permission::Microphone => self.check(permission),
        }
    }

    pub fn open_settings(&self, permission: Permission) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            let _ = permission;
            if super::os::headless() {
                return Ok(());
            }
            for (command, args) in [
                ("gnome-control-center", vec!["sound"]),
                ("systemsettings", vec!["kcm_pulseaudio"]),
                ("pavucontrol", vec![]),
            ] {
                match std::process::Command::new(command).args(args).spawn() {
                    Ok(_) => return Ok(()),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => return Err(ServiceError::Failed(error.to_string().into())),
                }
            }
            Err(ServiceError::Failed("Open your desktop's Sound settings and allow the microphone and PipeWire portal capture for Anarlog.".into()))
        }
        #[cfg(not(target_os = "linux"))]
        {
            let url = settings_url(permission);
            super::os::open(url).map_err(|e| ServiceError::Failed(e.to_string().into()))
        }
    }
}

#[cfg(target_os = "macos")]
fn request_microphone() -> Status {
    let (sender, receiver) = std::sync::mpsc::channel();
    let completion = block2::RcBlock::new(move |granted: objc2::runtime::Bool| {
        let _ = sender.send(granted.as_bool());
    });
    unsafe {
        let Some(media_type) = objc2_av_foundation::AVMediaTypeAudio else {
            return Status::Denied("Audio media type unavailable".into());
        };
        objc2_av_foundation::AVCaptureDevice::requestAccessForMediaType_completionHandler(
            media_type,
            &completion,
        );
    }
    match receiver.recv_timeout(std::time::Duration::from_secs(60)) {
        Ok(true) => Status::Granted,
        Ok(false) => Status::Denied("Microphone access denied in System Settings".into()),
        Err(error) => Status::Denied(error.to_string()),
    }
}

#[cfg(not(target_os = "linux"))]
fn settings_url(permission: Permission) -> &'static str {
    if cfg!(target_os = "macos") {
        match permission {
            Permission::Microphone => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
            }
            Permission::SystemAudio => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_AudioCapture"
            }
            Permission::Accessibility => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            }
            Permission::Calendar => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Calendars"
            }
        }
    } else if cfg!(target_os = "windows") {
        match permission {
            Permission::Calendar => "ms-settings:privacy-calendar",
            Permission::Accessibility => "ms-settings:easeofaccess",
            _ => "ms-settings:privacy-microphone",
        }
    } else {
        "settings://privacy"
    }
}

#[cfg(target_os = "macos")]
fn accessibility(request: bool) -> Status {
    let granted = if request {
        macos_accessibility_client::accessibility::application_is_trusted_with_prompt()
    } else {
        macos_accessibility_client::accessibility::application_is_trusted()
    };
    if granted {
        Status::Granted
    } else {
        Status::Denied("Enable Anarlog in Privacy & Security → Accessibility".into())
    }
}

#[cfg(not(target_os = "macos"))]
fn accessibility(_: bool) -> Status {
    Status::NotRequired
}

#[cfg(target_os = "macos")]
fn calendar(request: bool) -> Status {
    let granted = if request {
        anlg_apple_calendar::Handle::request_full_access()
    } else {
        anlg_apple_calendar::Handle::authorization_status()
            == anlg_apple_calendar::CalendarAuthStatus::Authorized
    };
    if granted {
        Status::Granted
    } else {
        Status::Denied("Allow calendar access in System Settings".into())
    }
}

#[cfg(not(target_os = "macos"))]
fn calendar(_: bool) -> Status {
    Status::NotRequired
}
