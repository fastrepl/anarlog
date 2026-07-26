#[derive(Debug, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum PermissionStatus {
    NeverRequested,
    Denied,
    Authorized,
}

#[cfg(target_os = "macos")]
use objc2_av_foundation::AVAuthorizationStatus;
#[cfg(target_os = "macos")]
use objc2_contacts::CNAuthorizationStatus;
#[cfg(target_os = "macos")]
use objc2_event_kit::EKAuthorizationStatus;

#[cfg(target_os = "macos")]
impl From<isize> for PermissionStatus {
    fn from(status: isize) -> Self {
        match status {
            hypr_tcc::GRANTED => Self::Authorized,
            hypr_tcc::NEVER_ASKED => Self::NeverRequested,
            _ => Self::Denied,
        }
    }
}

impl From<bool> for PermissionStatus {
    fn from(status: bool) -> Self {
        if status {
            Self::Authorized
        } else {
            Self::Denied
        }
    }
}

#[cfg(target_os = "macos")]
impl From<AVAuthorizationStatus> for PermissionStatus {
    fn from(status: AVAuthorizationStatus) -> Self {
        match status {
            AVAuthorizationStatus::NotDetermined => Self::NeverRequested,
            AVAuthorizationStatus::Authorized => Self::Authorized,
            _ => Self::Denied,
        }
    }
}

#[cfg(target_os = "macos")]
impl From<EKAuthorizationStatus> for PermissionStatus {
    fn from(status: EKAuthorizationStatus) -> Self {
        match status {
            EKAuthorizationStatus::NotDetermined => Self::NeverRequested,
            EKAuthorizationStatus::FullAccess => Self::Authorized,
            _ => Self::Denied,
        }
    }
}

#[cfg(target_os = "macos")]
impl From<CNAuthorizationStatus> for PermissionStatus {
    fn from(status: CNAuthorizationStatus) -> Self {
        match status {
            CNAuthorizationStatus::NotDetermined => Self::NeverRequested,
            CNAuthorizationStatus::Authorized => Self::Authorized,
            _ => Self::Denied,
        }
    }
}
