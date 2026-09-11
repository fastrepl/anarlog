//! `automations/*` in the desktop app: the starter automations, the custom
//! workflows persisted in the `automation_workflows` setting, and the
//! settings keys each starter writes (`starters.tsx`, `workflows.ts`,
//! `types.ts`).

use chrono::{DateTime, Utc};

pub const WORKFLOWS_KEY: &str = "automation_workflows";
pub const DRAFT_TEMPLATE_KEY: &str = "automation_draft_template";
pub const MARKDOWN_EXPORT_DIRECTORY_KEY: &str = "automation_markdown_export_directory";
pub const SLACK_CHANNEL_KEY: &str = "automation_slack_recap_channel";
pub const LINEAR_TEAM_KEY: &str = "automation_linear_issues_team";
pub const NOTION_PAGE_KEY: &str = "automation_notion_update_page";

/// `STARTER_AUTOMATIONS`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StarterId {
    SlackRecap,
    NotionProjectNotes,
    LinearActionItems,
    MarkdownExport,
}

impl StarterId {
    pub const ALL: [StarterId; 4] = [
        StarterId::SlackRecap,
        StarterId::NotionProjectNotes,
        StarterId::LinearActionItems,
        StarterId::MarkdownExport,
    ];

    /// `isStarterId`
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|id| id.as_str() == value)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            StarterId::SlackRecap => "slack-recap",
            StarterId::NotionProjectNotes => "notion-project-notes",
            StarterId::LinearActionItems => "linear-action-items",
            StarterId::MarkdownExport => "markdown-export",
        }
    }

    pub fn enabled_key(self) -> &'static str {
        match self {
            StarterId::SlackRecap => "automation_slack_recap_enabled",
            StarterId::NotionProjectNotes => "automation_notion_update_enabled",
            StarterId::LinearActionItems => "automation_linear_issues_enabled",
            StarterId::MarkdownExport => "automation_markdown_export_enabled",
        }
    }

    pub fn target_key(self) -> &'static str {
        match self {
            StarterId::SlackRecap => SLACK_CHANNEL_KEY,
            StarterId::NotionProjectNotes => NOTION_PAGE_KEY,
            StarterId::LinearActionItems => LINEAR_TEAM_KEY,
            StarterId::MarkdownExport => MARKDOWN_EXPORT_DIRECTORY_KEY,
        }
    }

    pub fn last_run_key(self) -> &'static str {
        match self {
            StarterId::SlackRecap => "automation_slack_recap_last_run",
            StarterId::NotionProjectNotes => "automation_notion_update_last_run",
            StarterId::LinearActionItems => "automation_linear_issues_last_run",
            StarterId::MarkdownExport => "automation_markdown_export_last_run",
        }
    }

    /// `isReady`: the markdown starter needs a directory, the others a target.
    pub fn is_ready(self, target_raw: &str) -> bool {
        match self {
            StarterId::MarkdownExport => !target_raw.trim().is_empty(),
            _ => parse_target_ref(target_raw).is_some(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepKind {
    Trigger,
    Ai,
    Action,
}

pub struct StarterStep {
    pub kind: StepKind,
    pub title: &'static str,
    pub detail: &'static str,
}

/// `StarterAutomation`; the icon is the brand asset (`logos:*-icon`) or the
/// Markdown mark.
pub struct Starter {
    pub id: StarterId,
    pub title: &'static str,
    pub description: &'static str,
    pub steps: &'static [StarterStep],
    pub preview: &'static str,
}

/// `useStarterAutomations()`
pub fn starters() -> [Starter; 4] {
    [
        Starter {
            id: StarterId::SlackRecap,
            title: "Share a meeting recap in Slack",
            description: "Post a meeting recap to a Slack channel.",
            steps: &[
                StarterStep {
                    kind: StepKind::Trigger,
                    title: "Meeting ends",
                    detail: "Runs once the AI summary for the meeting is ready.",
                },
                StarterStep {
                    kind: StepKind::Ai,
                    title: "Use the AI meeting summary",
                    detail: "Take the enhanced note with decisions and action items.",
                },
                StarterStep {
                    kind: StepKind::Action,
                    title: "Post to a channel",
                    detail: "Send the recap to the selected Slack channel.",
                },
            ],
            preview: "A Slack message with the meeting title and recap.",
        },
        Starter {
            id: StarterId::NotionProjectNotes,
            title: "Update project notes in Notion",
            description: "Add meeting decisions and follow-ups to a Notion project.",
            steps: &[
                StarterStep {
                    kind: StepKind::Trigger,
                    title: "Meeting ends",
                    detail: "Runs once the AI summary for the meeting is ready.",
                },
                StarterStep {
                    kind: StepKind::Ai,
                    title: "Use the AI meeting summary",
                    detail: "Take the enhanced note with decisions and follow-ups.",
                },
                StarterStep {
                    kind: StepKind::Action,
                    title: "Append the meeting update",
                    detail: "Add a dated update to the selected Notion page.",
                },
            ],
            preview: "A dated Notion update with the meeting summary.",
        },
        Starter {
            id: StarterId::LinearActionItems,
            title: "Turn action items into Linear issues",
            description: "Turn assigned follow-ups into Linear issue drafts.",
            steps: &[
                StarterStep {
                    kind: StepKind::Trigger,
                    title: "Meeting ends",
                    detail: "Runs once the AI summary for the meeting is ready.",
                },
                StarterStep {
                    kind: StepKind::Ai,
                    title: "Collect action items",
                    detail: "Use the meeting's action items and summary tasks.",
                },
                StarterStep {
                    kind: StepKind::Action,
                    title: "Create Linear issues",
                    detail: "File each action item as an issue in the selected team.",
                },
            ],
            preview: "Linear issues created from meeting follow-ups.",
        },
        Starter {
            id: StarterId::MarkdownExport,
            title: "Export every meeting as Markdown",
            description: "Save completed meetings as local Markdown files.",
            steps: &[
                StarterStep {
                    kind: StepKind::Trigger,
                    title: "Meeting ends",
                    detail: "Wait until the transcript and note are complete.",
                },
                StarterStep {
                    kind: StepKind::Action,
                    title: "Render canonical Markdown",
                    detail: "Combine metadata, summary, notes, and transcript.",
                },
                StarterStep {
                    kind: StepKind::Action,
                    title: "Write to a folder",
                    detail: "Use a stable filename in the configured export directory.",
                },
            ],
            preview: "A Markdown file with the note, summary, and transcript.",
        },
    ]
}

/// `AutomationTargetRef`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetRef {
    pub id: String,
    pub name: String,
}

impl TargetRef {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({ "id": self.id, "name": self.name })
    }
}

/// `parseAutomationTargetRef`
pub fn parse_target_ref(value: &str) -> Option<TargetRef> {
    if value.is_empty() {
        return None;
    }
    let parsed: serde_json::Value = serde_json::from_str(value).ok()?;
    target_from_value(&parsed)
}

fn target_from_value(value: &serde_json::Value) -> Option<TargetRef> {
    Some(TargetRef {
        id: value.get("id")?.as_str()?.to_string(),
        name: value.get("name")?.as_str()?.to_string(),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunStatus {
    Success,
    Error,
}

/// `AutomationRunRecord`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunRecord {
    pub at: String,
    pub status: RunStatus,
    pub detail: String,
}

impl RunRecord {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "at": self.at,
            "status": match self.status {
                RunStatus::Success => "success",
                RunStatus::Error => "error",
            },
            "detail": self.detail,
        })
    }

    /// `AutomationLastRunLine`
    pub fn line(&self, now: DateTime<Utc>) -> String {
        let relative = self
            .at
            .parse::<DateTime<Utc>>()
            .map(|at| format_distance_to_now(at, now))
            .unwrap_or_else(|_| "Invalid Date".to_string());
        match self.status {
            RunStatus::Success => format!("Last run {relative}: {}", self.detail),
            RunStatus::Error => format!("Last run failed {relative}: {}", self.detail),
        }
    }
}

