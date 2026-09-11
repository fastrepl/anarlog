//! `automations/engine.ts` + the `local-api` plugin's `dispatch_event`: what
//! runs when a meeting completes and when its note is enhanced — the
//! outbound webhooks, the starter automations (`automation_*` settings) and
//! the custom workflows (`automation_workflows`), each recording its
//! `lastRun` and processed sessions the way the frontend does. The shell
//! is signed out, so the Slack / Linear / Notion steps end where
//! `requireSupabaseSession` ends: `sign in to run this automation`.

use anlg_local_api_core::{dispatch, export};
use sqlx::SqlitePool;

use crate::automations::{
    RunRecord, RunStatus, StepType, TargetRef, Trigger, Workflow, parse_target_ref,
    parse_workflows, serialize_workflows,
};

/// `MAX_PROCESSED_SESSIONS`
const MAX_PROCESSED_SESSIONS: usize = 50;
const SIGN_IN_ERROR: &str = "sign in to run this automation";
const NO_SUMMARY_ERROR: &str = "no meeting summary is available yet";

/// `dispatchMeetingCompleted`: the `meeting.completed` webhooks, then
/// `runMeetingCompletedAutomations` (the markdown-export starter and the
/// `meeting_completed` workflows).
pub async fn meeting_completed(pool: &SqlitePool, session_id: &str) {
    dispatch_event(pool, dispatch::EVENT_MEETING_COMPLETED, session_id).await;
    export::run_markdown_export_automation(pool, session_id).await;
    run_custom_workflows(pool, session_id, Trigger::MeetingCompleted).await;
}

/// `dispatchEvent("note.enhanced")` (the plugin re-exports the markdown
/// with the summary, then fans out) and `runNoteEnhancedAutomations`.
pub async fn note_enhanced(pool: &SqlitePool, session_id: &str) {
    export::run_markdown_export_automation(pool, session_id).await;
    dispatch_event(pool, dispatch::EVENT_NOTE_ENHANCED, session_id).await;
    run_slack_recap(pool, session_id).await;
    run_linear_issues(pool, session_id).await;
    run_notion_update(pool, session_id).await;
    run_custom_workflows(pool, session_id, Trigger::NoteEnhanced).await;
}

async fn dispatch_event(pool: &SqlitePool, event: &str, session_id: &str) {
    match dispatch::dispatch_event(pool, event, session_id).await {
        Ok(targeted) => {
            if targeted > 0 {
                tracing::info!(
                    event,
                    session_id,
                    targeted,
                    "[automations] webhooks dispatched"
                );
            }
        }
        Err(error) => tracing::warn!(%error, event, "[automations] webhook dispatch failed"),
    }
}

/// `runCustomWorkflows`
async fn run_custom_workflows(pool: &SqlitePool, session_id: &str, trigger: Trigger) {
    let workflows = load_workflows(pool).await;
    for workflow in workflows {
        if !workflow.enabled || workflow.trigger != trigger || workflow.steps.is_empty() {
            continue;
        }
        if workflow
            .processed_session_ids
            .iter()
            .any(|id| id == session_id)
        {
            continue;
        }
        run_workflow(pool, session_id, &workflow).await;
    }
}

