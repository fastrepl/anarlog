use std::{
    collections::HashSet,
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

use anlg_agent_access::MeetingExport;
use desktop_runtime::{Reply, Result, RuntimeHandle, ServiceError, Services, SessionId};
use reqwest::{Client, Method, Url};
use serde_json::{Value, json};

use super::{
    automations::{already_processed, workflow_configured},
    mutations::{NOW, failure, statement, transaction},
    ports::decode_json_text,
};

fn active_runs() -> &'static Mutex<HashSet<String>> {
    static RUNS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    RUNS.get_or_init(Mutex::default)
}

struct RunGuard(String);

impl RunGuard {
    fn acquire(workflow: &str, session: &SessionId) -> Result<Self> {
        let key = format!("{workflow}:{}", session.0);
        if !active_runs()
            .lock()
            .map_err(|_| failure("Automation execution state is unavailable"))?
            .insert(key.clone())
        {
            return Err(failure("This automation is already running for the note"));
        }
        Ok(Self(key))
    }
}

impl Drop for RunGuard {
    fn drop(&mut self) {
        if let Ok(mut runs) = active_runs().lock() {
            runs.remove(&self.0);
        }
    }
}

#[derive(Clone, Copy)]
pub enum Trigger {
    NoteEnhanced,
    MeetingCompleted,
}

pub fn matching_workflows(
    runtime: &RuntimeHandle,
    trigger: Trigger,
    session: SessionId,
) -> Result<Reply<Vec<Arc<str>>>> {
    runtime.read(
        desktop_runtime::CancellationToken::new(),
        move |services| async move {
            let rows = services
                .executor
                .execute(
                    "SELECT value_json FROM app_settings WHERE id='automation_workflows'".into(),
                    vec![],
                )
                .await
                .map_err(failure)?;
            let Some(raw) = rows.first().and_then(|row| row["value_json"].as_str()) else {
                return Ok(Vec::new());
            };
            let workflows = decode_json_text(raw)?;
            let trigger = match trigger {
                Trigger::NoteEnhanced => "note_enhanced",
                Trigger::MeetingCompleted => "meeting_completed",
            };
            let workflows = workflows
                .as_array()
                .ok_or_else(|| failure("Invalid automation workflow storage"))?;
            if workflows.len() > 100 {
                return Err(failure("Automation dispatch exceeds 100 workflows"));
            }
            Ok(workflows
                .iter()
                .filter(|workflow| {
                    workflow["enabled"] == true
                        && workflow["trigger"] == trigger
                        && workflow_configured(workflow)
                        && !already_processed(workflow, &session.0)
                })
                .filter_map(|workflow| workflow["id"].as_str().map(Arc::from))
                .collect())
        },
    )
}

#[derive(Clone)]
pub struct AutomationClient {
    client: Client,
    base: Url,
    token: Arc<str>,
    sender: Arc<str>,
}

