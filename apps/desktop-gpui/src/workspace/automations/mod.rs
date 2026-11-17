//! The automations route's data: shipping's `automations/workflows.ts`, `starters.tsx` and
//! `actions.ts` over the `automation_*` rows of `app_settings`.

pub mod assets;
pub mod view;

use std::{collections::HashMap, sync::Arc};

use anlg_db_execute::DbExecutor;
use desktop_runtime::{Reply, Result, RuntimeHandle, ServiceError};
use serde_json::{Value, json};
use uuid::Uuid;

use super::mutations::{NOW, decode_json_text, failure, statement, transaction};

pub const WORKFLOWS: &str = "automation_workflows";
pub const DRAFT_TEMPLATE: &str = "automation_draft_template";
pub const DRIVE_STARTER_ID: &str = "starter-google-drive";
const MAX_WORKFLOWS: usize = 100;

pub const SETTINGS_SQL: &str = "SELECT id, value_json FROM app_settings WHERE id IN (
    'automation_workflows', 'automation_draft_template',
    'automation_markdown_export_enabled', 'automation_markdown_export_directory',
    'automation_markdown_export_last_run',
    'automation_slack_recap_enabled', 'automation_slack_recap_channel',
    'automation_slack_recap_last_run',
    'automation_linear_issues_enabled', 'automation_linear_issues_team',
    'automation_linear_issues_last_run',
    'automation_notion_update_enabled', 'automation_notion_update_page',
    'automation_notion_update_last_run')";

/// Chat groups whose messages were sent from the automations scope (`useChatGroups`).
pub const CHATS_SQL: &str = "SELECT g.id, g.title, g.created_at FROM chat_groups AS g
    WHERE g.deleted_at IS NULL AND EXISTS (SELECT 1 FROM chat_messages AS m
        WHERE m.chat_group_id = g.id AND m.deleted_at IS NULL
        AND CASE WHEN json_valid(m.metadata_json)
            THEN json_extract(m.metadata_json, '$.chatScope') ELSE NULL END = 'automations')
    ORDER BY g.created_at DESC, g.id DESC";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Trigger {
    NoteEnhanced,
    MeetingCompleted,
}

