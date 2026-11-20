//! Shared pieces for driving a native meeting app over Accessibility: which app a call is in,
//! which windows and controls only a call shows, and the one lock that every action taking the
//! front or pressing keys holds (the chat notice and the camera switch). Provenance: crates/detect/PROVENANCE.md.

#[cfg(any(target_os = "macos", test))]
pub(crate) mod matching;

#[cfg(target_os = "macos")]
pub(crate) mod macos;

/// A Mac meeting app whose call Anarlog drives itself. Google Meet in a browser is not one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeMeetingApp {
    Zoom,
    Teams,
    Slack,
    Webex,
}

impl NativeMeetingApp {
    pub const ALL: [NativeMeetingApp; 4] = [
        NativeMeetingApp::Zoom,
        NativeMeetingApp::Teams,
        NativeMeetingApp::Webex,
        NativeMeetingApp::Slack,
    ];

    pub fn bundle_id(self) -> &'static str {
        match self {
            NativeMeetingApp::Zoom => "us.zoom.xos",
            NativeMeetingApp::Teams => "com.microsoft.teams2",
            NativeMeetingApp::Slack => "com.tinyspeck.slackmacgap",
            NativeMeetingApp::Webex => "Cisco-Systems.Spark",
        }
    }

    pub fn from_bundle_id(bundle_id: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|app| app.bundle_id() == bundle_id.trim())
    }

    /// Teams draws its call in a web view whose controls Accessibility can read only while Teams
    /// is the front app; seconds after it loses the front they drop out of the tree.
    pub fn shows_call_only_in_front(self) -> bool {
        self == NativeMeetingApp::Teams
    }

    pub fn as_str(self) -> &'static str {
        match self {
            NativeMeetingApp::Zoom => "zoom",
            NativeMeetingApp::Teams => "teams",
            NativeMeetingApp::Slack => "slack",
            NativeMeetingApp::Webex => "webex",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|app| app.as_str() == value)
    }
}

#[cfg(test)]
mod tests {
    use super::NativeMeetingApp;

    #[test]
    fn names_and_bundles_round_trip() {
        for app in NativeMeetingApp::ALL {
            assert_eq!(NativeMeetingApp::parse(app.as_str()), Some(app));
            assert_eq!(NativeMeetingApp::from_bundle_id(app.bundle_id()), Some(app));
        }
        // A browser or Teams classic is not a native app Anarlog drives.
        assert_eq!(NativeMeetingApp::from_bundle_id("com.google.Chrome"), None);
        assert_eq!(
            NativeMeetingApp::from_bundle_id("com.microsoft.teams"),
            None
        );
    }
}