/// `parseAutomationRunRecord`
pub fn parse_run_record(value: &str) -> Option<RunRecord> {
    if value.is_empty() {
        return None;
    }
    let parsed: serde_json::Value = serde_json::from_str(value).ok()?;
    run_record_from_value(&parsed)
}

fn run_record_from_value(value: &serde_json::Value) -> Option<RunRecord> {
    let status = match value.get("status")?.as_str()? {
        "success" => RunStatus::Success,
        "error" => RunStatus::Error,
        _ => return None,
    };
    Some(RunRecord {
        at: value.get("at")?.as_str()?.to_string(),
        status,
        detail: value.get("detail")?.as_str()?.to_string(),
    })
}

/// `WORKFLOW_TRIGGERS`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    NoteEnhanced,
    MeetingCompleted,
}

impl Trigger {
    pub fn as_str(self) -> &'static str {
        match self {
            Trigger::NoteEnhanced => "note_enhanced",
            Trigger::MeetingCompleted => "meeting_completed",
        }
    }

    /// Unknown values fall back to `note_enhanced` like `parseWorkflow`.
    pub fn parse(value: &str) -> Self {
        if value == "meeting_completed" {
            Trigger::MeetingCompleted
        } else {
            Trigger::NoteEnhanced
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Trigger::NoteEnhanced => "After the meeting summary is ready",
            Trigger::MeetingCompleted => "After the meeting ends",
        }
    }
}