impl Trigger {
    pub fn key(self) -> &'static str {
        match self {
            Self::NoteEnhanced => "note_enhanced",
            Self::MeetingCompleted => "meeting_completed",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::NoteEnhanced => "After the meeting summary is ready",
            Self::MeetingCompleted => "After the meeting ends",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Target {
    pub id: Arc<str>,
    pub name: Arc<str>,
}

impl Target {
    fn parse(value: &Value) -> Option<Self> {
        Some(Self {
            id: value["id"].as_str()?.into(),
            name: value["name"].as_str()?.into(),
        })
    }

    /// `parseAutomationTargetRef`: a stored JSON string holding `{id, name}`.
    pub fn parse_stored(raw: &str) -> Option<Self> {
        Self::parse(&serde_json::from_str(raw).ok()?)
    }

    fn to_value(&self) -> Value {
        json!({"id": self.id, "name": self.name})
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkdownOptions {
    pub memo: bool,
    pub summary: bool,
    pub transcript: bool,
    pub action_items: bool,
    pub filename: String,
    pub id_suffix: bool,
}

impl Default for MarkdownOptions {
    fn default() -> Self {
        Self {
            memo: true,
            summary: true,
            transcript: true,
            action_items: true,
            filename: String::new(),
            id_suffix: true,
        }
    }
}

impl MarkdownOptions {
    fn parse(value: &Value) -> Self {
        let flag = |key: &str, default| value[key].as_bool().unwrap_or(default);
        Self {
            memo: flag("include_memo", true),
            summary: flag("include_summary", true),
            transcript: flag("include_transcript", true),
            action_items: flag("include_action_items", true),
            filename: value["filename"].as_str().unwrap_or_default().into(),
            id_suffix: flag("include_id_suffix", true),
        }
    }

    fn to_value(&self) -> Value {
        json!({
            "include_memo": self.memo,
            "include_summary": self.summary,
            "include_transcript": self.transcript,
            "include_action_items": self.action_items,
            "filename": self.filename,
            "include_id_suffix": self.id_suffix,
        })
    }

    pub fn has_content(&self) -> bool {
        self.memo || self.summary || self.transcript || self.action_items
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepType {
    Slack,
    Notion,
    Linear,
    Drive,
    Markdown,
}

impl StepType {
    pub const ALL: [Self; 5] = [
        Self::Slack,
        Self::Notion,
        Self::Linear,
        Self::Drive,
        Self::Markdown,
    ];

    fn key(self) -> &'static str {
        match self {
            Self::Slack => "slack_recap",
            Self::Notion => "notion_update",
            Self::Linear => "linear_issues",
            Self::Drive => "google_drive_export",
            Self::Markdown => "markdown_export",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.key() == key)
    }

    /// The label in a step's action select.
    pub fn label(self) -> &'static str {
        match self {
            Self::Slack => "Post a recap to Slack",
            Self::Notion => "Append an update to Notion",
            Self::Linear => "Create Linear issues from action items",
            Self::Drive => "Save to Google Drive",
            Self::Markdown => "Export the meeting as Markdown",
        }
    }

    /// The label in the "Add step" select.
    pub fn short_label(self) -> &'static str {
        match self {
            Self::Slack => "Slack recap",
            Self::Notion => "Notion update",
            Self::Linear => "Linear issues",
            Self::Drive => "Save to Google Drive",
            Self::Markdown => "Markdown export",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Step {
    Target {
        id: Arc<str>,
        kind: StepType,
        target: Option<Target>,
    },
    Drive {
        id: Arc<str>,
        google_docs: bool,
        connection_id: String,
        target: Option<Target>,
    },
    Markdown {
        id: Arc<str>,
        directory: String,
        options: Option<MarkdownOptions>,
    },
}

impl Step {
    /// `createWorkflowStep`.
    pub fn new(kind: StepType) -> Self {
        let id: Arc<str> = Uuid::new_v4().to_string().into();
        match kind {
            StepType::Drive => Self::Drive {
                id,
                google_docs: false,
                connection_id: String::new(),
                target: None,
            },
            StepType::Markdown => Self::Markdown {
                id,
                directory: String::new(),
                options: Some(MarkdownOptions::default()),
            },
            kind => Self::Target {
                id,
                kind,
                target: None,
            },
        }
    }

    pub fn id(&self) -> &Arc<str> {
        match self {
            Self::Target { id, .. } | Self::Drive { id, .. } | Self::Markdown { id, .. } => id,
        }
    }

    pub fn kind(&self) -> StepType {
        match self {
            Self::Target { kind, .. } => *kind,
            Self::Drive { .. } => StepType::Drive,
            Self::Markdown { .. } => StepType::Markdown,
        }
    }

    fn parse(value: &Value) -> Option<Self> {
        let id: Arc<str> = value["id"].as_str()?.into();
        let target = Target::parse(&value["target"]);
        Some(match StepType::from_key(value["type"].as_str()?)? {
            StepType::Drive => Self::Drive {
                id,
                google_docs: value["format"] == "google_docs",
                connection_id: value["connectionId"].as_str().unwrap_or_default().into(),
                target,
            },
            StepType::Markdown => Self::Markdown {
                id,
                directory: value["directory"].as_str().unwrap_or_default().into(),
                options: (!value["options"].is_null())
                    .then(|| MarkdownOptions::parse(&value["options"])),
            },
            kind => Self::Target { id, kind, target },
        })
    }

    fn to_value(&self) -> Value {
        let target =
            |target: &Option<Target>| target.as_ref().map_or(Value::Null, Target::to_value);
        match self {
            Self::Target {
                id,
                kind,
                target: t,
            } => {
                json!({"id": id, "type": kind.key(), "target": target(t)})
            }
            Self::Drive {
                id,
                google_docs,
                connection_id,
                target: t,
            } => {
                let mut value = json!({
                    "id": id, "type": "google_drive_export",
                    "connectionId": connection_id, "target": target(t),
                });
                if *google_docs {
                    value["format"] = json!("google_docs");
                }
                value
            }
            Self::Markdown {
                id,
                directory,
                options,
            } => {
                let mut value =
                    json!({"id": id, "type": "markdown_export", "directory": directory});
                if let Some(options) = options {
                    value["options"] = options.to_value();
                }
                value
            }
        }
    }

    /// `isWorkflowStepReady`.
    pub fn ready(&self) -> bool {
        match self {
            Self::Target { target, .. } => target.is_some(),
            Self::Drive {
                connection_id,
                target,
                ..
            } => !connection_id.is_empty() && target.as_ref().is_some_and(|t| !t.id.is_empty()),
            Self::Markdown {
                directory, options, ..
            } => {
                !directory.trim().is_empty()
                    && options.as_ref().is_none_or(MarkdownOptions::has_content)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunRecord {
    pub at: Arc<str>,
    pub success: bool,
    pub detail: Arc<str>,
}

impl RunRecord {
    /// `parseLastRun`: only finished runs; the native runner's "running" marker is hidden.
    fn parse(value: &Value) -> Option<Self> {
        let success = match value["status"].as_str()? {
            "success" => true,
            "error" => false,
            _ => return None,
        };
        Some(Self {
            at: value["at"].as_str()?.into(),
            success,
            detail: value["detail"].as_str()?.into(),
        })
    }

    pub fn parse_stored(raw: &str) -> Option<Self> {
        Self::parse(&serde_json::from_str(raw).ok()?)
    }
}

/// One Google Drive upload attempt (`DriveExportRun`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DriveRun {
    pub step_id: Arc<str>,
    pub connection_id: Arc<str>,
    pub folder_id: Arc<str>,
    pub google_docs: bool,
    pub file_id: Option<Arc<str>>,
    pub success: bool,
    pub detail: Arc<str>,
    pub at: Arc<str>,
}

impl DriveRun {
    fn parse(value: &Value) -> Option<Self> {
        let text = |key: &str| value[key].as_str().map(Arc::from);
        text("sessionId")?;
        Some(Self {
            step_id: text("stepId")?,
            connection_id: text("connectionId")?,
            folder_id: text("folderId")?,
            google_docs: value["format"] == "google_docs",
            file_id: text("fileId"),
            success: match value["status"].as_str()? {
                "success" => true,
                "pending" | "error" => false,
                _ => return None,
            },
            detail: text("detail")?,
            at: text("at")?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Workflow {
    pub id: Arc<str>,
    pub title: String,
    pub enabled: bool,
    pub trigger: Trigger,
    pub steps: Vec<Step>,
    pub last_run: Option<RunRecord>,
    pub chat_group_id: Option<Arc<str>>,
    pub drive_runs: Vec<DriveRun>,
}

impl Workflow {
    /// `createEmptyWorkflow`.
    pub fn empty(id: Option<Arc<str>>, chat_group_id: Option<Arc<str>>, title: &str) -> Self {
        Self {
            id: id.unwrap_or_else(|| Uuid::new_v4().to_string().into()),
            title: title.into(),
            enabled: false,
            trigger: Trigger::NoteEnhanced,
            steps: Vec::new(),
            last_run: None,
            chat_group_id,
            drive_runs: Vec::new(),
        }
    }

    /// `createGoogleDriveWorkflow` under the starter's fixed id.
    pub fn drive_starter() -> Self {
        let mut workflow = Self::empty(Some(DRIVE_STARTER_ID.into()), None, STARTERS[0].title);
        workflow.steps.push(Step::Drive {
            id: Uuid::new_v4().to_string().into(),
            google_docs: false,
            connection_id: String::new(),
            target: None,
        });
        workflow
    }

    /// `parseWorkflow`: lenient like shipping; unknown steps are dropped.
    fn parse(value: &Value) -> Option<Self> {
        Some(Self {
            id: value["id"].as_str()?.into(),
            title: value["title"]
                .as_str()
                .unwrap_or("Untitled automation")
                .into(),
            enabled: value["enabled"] == true,
            trigger: if value["trigger"] == "meeting_completed" {
                Trigger::MeetingCompleted
            } else {
                Trigger::NoteEnhanced
            },
            steps: value["steps"]
                .as_array()
                .map(|steps| steps.iter().filter_map(Step::parse).collect())
                .unwrap_or_default(),
            last_run: RunRecord::parse(&value["lastRun"]),
            chat_group_id: value["chatGroupId"].as_str().map(Arc::from),
            drive_runs: value["driveExports"]
                .as_array()
                .map(|runs| runs.iter().filter_map(DriveRun::parse).collect())
                .unwrap_or_default(),
        })
    }

    /// The edited fields over `stored`, whose run history (`lastRun`, `processedSessionIds`,
    /// `driveExports`) is kept, as `useSaveWorkflow` does.
    fn merge_into(&self, stored: Option<&Value>) -> Value {
        let mut value = stored
            .cloned()
            .unwrap_or_else(|| json!({"lastRun": null, "processedSessionIds": []}));
        value["id"] = json!(self.id);
        value["title"] = json!(self.title);
        value["enabled"] = json!(self.enabled && self.ready());
        value["trigger"] = json!(self.trigger.key());
        value["steps"] = Value::Array(self.steps.iter().map(Step::to_value).collect());
        value["chatGroupId"] = json!(self.chat_group_id);
        value
    }

    /// `isWorkflowReady`.
    pub fn ready(&self) -> bool {
        !self.steps.is_empty()
            && self.steps.iter().all(Step::ready)
            && (self.trigger == Trigger::NoteEnhanced
                || !self.steps.iter().any(|step| step.kind() == StepType::Drive))
    }

    pub fn display_title(&self) -> &str {
        match self.title.trim() {
            "" => "Untitled automation",
            title => title,
        }
    }

    pub fn has_drive(&self) -> bool {
        self.steps.iter().any(|step| step.kind() == StepType::Drive)
    }

    /// The Drive result `DriveExportResult` shows for `step`: an unfinished run first.
    pub fn drive_result(&self, step: &Step) -> Option<&DriveRun> {
        let Step::Drive {
            id,
            google_docs,
            connection_id,
            target,
        } = step
        else {
            return None;
        };
        let mut runs = self
            .drive_runs
            .iter()
            .filter(|run| {
                &run.step_id == id
                    && run.connection_id.as_ref() == connection_id
                    && Some(&run.folder_id) == target.as_ref().map(|target| &target.id)
                    && run.google_docs == *google_docs
            })
            .collect::<Vec<_>>();
        runs.sort_by(|a, b| b.at.cmp(&a.at));
        runs.iter()
            .find(|run| !run.success)
            .or(runs.first())
            .copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepKind {
    Trigger,
    Ai,
    Action,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StarterId {
    GoogleDrive,
    SlackRecap,
    NotionProjectNotes,
    LinearActionItems,
    MarkdownExport,
}

impl StarterId {
    fn key(self) -> &'static str {
        match self {
            Self::GoogleDrive => "google-drive",
            Self::SlackRecap => "slack-recap",
            Self::NotionProjectNotes => "notion-project-notes",
            Self::LinearActionItems => "linear-action-items",
            Self::MarkdownExport => "markdown-export",
        }
    }

    pub fn parse(key: &str) -> Option<Self> {
        STARTERS
            .iter()
            .map(|starter| starter.id)
            .find(|id| id.key() == key)
    }

    /// The legacy settings behind every starter except Drive: enabled, target and last run.
    pub fn keys(self) -> Option<[&'static str; 3]> {
        Some(match self {
            Self::GoogleDrive => return None,
            Self::SlackRecap => [
                "automation_slack_recap_enabled",
                "automation_slack_recap_channel",
                "automation_slack_recap_last_run",
            ],
            Self::NotionProjectNotes => [
                "automation_notion_update_enabled",
                "automation_notion_update_page",
                "automation_notion_update_last_run",
            ],
            Self::LinearActionItems => [
                "automation_linear_issues_enabled",
                "automation_linear_issues_team",
                "automation_linear_issues_last_run",
            ],
            Self::MarkdownExport => [
                "automation_markdown_export_enabled",
                "automation_markdown_export_directory",
                "automation_markdown_export_last_run",
            ],
        })
    }

    pub fn readiness_hint(self) -> &'static str {
        match self {
            Self::GoogleDrive => "Choose a Google Drive folder first.",
            Self::MarkdownExport => "Choose an export folder first.",
            Self::SlackRecap => "Choose a Slack channel first.",
            Self::LinearActionItems => "Choose a Linear team first.",
            Self::NotionProjectNotes => "Choose a Notion page first.",
        }
    }
}

pub struct Starter {
    pub id: StarterId,
    pub title: &'static str,
    pub description: &'static str,
    pub steps: [(StepKind, &'static str, &'static str); 3],
    pub preview: &'static str,
}

pub const STARTERS: [Starter; 5] = [
    Starter {
        id: StarterId::GoogleDrive,
        title: "Save meetings to Google Drive",
        description: "Save meeting summaries and transcripts to a Google Drive folder.",
        steps: [
            (
                StepKind::Trigger,
                "Meeting ends",
                "Wait until the meeting recording ends.",
            ),
            (
                StepKind::Ai,
                "Summary ready",
                "Runs once the AI summary for the meeting is ready.",
            ),
            (
                StepKind::Action,
                "Upload to Google Drive folder",
                "Save the summary and transcript to the selected folder.",
            ),
        ],
        preview: "A Markdown file or Google document with the meeting summary and transcript.",
    },
    Starter {
        id: StarterId::SlackRecap,
        title: "Share a meeting recap in Slack",
        description: "Post a meeting recap to a Slack channel.",
        steps: [
            (
                StepKind::Trigger,
                "Meeting ends",
                "Runs once the AI summary for the meeting is ready.",
            ),
            (
                StepKind::Ai,
                "Use the AI meeting summary",
                "Take the enhanced note with decisions and action items.",
            ),
            (
                StepKind::Action,
                "Post to a channel",
                "Send the recap to the selected Slack channel.",
            ),
        ],
        preview: "A Slack message with the meeting title and recap.",
    },
    Starter {
        id: StarterId::NotionProjectNotes,
        title: "Update project notes in Notion",
        description: "Add meeting decisions and follow-ups to a Notion project.",
        steps: [
            (
                StepKind::Trigger,
                "Meeting ends",
                "Runs once the AI summary for the meeting is ready.",
            ),
            (
                StepKind::Ai,
                "Use the AI meeting summary",
                "Take the enhanced note with decisions and follow-ups.",
            ),
            (
                StepKind::Action,
                "Append the meeting update",
                "Add a dated update to the selected Notion page.",
            ),
        ],
        preview: "A dated Notion update with the meeting summary.",
    },
    Starter {
        id: StarterId::LinearActionItems,
        title: "Turn action items into Linear issues",
        description: "Turn assigned follow-ups into Linear issue drafts.",
        steps: [
            (
                StepKind::Trigger,
                "Meeting ends",
                "Runs once the AI summary for the meeting is ready.",
            ),
            (
                StepKind::Ai,
                "Collect action items",
                "Use the meeting's action items and summary tasks.",
            ),
            (
                StepKind::Action,
                "Create Linear issues",
                "File each action item as an issue in the selected team.",
            ),
        ],
        preview: "Linear issues created from meeting follow-ups.",
    },
    Starter {
        id: StarterId::MarkdownExport,
        title: "Export every meeting as Markdown",
        description: "Save completed meetings as local Markdown files.",
        steps: [
            (
                StepKind::Trigger,
                "Meeting ends",
                "Wait until the transcript and note are complete.",
            ),
            (
                StepKind::Action,
                "Render canonical Markdown",
                "Combine metadata, summary, notes, and transcript.",
            ),
            (
                StepKind::Action,
                "Write to a folder",
                "Use a stable filename in the configured export directory.",
            ),
        ],
        preview: "A Markdown file with the note, summary, and transcript.",
    },
];

pub fn starter(id: StarterId) -> &'static Starter {
    STARTERS
        .iter()
        .find(|starter| starter.id == id)
        .expect("every starter id has a starter")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatAutomation {
    pub id: Arc<str>,
    pub title: Arc<str>,
    pub created_at: Arc<str>,
}

pub fn decode_chats(rows: &[Value]) -> Vec<ChatAutomation> {
    rows.iter()
        .filter_map(|row| {
            Some(ChatAutomation {
                id: row["id"].as_str()?.into(),
                title: row["title"].as_str().unwrap_or_default().into(),
                created_at: row["created_at"].as_str().unwrap_or_default().into(),
            })
        })
        .collect()
}

/// The stored automation settings, each value decoded from its `value_json`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Settings {
    values: HashMap<String, Value>,
    pub workflows: Vec<Workflow>,
}

impl Settings {
    pub fn decode(rows: &[Value]) -> Self {
        let values: HashMap<String, Value> = rows
            .iter()
            .filter_map(|row| {
                let value = serde_json::from_str(row["value_json"].as_str()?).ok()?;
                Some((row["id"].as_str()?.to_owned(), value))
            })
            .collect();
        let workflows = values
            .get(WORKFLOWS)
            .and_then(Value::as_str)
            .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
            .and_then(|list| list.as_array().cloned())
            .map(|list| list.iter().filter_map(Workflow::parse).collect())
            .unwrap_or_default();
        Self { values, workflows }
    }

    pub fn flag(&self, key: &str) -> bool {
        self.values.get(key) == Some(&Value::Bool(true))
    }

    pub fn text(&self, key: &str) -> &str {
        self.values
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
    }

    pub fn draft_template(&self) -> Option<StarterId> {
        StarterId::parse(self.text(DRAFT_TEMPLATE))
    }

    pub fn workflow(&self, id: &str) -> Option<&Workflow> {
        self.workflows
            .iter()
            .find(|workflow| workflow.id.as_ref() == id)
    }

    pub fn drive_starter(&self) -> Workflow {
        self.workflow(DRIVE_STARTER_ID)
            .cloned()
            .unwrap_or_else(Workflow::drive_starter)
    }

    pub fn starter_enabled(&self, id: StarterId) -> bool {
        match id.keys() {
            Some([enabled, ..]) => self.flag(enabled),
            None => self.drive_starter().enabled,
        }
    }

    pub fn starter_ready(&self, id: StarterId) -> bool {
        match id.keys() {
            None => self.drive_starter().ready(),
            Some([_, target, _]) if id == StarterId::MarkdownExport => {
                !self.text(target).trim().is_empty()
            }
            Some([_, target, _]) => Target::parse_stored(self.text(target)).is_some(),
        }
    }

    pub fn starter_last_run(&self, id: StarterId) -> Option<RunRecord> {
        RunRecord::parse_stored(self.text(id.keys()?[2]))
    }
}

/// One stored workflow's raw item.
pub async fn stored_workflow(executor: &DbExecutor, id: &str) -> Result<Option<Value>> {
    let (_, list) = stored_workflows(executor).await?;
    Ok(list.into_iter().find(|workflow| workflow["id"] == id))
}

/// Shipping's shared copy of a workflow: disabled, without its run history or any destination
/// that points into the sharer's own accounts and folders.
pub fn clear_shared_state(workflow: &mut Value) {
    for step in workflow["steps"].as_array_mut().into_iter().flatten() {
        match step["type"].as_str() {
            Some("google_drive_export") => {
                step["connectionId"] = json!("");
                step["target"] = Value::Null;
            }
            Some("markdown_export") => step["directory"] = json!(""),
            _ => step["target"] = Value::Null,
        }
    }
    workflow["enabled"] = json!(false);
    workflow["lastRun"] = Value::Null;
    workflow["processedSessionIds"] = json!([]);
    workflow["driveExports"] = json!([]);
    workflow["chatGroupId"] = Value::Null;
}

/// `importSharedAutomation`: a cleared copy under a new id, first in the list.
pub async fn import_shared(executor: &DbExecutor, workflow: &Value) -> Result<Arc<str>> {
    if Workflow::parse(workflow).is_none() {
        return Err(failure("This shared automation is invalid"));
    }
    let id: Arc<str> = Uuid::new_v4().to_string().into();
    let mut copy = workflow.clone();
    copy["id"] = json!(id);
    clear_shared_state(&mut copy);
    mutate_workflows(executor, |list| {
        list.insert(0, copy.clone());
        Ok(())
    })
    .await?;
    Ok(id)
}

/// Reads the stored list's raw items. Unreadable storage is an error, so a write never
/// replaces it.
async fn stored_workflows(executor: &DbExecutor) -> Result<(Option<String>, Vec<Value>)> {
    let rows = executor
        .execute(
            "SELECT value_json FROM app_settings WHERE id='automation_workflows'".into(),
            vec![],
        )
        .await
        .map_err(failure)?;
    let raw = rows
        .first()
        .and_then(|row| row["value_json"].as_str())
        .map(str::to_owned);
    let list = match raw.as_deref() {
        None => Vec::new(),
        Some(raw) => decode_json_text(raw)?
            .as_array()
            .cloned()
            .ok_or_else(|| failure("Invalid automation workflow storage"))?,
    };
    Ok((raw, list))
}

/// `mutateAutomationWorkflows`: a read-modify-write that retries when another writer (the
/// runner recording a run) changed the list in between. Shipping stores the list as a JSON
/// string setting.
async fn mutate_workflows(
    executor: &DbExecutor,
    mut update: impl FnMut(&mut Vec<Value>) -> Result<()>,
) -> Result<()> {
    for _ in 0..3 {
        let (raw, mut list) = stored_workflows(executor).await?;
        update(&mut list)?;
        if list.len() > MAX_WORKFLOWS {
            return Err(failure("Automations are limited to 100 workflows"));
        }
        let encoded = json!(serde_json::to_string(&list).map_err(failure)?).to_string();
        let write = match &raw {
            Some(raw) => statement(
                format!("UPDATE app_settings SET value_json=?,updated_at={NOW} WHERE id='automation_workflows' AND value_json=?"),
                vec![json!(encoded), json!(raw)],
                Some(1),
            ),
            None => statement(
                "INSERT INTO app_settings(id,value_json) SELECT 'automation_workflows',? WHERE NOT EXISTS(SELECT 1 FROM app_settings WHERE id='automation_workflows')".into(),
                vec![json!(encoded)],
                Some(1),
            ),
        };
        match transaction(executor, vec![write]).await {
            Err(ServiceError::Conflict) => continue,
            result => return result,
        }
    }
    Err(ServiceError::Conflict)
}

pub(super) async fn set_settings(
    executor: &DbExecutor,
    values: Vec<(&'static str, Value)>,
) -> Result<()> {
    if values.is_empty() {
        return Ok(());
    }
    transaction(
        executor,
        values
            .into_iter()
            .map(|(key, value)| {
                statement(
                    format!("INSERT INTO app_settings(id,value_json) VALUES (?,?) ON CONFLICT(id) DO UPDATE SET value_json=excluded.value_json,updated_at={NOW}"),
                    vec![json!(key), json!(value.to_string())],
                    Some(1),
                )
            })
            .collect(),
    )
    .await
}

/// `useSaveWorkflow`: new workflows go first; enabling requires a ready workflow.
pub fn save(runtime: &RuntimeHandle, workflow: Workflow) -> Result<Reply<()>> {
    runtime.submit(move |services| async move {
        mutate_workflows(&services.executor, |list| {
            match list
                .iter_mut()
                .find(|item| item["id"] == workflow.id.as_ref())
            {
                Some(item) => *item = workflow.merge_into(Some(item)),
                None => list.insert(0, workflow.merge_into(None)),
            }
            Ok(())
        })
        .await
    })
}

/// Stores `workflow` unless a workflow with its id or chat group already exists
/// (`useEnsuredWorkflow`).
pub fn ensure(runtime: &RuntimeHandle, workflow: Workflow) -> Result<Reply<()>> {
    runtime.submit(move |services| async move {
        mutate_workflows(&services.executor, |list| {
            let exists = list.iter().any(|item| {
                item["id"] == workflow.id.as_ref()
                    || workflow
                        .chat_group_id
                        .as_ref()
                        .is_some_and(|group| item["chatGroupId"] == group.as_ref())
            });
            if !exists {
                list.insert(0, workflow.merge_into(None));
            }
            Ok(())
        })
        .await
    })
}

/// `useDeleteWorkflow` and `useDeleteChatAutomation`: removes the workflow(s) and their
/// chat thread.
pub fn delete(
    runtime: &RuntimeHandle,
    workflow_id: Option<Arc<str>>,
    chat_group_id: Option<Arc<str>>,
) -> Result<Reply<()>> {
    runtime.submit(move |services| async move {
        let mut group = chat_group_id.clone();
        mutate_workflows(&services.executor, |list| {
            if let Some(id) = &workflow_id
                && group.is_none()
                && let Some(item) = list.iter().find(|item| item["id"] == id.as_ref())
            {
                group = item["chatGroupId"].as_str().map(Arc::from);
            }
            list.retain(|item| {
                workflow_id.as_ref().is_none_or(|id| item["id"] != id.as_ref())
                    && chat_group_id
                        .as_ref()
                        .is_none_or(|group| item["chatGroupId"] != group.as_ref())
            });
            Ok(())
        })
        .await?;
        let Some(group) = group else { return Ok(()) };
        transaction(
            &services.executor,
            vec![
                statement(
                    format!("UPDATE chat_groups SET deleted_at={NOW},updated_at={NOW} WHERE id=? AND deleted_at IS NULL"),
                    vec![json!(group)],
                    None,
                ),
                statement(
                    format!("UPDATE chat_messages SET deleted_at={NOW},updated_at={NOW} WHERE chat_group_id=? AND deleted_at IS NULL"),
                    vec![json!(group)],
                    None,
                ),
            ],
        )
        .await
    })
}

/// The starter actions of `StarterAutomationDetails` and `useRemoveStarterDraft`.
#[derive(Clone, Debug)]
pub enum StarterCommand {
    SaveDraft,
    SetEnabled(bool),
    Remove,
    /// The Drive starter's one step changed.
    ConfigureDrive(Step),
    /// A legacy starter's target setting: a folder path, or a `{id, name}` reference.
    SetTarget(String),
}

pub fn starter_command(
    runtime: &RuntimeHandle,
    id: StarterId,
    command: StarterCommand,
) -> Result<Reply<()>> {
    runtime.submit(move |services| async move {
        let executor = &services.executor;
        let drive = |enabled: Option<bool>, step: Option<Step>| async move {
            mutate_workflows(executor, |list| {
                let index = list.iter().position(|item| item["id"] == DRIVE_STARTER_ID);
                let mut workflow = index
                    .and_then(|index| Workflow::parse(&list[index]))
                    .unwrap_or_else(Workflow::drive_starter);
                if let Some(enabled) = enabled {
                    workflow.enabled = enabled;
                }
                if let Some(step) = step.clone() {
                    workflow.steps = vec![step];
                }
                // Drive's own save keeps whatever the workflow is, ready or not.
                let mut value = workflow.merge_into(index.map(|index| &list[index]));
                value["enabled"] = json!(workflow.enabled);
                if let Some(index) = index {
                    list.remove(index);
                }
                list.insert(0, value);
                Ok(())
            })
            .await
        };
        let mut settings = Vec::new();
        match command {
            StarterCommand::SaveDraft => {
                if id == StarterId::GoogleDrive {
                    drive(None, None).await?;
                }
                settings.push((DRAFT_TEMPLATE, json!(id.key())));
            }
            StarterCommand::SetEnabled(enabled) => {
                settings.push((DRAFT_TEMPLATE, json!(id.key())));
                match id.keys() {
                    Some([key, ..]) => settings.push((key, json!(enabled))),
                    None => drive(Some(enabled), None).await?,
                }
            }
            StarterCommand::Remove => {
                match id.keys() {
                    Some([key, ..]) => settings.push((key, json!(false))),
                    None => {
                        mutate_workflows(executor, |list| {
                            for item in list
                                .iter_mut()
                                .filter(|item| item["id"] == DRIVE_STARTER_ID)
                            {
                                item["enabled"] = json!(false);
                            }
                            Ok(())
                        })
                        .await?
                    }
                }
                let rows = executor
                    .execute(
                        "SELECT value_json FROM app_settings WHERE id='automation_draft_template'"
                            .into(),
                        vec![],
                    )
                    .await
                    .map_err(failure)?;
                if rows.first().and_then(|row| row["value_json"].as_str())
                    == Some(json!(id.key()).to_string().as_str())
                {
                    settings.push((DRAFT_TEMPLATE, json!("")));
                }
            }
            StarterCommand::ConfigureDrive(step) => drive(None, Some(step)).await?,
            StarterCommand::SetTarget(target) => {
                let [_, key, _] = id
                    .keys()
                    .ok_or_else(|| failure("Google Drive has no target setting"))?;
                settings.push((key, json!(target)));
            }
        }
        set_settings(executor, settings).await
    })
}

/// The runner's view of a stored workflow: `isWorkflowStepReady` over raw JSON.
pub fn step_configured(step: &Value) -> bool {
    Step::parse(step).is_some_and(|step| step.ready())
}

/// `isWorkflowReady` over raw JSON.
pub fn workflow_configured(workflow: &Value) -> bool {
    Workflow::parse(workflow).is_some_and(|parsed| {
        // Shipping drops unknown steps; the runner must not run a workflow it cannot finish.
        parsed.steps.len() == workflow["steps"].as_array().map_or(0, Vec::len) && parsed.ready()
    })
}

pub fn already_processed(workflow: &Value, session_id: &str) -> bool {
    workflow["processedSessionIds"]
        .as_array()
        .is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(session_id)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn explicit_export_false_options_differ_from_legacy_defaults() {
        let mut step = json!({"id":"s","type":"markdown_export","directory":"/exports"});
        assert!(step_configured(&step));
        step["options"] = json!({"include_memo":false,"include_summary":false,"include_transcript":false,"include_action_items":false});
        assert!(!step_configured(&step));
        step["options"]["include_summary"] = json!(true);
        assert!(step_configured(&step));
        step["directory"] = json!(" ");
        assert!(!step_configured(&step));
    }

    #[test]
    fn every_step_must_be_configured_and_unknown_steps_are_not_executable() {
        let mut workflow = json!({"trigger":"meeting_completed","id":"w","steps":[
            {"id":"a","type":"slack_recap","target":{"id":"channel","name":"general"}},
            {"id":"b","type":"linear_issues","target":null}
        ],"processedSessionIds":["saved"]});
        assert!(!workflow_configured(&workflow));
        workflow["steps"][1]["target"] = json!({"id":"team","name":"Engineering"});
        assert!(workflow_configured(&workflow));
        assert!(already_processed(&workflow, "saved"));
        assert!(!already_processed(&workflow, "retryable"));
        workflow["steps"][1]["type"] = json!("unknown");
        assert!(!workflow_configured(&workflow));
    }

    #[test]
    fn drive_steps_are_ready_only_with_a_folder_and_after_the_summary() {
        let mut workflow = Workflow::drive_starter();
        assert!(!workflow.ready());
        workflow.steps[0] = Step::Drive {
            id: "d".into(),
            google_docs: true,
            connection_id: "conn".into(),
            target: Some(Target {
                id: "folder".into(),
                name: "Meetings".into(),
            }),
        };
        assert!(workflow.ready());
        workflow.trigger = Trigger::MeetingCompleted;
        assert!(!workflow.ready());
    }

    #[test]
    fn saving_keeps_run_history_and_never_enables_an_unready_workflow() {
        let stored = json!({"id":"w","title":"Old","enabled":true,"trigger":"note_enhanced",
            "steps":[],"lastRun":{"at":"2026-01-01T00:00:00Z","status":"running","detail":""},
            "processedSessionIds":["s1"],"driveExports":[],"chatGroupId":null,"custom":1});
        let mut workflow = Workflow::parse(&stored).unwrap();
        assert_eq!(
            workflow.last_run, None,
            "unfinished runs are hidden like shipping"
        );
        workflow.title = "New".into();
        workflow.enabled = true;
        let merged = workflow.merge_into(Some(&stored));
        assert_eq!(merged["title"], "New");
        assert_eq!(merged["enabled"], false);
        assert_eq!(merged["processedSessionIds"], json!(["s1"]));
        assert_eq!(merged["lastRun"]["status"], "running");
        assert_eq!(merged["custom"], 1);
        workflow.steps.push(Step::new(StepType::Markdown));
        if let Step::Markdown { directory, .. } = &mut workflow.steps[0] {
            *directory = "/exports".into();
        }
        assert_eq!(workflow.merge_into(Some(&stored))["enabled"], true);
    }

    #[test]
    fn settings_decode_shipping_string_settings_and_starter_state() {
        let workflows =
            json!([{"id":"w1","title":" ","steps":[{"id":"x","type":"zap"}]}]).to_string();
        let rows = [
            json!({"id": WORKFLOWS, "value_json": json!(workflows).to_string()}),
            json!({"id": DRAFT_TEMPLATE, "value_json": "\"markdown-export\""}),
            json!({"id": "automation_markdown_export_enabled", "value_json": "true"}),
            json!({"id": "automation_markdown_export_directory", "value_json": "\"/notes\""}),
            json!({"id": "automation_slack_recap_channel", "value_json": json!(r#"{"id":"C1","name":"eng"}"#).to_string()}),
            json!({"id": "automation_slack_recap_last_run", "value_json": json!(r#"{"at":"2026-01-01T00:00:00Z","status":"error","detail":"no summary"}"#).to_string()}),
        ];
        let settings = Settings::decode(&rows);
        assert_eq!(settings.workflows.len(), 1);
        assert_eq!(settings.workflows[0].display_title(), "Untitled automation");
        assert!(settings.workflows[0].steps.is_empty());
        assert_eq!(settings.draft_template(), Some(StarterId::MarkdownExport));
        assert!(settings.starter_enabled(StarterId::MarkdownExport));
        assert!(settings.starter_ready(StarterId::MarkdownExport));
        assert!(settings.starter_ready(StarterId::SlackRecap));
        assert!(!settings.starter_ready(StarterId::LinearActionItems));
        assert!(!settings.starter_ready(StarterId::GoogleDrive));
        let run = settings.starter_last_run(StarterId::SlackRecap).unwrap();
        assert!(!run.success);
    }
}
