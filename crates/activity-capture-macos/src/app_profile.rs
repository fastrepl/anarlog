#![cfg(target_os = "macos")]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AppProfile {
    Generic,
    Safari,
    Chrome,
    Arc,
    Brave,
    Edge,
    Slack,
    Spotify,
}

impl AppProfile {
    pub(crate) fn from_bundle_id(bundle_id: Option<&str>) -> Self {
        match bundle_id {
            Some("com.apple.Safari") => Self::Safari,
            Some("com.google.Chrome") => Self::Chrome,
            Some("company.thebrowser.Browser") => Self::Arc,
            Some("com.brave.Browser") => Self::Brave,
            Some("com.microsoft.edgemac") => Self::Edge,
            Some("com.tinyspeck.slackmacgap") | Some("com.slack.Slack") => Self::Slack,
            Some("com.spotify.client") => Self::Spotify,
            _ => Self::Generic,
        }
    }

    pub(crate) fn is_browser(self) -> bool {
        matches!(
            self,
            Self::Safari | Self::Chrome | Self::Arc | Self::Brave | Self::Edge
        )
    }

    pub(crate) fn is_slack(self) -> bool {
        self == Self::Slack
    }

    pub(crate) fn supports_private_window_detection(self) -> bool {
        matches!(self, Self::Chrome | Self::Arc | Self::Brave | Self::Edge)
    }

    pub(crate) fn browser_bundle_id(self) -> Option<&'static str> {
        match self {
            Self::Chrome => Some("com.google.Chrome"),
            Self::Arc => Some("company.thebrowser.Browser"),
            Self::Brave => Some("com.brave.Browser"),
            Self::Edge => Some("com.microsoft.edgemac"),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AppProfile;

    #[test]
    fn classifies_supported_bundles() {
        assert_eq!(
            AppProfile::from_bundle_id(Some("com.apple.Safari")),
            AppProfile::Safari
        );
        assert_eq!(
            AppProfile::from_bundle_id(Some("com.google.Chrome")),
            AppProfile::Chrome
        );
        assert_eq!(
            AppProfile::from_bundle_id(Some("company.thebrowser.Browser")),
            AppProfile::Arc
        );
        assert_eq!(
            AppProfile::from_bundle_id(Some("com.brave.Browser")),
            AppProfile::Brave
        );
        assert_eq!(
            AppProfile::from_bundle_id(Some("com.microsoft.edgemac")),
            AppProfile::Edge
        );
        assert_eq!(
            AppProfile::from_bundle_id(Some("com.tinyspeck.slackmacgap")),
            AppProfile::Slack
        );
        assert_eq!(
            AppProfile::from_bundle_id(Some("com.spotify.client")),
            AppProfile::Spotify
        );
        assert_eq!(
            AppProfile::from_bundle_id(Some("com.example.Unknown")),
            AppProfile::Generic
        );
    }

    #[test]
    fn reports_browser_capabilities() {
        assert!(AppProfile::Safari.is_browser());
        assert!(AppProfile::Chrome.supports_private_window_detection());
        assert!(!AppProfile::Safari.supports_private_window_detection());
        assert!(AppProfile::Slack.is_slack());
        assert!(!AppProfile::Spotify.is_browser());
    }
}