/// `runWorkflow`: every step in order, the session marked processed after
/// each, the run recorded as `ok` / the joined details, or the first error.
async fn run_workflow(pool: &SqlitePool, session_id: &str, workflow: &Workflow) {
    let mut record = RunRecord {
        at: now_iso(),
        status: RunStatus::Success,
        detail: String::new(),
    };
    let mut marked = false;
    let mut details = Vec::new();
    let mut failure = None;
    for step in &workflow.steps {
        match execute_step(pool, session_id, step).await {
            Ok(detail) => {
                details.push(detail);
                marked = true;
                persist_workflow_result(pool, &workflow.id, None, Some(session_id)).await;
            }
            Err(error) => {
                failure = Some(error);
                break;
            }
        }
    }
    match failure {
        None => {
            let joined = details
                .into_iter()
                .filter(|detail| !detail.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            record.detail = if joined.is_empty() {
                "ok".to_string()
            } else {
                joined
            };
            persist_workflow_result(pool, &workflow.id, Some(record), Some(session_id)).await;
        }
        Some(error) => {
            tracing::error!(%error, workflow = %workflow.id, "[automations] workflow failed");
            record.status = RunStatus::Error;
            record.detail = error;
            persist_workflow_result(
                pool,
                &workflow.id,
                Some(record),
                marked.then_some(session_id),
            )
            .await;
        }
    }
}

/// `persistWorkflowResult`
async fn persist_workflow_result(
    pool: &SqlitePool,
    workflow_id: &str,
    record: Option<RunRecord>,
    session_id: Option<&str>,
) {
    let workflows: Vec<Workflow> = load_workflows(pool)
        .await
        .into_iter()
        .map(|mut workflow| {
            if workflow.id == workflow_id {
                if let Some(record) = &record {
                    workflow.last_run = Some(record.clone());
                }
                if let Some(session_id) = session_id {
                    workflow.processed_session_ids =
                        append_processed(workflow.processed_session_ids, session_id);
                }
            }
            workflow
        })
        .collect();
    set_setting(
        pool,
        "automation_workflows",
        serde_json::Value::String(serialize_workflows(&workflows)),
    )
    .await;
}

/// `appendProcessedSession`
fn append_processed(mut processed: Vec<String>, session_id: &str) -> Vec<String> {
    if processed.iter().any(|id| id == session_id) {
        return processed;
    }
    processed.push(session_id.to_string());
    let excess = processed.len().saturating_sub(MAX_PROCESSED_SESSIONS);
    processed.drain(..excess);
    processed
}

/// `executeWorkflowStep`
async fn execute_step(
    pool: &SqlitePool,
    session_id: &str,
    step: &crate::automations::Step,
) -> Result<String, String> {
    if step.kind == StepType::MarkdownExport {
        let directory = step.directory.trim();
        if directory.is_empty() {
            return Err("choose an export folder first".to_string());
        }
        return export::export_meeting_markdown(pool, session_id.to_string(), directory).await;
    }
    let Some(target) = &step.target else {
        return Err(format!("choose a {} first", step_label(step.kind)));
    };
    match step.kind {
        StepType::SlackRecap => execute_slack_recap(pool, session_id, target).await,
        StepType::LinearIssues => execute_linear_issues(pool, session_id, target).await,
        StepType::NotionUpdate => execute_notion_update(pool, session_id, target).await,
        StepType::MarkdownExport => unreachable!(),
    }
}

/// `stepLabel`
fn step_label(kind: StepType) -> &'static str {
    match kind {
        StepType::SlackRecap => "Slack channel",
        StepType::LinearIssues => "Linear team",
        StepType::NotionUpdate => "Notion page",
        StepType::MarkdownExport => "export folder",
    }
}

/// `executeSlackRecap`: the recap is loaded first, then the signed-in
/// session the shell does not have.
async fn execute_slack_recap(
    pool: &SqlitePool,
    session_id: &str,
    _channel: &TargetRef,
) -> Result<String, String> {
    if load_meeting_recap(pool, session_id).await.is_none() {
        return Err(NO_SUMMARY_ERROR.to_string());
    }
    Err(SIGN_IN_ERROR.to_string())
}

/// `executeLinearIssues`: a meeting without action items is a success with
/// nothing to create.
async fn execute_linear_issues(
    pool: &SqlitePool,
    session_id: &str,
    _team: &TargetRef,
) -> Result<String, String> {
    if load_meeting_action_items(pool, session_id).await.is_empty() {
        return Ok("no action items found for this meeting".to_string());
    }
    Err(SIGN_IN_ERROR.to_string())
}

/// `executeNotionUpdate`
async fn execute_notion_update(
    pool: &SqlitePool,
    session_id: &str,
    _page: &TargetRef,
) -> Result<String, String> {
    if load_meeting_recap(pool, session_id).await.is_none() {
        return Err(NO_SUMMARY_ERROR.to_string());
    }
    Err(SIGN_IN_ERROR.to_string())
}

/// `runSlackRecap` / `runLinearIssues` / `runNotionUpdate`: the starter
/// automations behind their `automation_*` settings.
async fn run_slack_recap(pool: &SqlitePool, session_id: &str) {
    run_starter(pool, session_id, StepType::SlackRecap).await;
}

async fn run_linear_issues(pool: &SqlitePool, session_id: &str) {
    run_starter(pool, session_id, StepType::LinearIssues).await;
}

async fn run_notion_update(pool: &SqlitePool, session_id: &str) {
    run_starter(pool, session_id, StepType::NotionUpdate).await;
}