impl AutomationClient {
    pub fn new(base: &str, access_token: Arc<str>, sender: Arc<str>) -> Result<Self> {
        let base = Url::parse(base).map_err(failure)?;
        if base.scheme() != "https"
            && !matches!(base.host_str(), Some("localhost" | "127.0.0.1" | "::1"))
        {
            return Err(failure("Automation API requires HTTPS"));
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(failure)?;
        Ok(Self {
            client,
            base,
            token: access_token,
            sender,
        })
    }

    async fn request(&self, method: Method, path: &str, body: Option<Value>) -> Result<Value> {
        let url = self.base.join(path).map_err(failure)?;
        let mut request = self
            .client
            .request(method, url)
            .bearer_auth(self.token.as_ref());
        if let Some(body) = body {
            request = request.json(&body);
        }
        let mut response = request
            .send()
            .await
            .map_err(|_| failure("Automation API could not be reached"))?;
        if !response.status().is_success() {
            return Err(failure(format!(
                "Automation API returned HTTP {}",
                response.status().as_u16()
            )));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| failure("Automation response was interrupted"))?
        {
            if bytes.len() + chunk.len() > 1024 * 1024 {
                return Err(failure("Automation response exceeds 1 MiB"));
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.is_empty() {
            Ok(Value::Null)
        } else {
            serde_json::from_slice(&bytes).map_err(failure)
        }
    }

    async fn connection(&self, provider: &str) -> Result<String> {
        let connections = self
            .request(Method::GET, "/nango/connections", None)
            .await?;
        let connection = connections["connections"]
            .as_array()
            .and_then(|rows| rows.iter().find(|row| row["integration_id"] == provider))
            .ok_or_else(|| failure(format!("Connect {provider} before running this automation")))?;
        if connection["status"] == "reconnect_required" {
            return Err(failure(format!(
                "Reconnect {provider} before running this automation"
            )));
        }
        connection["connection_id"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| failure("Connection response has no ID"))
    }
}

pub fn run(
    runtime: &RuntimeHandle,
    workflow_id: Arc<str>,
    session: SessionId,
    client: Option<AutomationClient>,
) -> Result<Reply<String>> {
    runtime.submit(move |services| async move {
        let _guard=RunGuard::acquire(&workflow_id,&session)?;
        let rows = services
            .executor
            .execute(
                "SELECT value_json FROM app_settings WHERE id='automation_workflows'".into(),
                vec![],
            )
            .await
            .map_err(failure)?;
        let raw = rows
            .first()
            .and_then(|row| row["value_json"].as_str())
            .ok_or_else(|| failure("Automation was removed"))?;
        let workflows = decode_json_text(raw)?;
        let workflow = workflows
            .as_array()
            .and_then(|rows| rows.iter().find(|row| row["id"] == workflow_id.as_ref()))
            .ok_or_else(|| failure("Automation was removed"))?
            .clone();
        if workflow["enabled"] != true {
            return Err(failure("Enable this automation before running it"));
        }
        if !workflow_configured(&workflow) {
            return Err(failure("Configure each automation step first"));
        }
        if already_processed(&workflow, &session.0) {
            return Err(failure(
                "This automation already processed the note; automatic replay is blocked",
            ));
        }
        let export =
            anlg_agent_access::get_meeting_export(services.db.pool(), session.0.to_string())
                .await
                .map_err(failure)?;
        let steps = workflow["steps"]
            .as_array()
            .ok_or_else(|| failure("Invalid steps"))?;
        if steps.len() > 32 {
            return Err(failure("Automation exceeds 32 steps"));
        }
        let mut results = Vec::new();
        if client.is_none() && steps.iter().any(|step| step["type"] != "markdown_export") {
            record(&services, &workflow_id, &session, "error", "Sign in to run this automation", false).await?;
            return Err(failure("Sign in to run this automation"));
        }
        record(&services, &workflow_id, &session, "running", "Dispatch started; verify destinations before retrying an interrupted run", true).await?;
        for (index, step) in steps.iter().enumerate() {
            let result = execute_step(step, &export, client.as_ref()).await;
            match result {
                Ok(detail) => {
                    results.push(detail);
                }
                Err(error) => {
                    let detail = format!("Step {}: {error}. Completed: {}. Verify destinations before allowing a retry.", index + 1, results.join(" · "));
                    record(
                        &services,
                        &workflow_id,
                        &session,
                        "error",
                        &detail,
                        false,
                    )
                    .await?;
                    return Err(failure(detail));
                }
            }
        }
        record(&services, &workflow_id, &session, "success", &results.join(" · "), false).await?;
        Ok(results.join(" · "))
    })
}

async fn record(
    services: &Services,
    id: &str,
    session: &SessionId,
    status: &str,
    detail: &str,
    reserve: bool,
) -> Result<()> {
    let rows = services
        .executor
        .execute(
            "SELECT value_json FROM app_settings WHERE id='automation_workflows'".into(),
            vec![],
        )
        .await
        .map_err(failure)?;
    let raw = rows
        .first()
        .and_then(|row| row["value_json"].as_str())
        .ok_or(ServiceError::Conflict)?;
    let mut workflows = decode_json_text(raw)?;
    let workflow = workflows
        .as_array_mut()
        .and_then(|rows| rows.iter_mut().find(|row| row["id"] == id))
        .ok_or(ServiceError::Conflict)?;
    let date = services
        .executor
        .execute(format!("SELECT {NOW} AS value"), vec![])
        .await
        .map_err(failure)?;
    if reserve && already_processed(workflow, &session.0) {
        return Err(ServiceError::Conflict);
    }
    workflow["lastRun"] =
        json!({"at":date[0]["value"],"status":status,"detail":detail,"sessionId":session.0});
    if reserve {
        let mut ids = workflow["processedSessionIds"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        ids.retain(|id| id.as_str() != Some(&session.0));
        ids.push(json!(session.0));
        if ids.len() > 50 {
            ids.drain(..ids.len() - 50);
        }
        workflow["processedSessionIds"] = json!(ids);
    }
    let encoded = serde_json::to_string(&workflows).map_err(failure)?;
    let encoded = if serde_json::from_str::<Value>(raw).is_ok_and(|value| value.is_string()) {
        json!(encoded).to_string()
    } else {
        encoded
    };
    transaction(&services.executor,vec![statement(format!("UPDATE app_settings SET value_json=?,updated_at={NOW} WHERE id='automation_workflows' AND value_json=?"),vec![json!(encoded),json!(raw)],Some(1))]).await
}

pub fn allow_retry(
    runtime: &RuntimeHandle,
    workflow_id: Arc<str>,
    session: SessionId,
) -> Result<Reply<()>> {
    runtime.submit(move |services| async move {
        let _guard=RunGuard::acquire(&workflow_id,&session)?;
        let rows=services.executor.execute("SELECT value_json FROM app_settings WHERE id='automation_workflows'".into(),vec![]).await.map_err(failure)?;
        let raw=rows.first().and_then(|row| row["value_json"].as_str()).ok_or(ServiceError::Conflict)?;
        let mut workflows=decode_json_text(raw)?;
        let workflow=workflows.as_array_mut().and_then(|rows| rows.iter_mut().find(|row| row["id"]==workflow_id.as_ref())).ok_or(ServiceError::Conflict)?;
        if let Some(ids)=workflow["processedSessionIds"].as_array_mut() {ids.retain(|id| id.as_str()!=Some(&session.0));}
        let encoded=serde_json::to_string(&workflows).map_err(failure)?;
        let encoded=if serde_json::from_str::<Value>(raw).is_ok_and(|value| value.is_string()) {json!(encoded).to_string()} else {encoded};
        transaction(&services.executor,vec![statement(format!("UPDATE app_settings SET value_json=?,updated_at={NOW} WHERE id='automation_workflows' AND value_json=?"),vec![json!(encoded),json!(raw)],Some(1))]).await
    })
}

async fn execute_step(
    step: &Value,
    export: &MeetingExport,
    client: Option<&AutomationClient>,
) -> Result<String> {
    let kind = step["type"].as_str().unwrap_or("");
    if kind == "markdown_export" {
        return write_export(step, export);
    }
    let client = client.ok_or_else(|| failure("Sign in to run this automation"))?;
    let target = step["target"]["id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .ok_or_else(|| failure("Choose an automation target"))?;
    let summary = export
        .meeting
        .summaries
        .first()
        .filter(|summary| !summary.markdown.trim().is_empty());
    let date = export
        .meeting
        .started_at
        .get(..10)
        .filter(|date| !date.is_empty())
        .unwrap_or_else(|| export.meeting.created_at.get(..10).unwrap_or(""));
    match kind {
        "slack_recap" => {
            let summary = summary.ok_or_else(|| failure("No meeting summary is available yet"))?;
            let prefix = format!("*{}*\n\n", export.meeting.title);
            let suffix = format!("\n\n_Sent by {} via Anarlog_", client.sender);
            let budget = 40000usize.saturating_sub(prefix.chars().count() + suffix.chars().count());
            let body = summary.markdown.chars().take(budget).collect::<String>();
            client
                .request(
                    Method::POST,
                    "/messenger/slack/messages",
                    Some(json!({"channel":target,"text":format!("{prefix}{body}{suffix}")})),
                )
                .await?;
        }
        "notion_update" => {
            let summary = summary.ok_or_else(|| failure("No meeting summary is available yet"))?;
            let connection = client.connection("notion").await?;
            client.request(Method::POST,"/notion/append-update",Some(json!({"connection_id":connection,"page_id":target,"heading":format!("{date} — {}",export.meeting.title),"markdown":summary.markdown}))).await?;
        }
        "linear_issues" => {
            let items = export
                .meeting
                .action_items
                .iter()
                .filter(|item| {
                    item.completed_at.is_none()
                        && !matches!(item.status.as_str(), "done" | "completed")
                        && !item.text.trim().is_empty()
                })
                .take(10)
                .collect::<Vec<_>>();
            if items.is_empty() {
                return Ok("No action items found for this meeting".into());
            }
            let connection = client.connection("linear").await?;
            for item in &items {
                client.request(Method::POST,"/ticket/linear/create-issue",Some(json!({"connection_id":connection,"team_id":target,"title":item.text,"description":format!("Action item from the Anarlog meeting \"{}\" ({date}).",export.meeting.title)}))).await?;
            }
            return Ok(format!("{} issues created", items.len()));
        }
        _ => return Err(failure("Unknown automation step")),
    }
    Ok(format!(
        "Delivered to {}",
        step["target"]["name"].as_str().unwrap_or(target)
    ))
}

fn write_export(step: &Value, export: &MeetingExport) -> Result<String> {
    let directory = step["directory"]
        .as_str()
        .filter(|directory| !directory.trim().is_empty())
        .ok_or_else(|| failure("Choose an export directory"))?;
    let options = &step["options"];
    let include = |key| options[key].as_bool().unwrap_or(true);
    let mut filtered = export.clone();
    if !include("include_memo") {
        filtered.meeting.note = None;
    }
    if !include("include_summary") {
        filtered.meeting.summaries.clear();
    }
    if !include("include_transcript") {
        filtered.transcripts.clear();
    }
    if !include("include_action_items") {
        filtered.meeting.action_items.clear();
    }
    if filtered.meeting.note.is_none()
        && filtered.meeting.summaries.is_empty()
        && filtered.transcripts.is_empty()
        && filtered.meeting.action_items.is_empty()
    {
        return Err(failure("No selected content to export"));
    }
    let date = export
        .meeting
        .started_at
        .get(..10)
        .filter(|date| !date.is_empty())
        .unwrap_or_else(|| export.meeting.created_at.get(..10).unwrap_or(""));
    let title = if export.meeting.title.trim().is_empty() {
        "Untitled meeting"
    } else {
        export.meeting.title.trim()
    };
    let custom = options["filename"].as_str().unwrap_or("").trim();
    let stem = if custom.is_empty() {
        format!("{date} {title}")
    } else {
        custom
            .split("{title}")
            .map(|part| part.replace("{date}", date))
            .collect::<Vec<_>>()
            .join(title)
    };
    let stem = if stem.to_lowercase().ends_with(".md") {
        &stem[..stem.len() - 3]
    } else {
        &stem
    };
    let mut safe = stem
        .chars()
        .map(|ch| {
            if ch.is_control() || "<>:\"/\\|?*".contains(ch) {
                '_'
            } else {
                ch
            }
        })
        .collect::<String>();
    while safe.len() > 180 {
        safe.pop();
    }
    safe = safe.trim_matches([' ', '.']).to_owned();
    if safe.is_empty() {
        safe = "Untitled meeting".into();
    }
    let reserved = safe.split('.').next().unwrap_or("").to_ascii_uppercase();
    if matches!(reserved.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (reserved.len() == 4
            && (reserved.starts_with("COM") || reserved.starts_with("LPT"))
            && matches!(reserved.as_bytes()[3], b'1'..=b'9'))
    {
        safe.insert(0, '_');
    }
    if include("include_id_suffix") {
        safe.push_str(&format!(
            " [{}]",
            export.meeting.id.chars().take(8).collect::<String>()
        ));
    }
    let directory = PathBuf::from(directory);
    std::fs::create_dir_all(&directory).map_err(failure)?;
    let path = directory.join(format!("{safe}.md"));
    let exists = match std::fs::read_to_string(&path) {
        Ok(content) => {
            let marker = format!("- ID: `{}`", export.meeting.id);
            let mut lines = content.lines();
            if lines.find(|line| line.starts_with("- ID: `")) != Some(marker.as_str())
                || !lines
                    .next()
                    .is_some_and(|line| line.starts_with("- Date: "))
            {
                return Err(failure("Export filename belongs to another file"));
            }
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(failure(error)),
    };
    let mut temporary = tempfile::Builder::new()
        .prefix(".anlg-export-")
        .tempfile_in(&directory)
        .map_err(failure)?;
    temporary
        .write_all(format!("{}\n", filtered.to_markdown()).as_bytes())
        .map_err(failure)?;
    temporary.as_file().sync_all().map_err(failure)?;
    if exists {
        temporary
            .as_file()
            .set_permissions(std::fs::metadata(&path).map_err(failure)?.permissions())
            .map_err(failure)?;
        temporary.persist(&path).map_err(failure)?;
    } else {
        temporary.persist_noclobber(&path).map_err(failure)?;
    }
    Ok(path.to_string_lossy().into_owned())
}
