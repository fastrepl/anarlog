use std::time::SystemTime;

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CaptureAccess {
    None,
    #[default]
    Metadata,
    Url,
    Full,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PolicyMode {
    #[default]
    OptIn,
    OptOut,
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct BundleRule {
    pub bundle_id: String,
    pub access: CaptureAccess,
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct DomainRule {
    pub domain: String,
    pub include_subdomains: bool,
    pub access: CaptureAccess,
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct BrowserPolicy {
    pub rules: Vec<DomainRule>,
    pub require_url_for_text_access: bool,
    pub block_private_browsing: bool,
    pub strip_query: bool,
    pub strip_fragment: bool,
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct CapturePolicy {
    pub mode: PolicyMode,
    pub app_rules: Vec<BundleRule>,
    pub browser: BrowserPolicy,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum AppIdKind {
    BundleId,
    ExecutablePath,
    ProcessName,
    Pid,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ContentLevel {
    Metadata,
    Url,
    Full,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotSource {
    Accessibility,
    Workspace,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    ForegroundWindow,
    Browser,
    AudioSession,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TextAnchorKind {
    FocusedEdit,
    SelectedText,
    FocusedElement,
    Document,
    None,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TextAnchorConfidence {
    High,
    Medium,
    Low,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct AppIdentity {
    pub pid: i32,
    pub app_name: String,
    pub app_id: String,
    pub app_id_kind: AppIdKind,
    pub bundle_id: Option<String>,
    pub executable_path: Option<String>,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct BrowserContext {
    pub raw_url: Option<String>,
    pub is_private: bool,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct CaptureCandidate {
    pub app: AppIdentity,
    pub activity_kind: ActivityKind,
    pub source: SnapshotSource,
    pub browser: Option<BrowserContext>,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct SanitizedBrowserUrl {
    pub url: String,
    pub host: Option<String>,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct CaptureDecision {
    pub access: CaptureAccess,
    pub skip: bool,
    pub source: SnapshotSource,
    pub activity_kind: ActivityKind,
    pub url: Option<String>,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct TextAnchor {
    pub kind: TextAnchorKind,
    pub identity: String,
    pub text: Option<String>,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub selected_text: Option<String>,
    pub confidence: TextAnchorConfidence,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct SnapshotSpec {
    pub captured_at: SystemTime,
    pub app: AppIdentity,
    pub activity_kind: ActivityKind,
    pub access: CaptureAccess,
    pub source: SnapshotSource,
    pub window_title: Option<String>,
    pub url: Option<String>,
    pub visible_text: Option<String>,
    pub text_anchor: Option<TextAnchor>,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct Snapshot {
    pub app: AppIdentity,
    pub activity_kind: ActivityKind,
    pub captured_at: SystemTime,
    pub pid: i32,
    pub app_name: String,
    pub bundle_id: Option<String>,
    pub window_title: Option<String>,
    pub url: Option<String>,
    pub visible_text: Option<String>,
    pub text_anchor_kind: Option<TextAnchorKind>,
    pub text_anchor_identity: Option<String>,
    pub text_anchor_text: Option<String>,
    pub text_anchor_prefix: Option<String>,
    pub text_anchor_suffix: Option<String>,
    pub text_anchor_selected_text: Option<String>,
    pub text_anchor_confidence: Option<TextAnchorConfidence>,
    pub content_level: ContentLevel,
    pub source: SnapshotSource,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct Event {
    pub started_at: SystemTime,
    pub ended_at: SystemTime,
    pub fingerprint: String,
    pub snapshot: Snapshot,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TransitionReason {
    Started,
    Idle,
    AppChanged,
    ActivityKindChanged,
    UrlChanged,
    TitleChanged,
    TextAnchorChanged,
    ContentChanged,
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct Transition {
    pub previous: Option<Event>,
    pub current: Option<Event>,
    pub reason: TransitionReason,
    pub sequence: u64,
    pub suppressed_snapshot_count: u32,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub struct Capabilities {
    pub can_watch: bool,
    pub can_capture_visible_text: bool,
    pub can_capture_browser_url: bool,
    pub requires_accessibility_permission: bool,
}