async fn run_starter(pool: &SqlitePool, session_id: &str, kind: StepType) {
    let (name, target_key) = match kind {
        StepType::SlackRecap => ("slack_recap", "automation_slack_recap_channel"),
        StepType::LinearIssues => ("linear_issues", "automation_linear_issues_team"),
        StepType::NotionUpdate => ("notion_update", "automation_notion_update_page"),
        StepType::MarkdownExport => return,
    };
    let enabled = export::load_setting(pool, &format!("automation_{name}_enabled"))
        .await
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    if !enabled {
        return;
    }
    let Some(target) = string_setting(pool, target_key)
        .await
        .and_then(|raw| parse_target_ref(&raw))
    else {
        return;
    };
    let processed_key = format!("automation_{name}_processed");
    let processed = string_setting(pool, &processed_key)
        .await
        .map(|raw| parse_processed(&raw))
        .unwrap_or_default();
    if processed.iter().any(|id| id == session_id) {
        return;
    }
    let mut record = RunRecord {
        at: now_iso(),
        status: RunStatus::Success,
        detail: String::new(),
    };
    let result = match kind {
        StepType::SlackRecap => execute_slack_recap(pool, session_id, &target).await,
        StepType::LinearIssues => execute_linear_issues(pool, session_id, &target).await,
        StepType::NotionUpdate => execute_notion_update(pool, session_id, &target).await,
        StepType::MarkdownExport => return,
    };
    match result {
        Ok(detail) => {
            record.detail = detail;
            let next = append_processed(processed, session_id);
            set_setting(
                pool,
                &processed_key,
                serde_json::Value::String(serde_json::to_string(&next).unwrap_or_default()),
            )
            .await;
        }
        Err(error) => {
            tracing::error!(%error, name, "[automations] starter failed");
            record.status = RunStatus::Error;
            record.detail = error;
        }
    }
    set_setting(
        pool,
        &format!("automation_{name}_last_run"),
        serde_json::Value::String(record_json(&record)),
    )
    .await;
}

/// `JSON.stringify(record)`
fn record_json(record: &RunRecord) -> String {
    serde_json::json!({
        "at": record.at,
        "status": match record.status {
            RunStatus::Success => "success",
            RunStatus::Error => "error",
        },
        "detail": record.detail,
    })
    .to_string()
}