/// `WORKFLOW_STEP_TYPES`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepType {
    SlackRecap,
    NotionUpdate,
    LinearIssues,
    MarkdownExport,
}

impl StepType {
    pub const ALL: [StepType; 4] = [
        StepType::SlackRecap,
        StepType::NotionUpdate,
        StepType::LinearIssues,
        StepType::MarkdownExport,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            StepType::SlackRecap => "slack_recap",
            StepType::NotionUpdate => "notion_update",
            StepType::LinearIssues => "linear_issues",
            StepType::MarkdownExport => "markdown_export",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == value)
    }

    /// The step card's select label.
    pub fn label(self) -> &'static str {
        match self {
            StepType::SlackRecap => "Post a recap to Slack",
            StepType::NotionUpdate => "Append an update to Notion",
            StepType::LinearIssues => "Create Linear issues from action items",
            StepType::MarkdownExport => "Export the meeting as Markdown",
        }
    }

    /// The `Add step` select label.
    pub fn short_label(self) -> &'static str {
        match self {
            StepType::SlackRecap => "Slack recap",
            StepType::NotionUpdate => "Notion update",
            StepType::LinearIssues => "Linear issues",
            StepType::MarkdownExport => "Markdown export",
        }
    }
}

/// `WorkflowStep`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub id: String,
    pub kind: StepType,
    /// `target` for the integration steps.
    pub target: Option<TargetRef>,
    /// `directory` for `markdown_export`.
    pub directory: String,
}

impl Step {
    /// `createWorkflowStep`
    pub fn new(kind: StepType) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            target: None,
            directory: String::new(),
        }
    }

    /// `isWorkflowStepReady`
    pub fn is_ready(&self) -> bool {
        match self.kind {
            StepType::MarkdownExport => !self.directory.trim().is_empty(),
            _ => self.target.is_some(),
        }
    }

    fn to_json(&self) -> serde_json::Value {
        match self.kind {
            StepType::MarkdownExport => serde_json::json!({
                "id": self.id,
                "type": self.kind.as_str(),
                "directory": self.directory,
            }),
            _ => serde_json::json!({
                "id": self.id,
                "type": self.kind.as_str(),
                "target": self.target.as_ref().map(TargetRef::to_json),
            }),
        }
    }

    fn from_value(value: &serde_json::Value) -> Option<Self> {
        let id = value.get("id")?.as_str()?.to_string();
        let kind = StepType::parse(value.get("type")?.as_str()?)?;
        Some(match kind {
            StepType::MarkdownExport => Step {
                id,
                kind,
                target: None,
                directory: value
                    .get("directory")
                    .and_then(|d| d.as_str())
                    .unwrap_or_default()
                    .to_string(),
            },
            _ => Step {
                id,
                kind,
                target: value.get("target").and_then(target_from_value),
                directory: String::new(),
            },
        })
    }
}

/// `AutomationWorkflow`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Workflow {
    pub id: String,
    pub title: String,
    pub enabled: bool,
    pub trigger: Trigger,
    pub steps: Vec<Step>,
    pub last_run: Option<RunRecord>,
    pub processed_session_ids: Vec<String>,
    pub chat_group_id: Option<String>,
}

