#[cfg(target_os = "macos")]
use cidre::{arc, ax, cg};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MeetingPlatform {
    Zoom,
    GoogleMeet,
    MicrosoftTeams,
    Slack,
    Discord,
    Webex,
    Unknown,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MeetingSurface {
    Native,
    Web,
    Unknown,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MeetingChatDirection {
    Incoming,
    Outgoing,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, PartialEq)]
pub struct AxRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MeetingApp {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MeetingParticipantStream {
    pub id: String,
    pub platform: MeetingPlatform,
    pub surface: MeetingSurface,
    pub participant_name: Option<String>,
    pub label: Option<String>,
    pub bounds: Option<AxRect>,
    pub confidence: f32,
    pub is_active_speaker: bool,
    pub signals: Vec<String>,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone)]
pub(super) struct MeetingChatTarget {
    pub(super) kind: String,
    #[cfg(test)]
    pub(super) settable: bool,
    pub(super) confidence: f32,
    #[cfg(test)]
    pub(super) signals: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MeetingAccessibilityInspection {
    pub app: MeetingApp,
    pub pid: i32,
    pub platform: MeetingPlatform,
    pub surface: MeetingSurface,
    pub accessibility_trusted: bool,
    pub window_title: Option<String>,
    pub participant_streams: Vec<MeetingParticipantStream>,
    pub active_speakers: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MeetingChatSendResult {
    pub sent: bool,
    pub app: Option<MeetingApp>,
    pub platform: MeetingPlatform,
    pub surface: MeetingSurface,
    pub input_label: Option<String>,
    pub send_action: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MeetingCapturedChatMessage {
    pub id: String,
    pub platform: MeetingPlatform,
    pub surface: MeetingSurface,
    pub sender: Option<String>,
    pub timestamp: Option<String>,
    pub direction: Option<MeetingChatDirection>,
    pub text: String,
    pub links: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MeetingChatCaptureResult {
    pub app: Option<MeetingApp>,
    pub platform: MeetingPlatform,
    pub surface: MeetingSurface,
    pub context_id: Option<String>,
    pub messages: Vec<MeetingCapturedChatMessage>,
    pub warnings: Vec<String>,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone)]
pub(super) struct AxNode {
    pub(super) index: usize,
    pub(super) tree_path: Vec<usize>,
    pub(super) element_hash: Option<usize>,
    pub(super) role: Option<String>,
    pub(super) identifier: Option<String>,
    pub(super) title: Option<String>,
    pub(super) value: Option<String>,
    pub(super) description: Option<String>,
    pub(super) placeholder: Option<String>,
    pub(super) enabled: Option<bool>,
    pub(super) settable_value: bool,
    pub(super) bounds: Option<AxRect>,
    pub(super) text: String,
    pub(super) within_zoom_meeting_scope: bool,
    pub(super) within_zoom_chat_scope: bool,
    pub(super) within_slack_huddle_scope: bool,
}

#[cfg(target_os = "macos")]
pub(super) struct AxChatElement {
    pub(super) node: AxNode,
    pub(super) ancestors: Vec<AxAncestor>,
    pub(super) element: arc::R<ax::UiElement>,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone)]
pub(super) struct AxAncestor {
    pub(super) path: Vec<usize>,
    pub(super) labels: Vec<String>,
}

#[cfg(target_os = "macos")]
pub(super) struct SlackHuddleRoot {
    pub(super) channel: String,
    pub(super) label: String,
    pub(super) nodes: Vec<AxNode>,
    pub(super) element: arc::R<ax::UiElement>,
}

#[cfg(target_os = "macos")]
pub(super) struct BrowserMeetingRoot {
    pub(super) platform: MeetingPlatform,
    pub(super) window_title: Option<String>,
    pub(super) web_area_url: Option<String>,
    pub(super) nodes: Vec<AxNode>,
}

#[cfg(target_os = "macos")]
pub(super) struct NativeMeetingRoot {
    pub(super) window_title: Option<String>,
    pub(super) nodes: Vec<AxNode>,
}

#[cfg(target_os = "macos")]
#[derive(Debug, PartialEq, Eq)]
pub(super) enum UniqueMatch {
    Missing,
    One(usize),
    Ambiguous,
}

#[cfg(target_os = "macos")]
impl From<cg::Rect> for AxRect {
    fn from(rect: cg::Rect) -> Self {
        Self {
            x: rect.origin.x,
            y: rect.origin.y,
            width: rect.size.width,
            height: rect.size.height,
        }
    }
}