/// `parseProcessedSessions`
fn parse_processed(value: &str) -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(value)
        .ok()
        .and_then(|parsed| parsed.as_array().cloned())
        .map(|items| {
            items
                .into_iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// `loadMeetingRecap`: the session title, its date and the first summary's
/// markdown.
async fn load_meeting_recap(
    pool: &SqlitePool,
    session_id: &str,
) -> Option<(String, String, String)> {
    let row = sqlx::query_as::<
        _,
        (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    >(
        "SELECT
           s.title AS session_title,
           COALESCE(NULLIF(s.started_at, ''), s.created_at) AS occurred_at,
           d.body,
           d.body_format
         FROM sessions s
         LEFT JOIN session_documents d
           ON d.session_id = s.id
           AND d.kind IN ('summary', 'template_output')
           AND d.deleted_at IS NULL
         WHERE s.id = ?
         ORDER BY d.sort_order, d.id
         LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()?;
    let (title, occurred_at, body, format) = row;
    let body = body.filter(|body| !body.is_empty())?;
    let body = if format.as_deref() == Some("prosemirror_json") {
        crate::db::enhancer::body_to_markdown(&body, "prosemirror_json")
            .trim()
            .to_string()
    } else {
        body.trim().to_string()
    };
    if body.is_empty() {
        return None;
    }
    let title = title
        .map(|title| title.trim().to_string())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| "Untitled meeting".to_string());
    let date = occurred_at.unwrap_or_default().chars().take(10).collect();
    Some((title, date, body))
}

/// `loadMeetingActionItems`: the open `action_items` rows, else the unchecked
/// task items of the first summary document.
async fn load_meeting_action_items(pool: &SqlitePool, session_id: &str) -> Vec<String> {
    let rows: Vec<(Option<String>,)> = sqlx::query_as(
        "SELECT text
         FROM action_items
         WHERE session_id = ?
           AND deleted_at IS NULL
           AND completed_at IS NULL
           AND status NOT IN ('done', 'completed')
         ORDER BY source_order, id",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let from_db: Vec<String> = rows
        .into_iter()
        .filter_map(|(text,)| text)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .collect();
    if !from_db.is_empty() {
        return from_db;
    }
    let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT body, body_format
         FROM session_documents
         WHERE session_id = ?
           AND kind IN ('summary', 'template_output')
           AND deleted_at IS NULL
         ORDER BY sort_order, id
         LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    let Some((Some(body), Some(format))) = row else {
        return Vec::new();
    };
    if format != "prosemirror_json" {
        return Vec::new();
    }
    serde_json::from_str::<serde_json::Value>(&body)
        .map(|doc| unchecked_task_items(&doc))
        .unwrap_or_default()
}

/// `collectUncheckedTaskItems`
pub fn unchecked_task_items(node: &serde_json::Value) -> Vec<String> {
    let mut items = Vec::new();
    if node.get("type").and_then(|t| t.as_str()) == Some("taskItem") {
        let checked = node
            .get("attrs")
            .and_then(|attrs| attrs.get("checked"))
            .and_then(|checked| checked.as_bool())
            == Some(true);
        if !checked {
            let text = node_text(node).trim().to_string();
            if !text.is_empty() {
                items.push(text);
            }
        }
        return items;
    }
    for child in node
        .get("content")
        .and_then(|content| content.as_array())
        .into_iter()
        .flatten()
    {
        items.extend(unchecked_task_items(child));
    }
    items
}

fn node_text(node: &serde_json::Value) -> String {
    if let Some(text) = node.get("text").and_then(|text| text.as_str()) {
        return text.to_string();
    }
    node.get("content")
        .and_then(|content| content.as_array())
        .into_iter()
        .flatten()
        .map(node_text)
        .collect()
}

async fn load_workflows(pool: &SqlitePool) -> Vec<Workflow> {
    string_setting(pool, "automation_workflows")
        .await
        .map(|raw| parse_workflows(&raw))
        .unwrap_or_default()
}

/// A setting stored as a JSON string.
async fn string_setting(pool: &SqlitePool, key: &str) -> Option<String> {
    export::load_setting(pool, key)
        .await
        .and_then(|value| value.as_str().map(str::to_string))
}

/// `setSettingValue` for the device-local automation keys.
async fn set_setting(pool: &SqlitePool, key: &str, value: serde_json::Value) {
    let now = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
    if let Err(error) = sqlx::query(
        "INSERT INTO app_settings (id, value_json, updated_at)
         VALUES (?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           value_json = excluded.value_json,
           updated_at = excluded.updated_at",
    )
    .bind(key)
    .bind(value.to_string())
    .bind(now)
    .execute(pool)
    .await
    {
        tracing::warn!(%error, key, "[automations] could not save the setting");
    }
}

/// `new Date().toISOString()`
fn now_iso() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn processed_sessions_cap_at_fifty_without_duplicates() {
        let processed: Vec<String> = (0..50).map(|i| format!("s{i}")).collect();
        let next = append_processed(processed.clone(), "s0");
        assert_eq!(next, processed);
        let next = append_processed(processed, "s50");
        assert_eq!(next.len(), 50);
        assert_eq!(next.first().map(String::as_str), Some("s1"));
        assert_eq!(next.last().map(String::as_str), Some("s50"));
        assert_eq!(parse_processed(r#"["a", 1, "b"]"#), ["a", "b"]);
        assert!(parse_processed("nope").is_empty());
    }

    #[test]
    fn unchecked_task_items_follow_the_summary_document() {
        let doc = serde_json::json!({
            "type": "doc",
            "content": [
                { "type": "taskList", "content": [
                    { "type": "taskItem", "attrs": { "checked": false }, "content": [
                        { "type": "paragraph", "content": [{ "type": "text", "text": "Ship " }, { "type": "text", "text": "it" }] }
                    ] },
                    { "type": "taskItem", "attrs": { "checked": true }, "content": [
                        { "type": "paragraph", "content": [{ "type": "text", "text": "Done" }] }
                    ] },
                    { "type": "taskItem", "content": [
                        { "type": "paragraph", "content": [{ "type": "text", "text": "   " }] }
                    ] }
                ] }
            ]
        });
        assert_eq!(unchecked_task_items(&doc), ["Ship it"]);
        assert_eq!(step_label(StepType::LinearIssues), "Linear team");
        let record = RunRecord {
            at: "2026-09-07T22:00:00.000Z".into(),
            status: RunStatus::Error,
            detail: "sign in to run this automation".into(),
        };
        assert_eq!(
            record_json(&record),
            r#"{"at":"2026-09-07T22:00:00.000Z","status":"error","detail":"sign in to run this automation"}"#
        );
    }
}