impl Workflow {
    /// `createEmptyWorkflow()`
    pub fn new_empty() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: "Untitled automation".to_string(),
            enabled: false,
            trigger: Trigger::NoteEnhanced,
            steps: Vec::new(),
            last_run: None,
            processed_session_ids: Vec::new(),
            chat_group_id: None,
        }
    }

    /// `isWorkflowReady`
    pub fn is_ready(&self) -> bool {
        !self.steps.is_empty() && self.steps.iter().all(Step::is_ready)
    }

    /// The list / header title with the untitled fallback.
    pub fn display_title(&self) -> String {
        let title = self.title.trim();
        if title.is_empty() {
            "Untitled automation".to_string()
        } else {
            title.to_string()
        }
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "title": self.title,
            "enabled": self.enabled,
            "trigger": self.trigger.as_str(),
            "steps": self.steps.iter().map(Step::to_json).collect::<Vec<_>>(),
            "lastRun": self.last_run.as_ref().map(RunRecord::to_json),
            "processedSessionIds": self.processed_session_ids,
            "chatGroupId": self.chat_group_id,
        })
    }

    fn from_value(value: &serde_json::Value) -> Option<Self> {
        let id = value.get("id")?.as_str()?.to_string();
        Some(Self {
            id,
            title: value
                .get("title")
                .and_then(|t| t.as_str())
                .unwrap_or("Untitled automation")
                .to_string(),
            enabled: value.get("enabled").and_then(|e| e.as_bool()) == Some(true),
            trigger: Trigger::parse(value.get("trigger").and_then(|t| t.as_str()).unwrap_or("")),
            steps: value
                .get("steps")
                .and_then(|s| s.as_array())
                .map(|steps| steps.iter().filter_map(Step::from_value).collect())
                .unwrap_or_default(),
            last_run: value.get("lastRun").and_then(run_record_from_value),
            processed_session_ids: value
                .get("processedSessionIds")
                .and_then(|ids| ids.as_array())
                .map(|ids| {
                    ids.iter()
                        .filter_map(|id| id.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            chat_group_id: value
                .get("chatGroupId")
                .and_then(|id| id.as_str())
                .map(str::to_string),
        })
    }
}

/// `parseAutomationWorkflows`: anything that is not a JSON array of workflow
/// objects yields nothing; malformed entries are skipped.
pub fn parse_workflows(value: &str) -> Vec<Workflow> {
    if value.is_empty() {
        return Vec::new();
    }
    let Ok(serde_json::Value::Array(items)) = serde_json::from_str::<serde_json::Value>(value)
    else {
        return Vec::new();
    };
    items.iter().filter_map(Workflow::from_value).collect()
}

/// `serializeAutomationWorkflows`: `JSON.stringify` with the object literals'
/// key order.
pub fn serialize_workflows(workflows: &[Workflow]) -> String {
    serde_json::Value::Array(workflows.iter().map(Workflow::to_json).collect()).to_string()
}

/// `saveAutomationWorkflows` after `useSaveWorkflow`: replace by id, or
/// prepend a new workflow.
pub fn upsert_workflow(workflows: &[Workflow], next: Workflow) -> Vec<Workflow> {
    if workflows.iter().any(|workflow| workflow.id == next.id) {
        workflows
            .iter()
            .map(|workflow| {
                if workflow.id == next.id {
                    next.clone()
                } else {
                    workflow.clone()
                }
            })
            .collect()
    } else {
        std::iter::once(next)
            .chain(workflows.iter().cloned())
            .collect()
    }
}

/// date-fns `formatDistanceToNow(date, { addSuffix: true })`.
pub fn format_distance_to_now(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let seconds = (now - at).num_seconds();
    let future = seconds < 0;
    let seconds = seconds.abs();
    let minutes = (seconds as f64 / 60.0).round() as i64;
    let distance = if seconds < 30 {
        "less than a minute".to_string()
    } else if minutes < 45 {
        if minutes == 1 {
            "1 minute".to_string()
        } else {
            format!("{minutes} minutes")
        }
    } else if minutes < 90 {
        "about 1 hour".to_string()
    } else if minutes < 24 * 60 {
        format!("about {} hours", (minutes as f64 / 60.0).round() as i64)
    } else if minutes < 42 * 60 {
        "1 day".to_string()
    } else if minutes < 30 * 24 * 60 {
        format!("{} days", (minutes as f64 / (24.0 * 60.0)).round() as i64)
    } else if minutes < 45 * 24 * 60 {
        "about 1 month".to_string()
    } else if minutes < 60 * 24 * 60 {
        "about 2 months".to_string()
    } else if minutes < 12 * 30 * 24 * 60 {
        format!(
            "{} months",
            (minutes as f64 / (30.0 * 24.0 * 60.0)).round() as i64
        )
    } else {
        let months = (minutes as f64 / (30.0 * 24.0 * 60.0)).floor() as i64;
        let years = months / 12;
        match months % 12 {
            0..=2 => {
                if years == 1 {
                    "about 1 year".to_string()
                } else {
                    format!("about {years} years")
                }
            }
            3..=8 => {
                if years == 1 {
                    "over 1 year".to_string()
                } else {
                    format!("over {years} years")
                }
            }
            _ => {
                if years + 1 == 1 {
                    "almost 1 year".to_string()
                } else {
                    format!("almost {} years", years + 1)
                }
            }
        }
    };
    if future {
        format!("in {distance}")
    } else {
        format!("{distance} ago")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_ids_round_trip_and_map_to_their_setting_keys() {
        for id in StarterId::ALL {
            assert_eq!(StarterId::parse(id.as_str()), Some(id));
        }
        assert_eq!(StarterId::parse("zapier"), None);
        assert_eq!(
            StarterId::SlackRecap.enabled_key(),
            "automation_slack_recap_enabled"
        );
        assert_eq!(
            StarterId::NotionProjectNotes.target_key(),
            "automation_notion_update_page"
        );
        assert_eq!(
            StarterId::LinearActionItems.last_run_key(),
            "automation_linear_issues_last_run"
        );
        assert!(StarterId::MarkdownExport.is_ready(" /tmp/out "));
        assert!(!StarterId::MarkdownExport.is_ready("  "));
        assert!(StarterId::SlackRecap.is_ready(r#"{"id":"C1","name":"general"}"#));
        assert!(!StarterId::SlackRecap.is_ready("general"));
        assert_eq!(starters().len(), 4);
        assert_eq!(starters()[3].steps.len(), 3);
    }

    #[test]
    fn workflows_parse_leniently_and_serialize_in_literal_order() {
        let raw = r#"[
            {"id":"w1","title":"Recap","enabled":true,"trigger":"meeting_completed",
             "steps":[
               {"id":"s1","type":"slack_recap","target":{"id":"C1","name":"general"}},
               {"id":"s2","type":"markdown_export","directory":"/tmp/out"},
               {"id":"s3","type":"unknown"},
               "junk"
             ],
             "lastRun":{"at":"2026-09-06T10:00:00.000Z","status":"error","detail":"boom"},
             "processedSessionIds":["a",1,"b"],"chatGroupId":"g1"},
            {"id":"w2","trigger":"bogus"},
            {"title":"no id"},
            5
        ]"#;
        let workflows = parse_workflows(raw);
        assert_eq!(workflows.len(), 2);
        let first = &workflows[0];
        assert_eq!(first.trigger, Trigger::MeetingCompleted);
        assert_eq!(first.steps.len(), 2);
        assert_eq!(
            first.steps[0].target,
            Some(TargetRef {
                id: "C1".into(),
                name: "general".into()
            })
        );
        assert_eq!(first.steps[1].directory, "/tmp/out");
        assert_eq!(first.last_run.as_ref().unwrap().status, RunStatus::Error);
        assert_eq!(first.processed_session_ids, vec!["a", "b"]);
        assert_eq!(first.chat_group_id.as_deref(), Some("g1"));
        assert!(first.is_ready());
        let second = &workflows[1];
        assert_eq!(second.title, "Untitled automation");
        assert!(!second.enabled);
        assert_eq!(second.trigger, Trigger::NoteEnhanced);
        assert!(!second.is_ready());

        assert_eq!(
            serialize_workflows(&workflows[..1]),
            r#"[{"id":"w1","title":"Recap","enabled":true,"trigger":"meeting_completed","steps":[{"id":"s1","type":"slack_recap","target":{"id":"C1","name":"general"}},{"id":"s2","type":"markdown_export","directory":"/tmp/out"}],"lastRun":{"at":"2026-09-06T10:00:00.000Z","status":"error","detail":"boom"},"processedSessionIds":["a","b"],"chatGroupId":"g1"}]"#
        );
        assert_eq!(
            serialize_workflows(&workflows[1..]),
            r#"[{"id":"w2","title":"Untitled automation","enabled":false,"trigger":"note_enhanced","steps":[],"lastRun":null,"processedSessionIds":[],"chatGroupId":null}]"#
        );
        assert!(parse_workflows("").is_empty());
        assert!(parse_workflows("{}").is_empty());
        assert!(parse_workflows("not json").is_empty());
    }

    #[test]
    fn upsert_replaces_or_prepends() {
        let a = Workflow {
            id: "a".into(),
            ..Workflow::new_empty()
        };
        let b = Workflow {
            id: "b".into(),
            ..Workflow::new_empty()
        };
        let updated = Workflow {
            title: "Renamed".into(),
            ..a.clone()
        };
        let list = upsert_workflow(&[a.clone(), b.clone()], updated.clone());
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].title, "Renamed");
        let c = Workflow {
            id: "c".into(),
            ..Workflow::new_empty()
        };
        let list = upsert_workflow(&[a, b], c);
        assert_eq!(list[0].id, "c");
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn steps_know_their_readiness() {
        let mut step = Step::new(StepType::SlackRecap);
        assert!(!step.is_ready());
        step.target = Some(TargetRef {
            id: "C1".into(),
            name: "general".into(),
        });
        assert!(step.is_ready());
        let mut export = Step::new(StepType::MarkdownExport);
        assert!(!export.is_ready());
        export.directory = "/tmp".into();
        assert!(export.is_ready());
        assert_eq!(parse_target_ref(r#"{"id":"x"}"#), None);
        assert_eq!(
            parse_run_record(r#"{"at":"t","status":"odd","detail":""}"#),
            None
        );
    }

    #[test]
    fn relative_distances_follow_date_fns() {
        let now: DateTime<Utc> = "2026-09-06T12:00:00Z".parse().unwrap();
        let at = |s: &str| s.parse::<DateTime<Utc>>().unwrap();
        assert_eq!(
            format_distance_to_now(at("2026-09-06T11:59:45Z"), now),
            "less than a minute ago"
        );
        assert_eq!(
            format_distance_to_now(at("2026-09-06T11:59:00Z"), now),
            "1 minute ago"
        );
        assert_eq!(
            format_distance_to_now(at("2026-09-06T11:20:00Z"), now),
            "40 minutes ago"
        );
        assert_eq!(
            format_distance_to_now(at("2026-09-06T11:00:00Z"), now),
            "about 1 hour ago"
        );
        assert_eq!(
            format_distance_to_now(at("2026-09-06T09:00:00Z"), now),
            "about 3 hours ago"
        );
        assert_eq!(
            format_distance_to_now(at("2026-09-05T12:00:00Z"), now),
            "1 day ago"
        );
        assert_eq!(
            format_distance_to_now(at("2026-09-01T12:00:00Z"), now),
            "5 days ago"
        );
        assert_eq!(
            format_distance_to_now(at("2026-08-01T12:00:00Z"), now),
            "about 1 month ago"
        );
        assert_eq!(
            format_distance_to_now(at("2026-03-01T12:00:00Z"), now),
            "6 months ago"
        );
        assert_eq!(
            format_distance_to_now(at("2025-08-01T12:00:00Z"), now),
            "about 1 year ago"
        );
        assert_eq!(
            format_distance_to_now(at("2026-09-06T12:10:00Z"), now),
            "in 10 minutes"
        );
        let run = RunRecord {
            at: "2026-09-06T11:00:00Z".into(),
            status: RunStatus::Success,
            detail: "Posted to #general".into(),
        };
        assert_eq!(
            run.line(now),
            "Last run about 1 hour ago: Posted to #general"
        );
    }
}
