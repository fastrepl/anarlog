use std::{io::Read, path::PathBuf, sync::Arc};

use anlg_db_execute::TransactionStatement;
use desktop_runtime::{
    CancellationToken, Result, RuntimeHandle, ServiceError, Services, SessionId,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use super::failure;

const WELCOME_ID: &str = "anarlog-onboarding-demo-v1";
const WELCOME: &str = "Welcome to Anarlog 👋\n\nThis note is a quick way to see how Anarlog works.\n\nClick **Join & record** in the top-right corner. It will open a private, prerecorded demo meeting, so you don't have to worry about your camera or microphone. Anarlog will save the audio. To create a transcript and notes, choose a provider in **Settings → Transcription**; if one is not ready, Anarlog will show you a setup shortcut.\n\nWhen the video ends, Anarlog will stop listening. If transcription and intelligence are configured, it will start creating your summary automatically.";

pub async fn complete_onboarding(runtime: &RuntimeHandle) -> Result<SessionId> {
    runtime.submit(|services| async move {
        let existing = services.executor.execute(
            "SELECT id FROM sessions WHERE deleted_at IS NULL AND CASE WHEN json_valid(event_json) THEN json_extract(event_json, '$.tracking_id') END = ? ORDER BY created_at, id LIMIT 1".into(),
            vec![json!(WELCOME_ID)],
        ).await.map_err(failure)?;
        let id = existing.first().and_then(|row| row["id"].as_str())
            .map(str::to_owned).unwrap_or_else(|| Uuid::new_v4().to_string());
        let mut statements = Vec::new();
        if existing.is_empty() {
            let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            statements.push(statement(
                "INSERT INTO sessions (id,title,kind,event_json) VALUES (?,'Welcome to Anarlog','meeting',?)",
                vec![json!(id), json!(json!({
                    "tracking_id": WELCOME_ID, "calendar_id": "", "title": "Welcome to Anarlog",
                    "started_at": now, "ended_at": "", "is_all_day": false,
                    "has_recurrence_rules": false, "meeting_link": "https://anarlog.so/onboarding-demo/",
                    "description": "A private, prerecorded introduction to Anarlog."
                }).to_string())],
            ));
            statements.push(statement(
                "INSERT INTO session_documents (id,session_id,body) VALUES (?,?,?)",
                vec![json!(id), json!(id), json!(anlg_tiptap::md_to_tiptap_json(WELCOME).map_err(failure)?.to_string())],
            ));
        }
        statements.push(setting(crate::product::onboarding::NEEDED_KEY, Value::Bool(false)));
        services.executor.execute_transaction(statements).await.map_err(failure)?;
        Ok(id.into())
    })?.receive().await
}

pub fn statement(sql: &str, params: Vec<Value>) -> TransactionStatement {
    TransactionStatement {
        sql: sql.into(),
        params,
        expected_rows_affected: Some(1),
    }
}

pub fn setting(key: &str, value: Value) -> TransactionStatement {
    statement(
        "INSERT INTO app_settings (id,value_json) VALUES (?,?) ON CONFLICT(id) DO UPDATE SET value_json=excluded.value_json,updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')",
        vec![json!(key), json!(value.to_string())],
    )
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalMeeting {
    pub version: u32,
    pub session: Value,
    pub documents: Vec<Value>,
    pub transcripts: Vec<Value>,
    #[serde(default)]
    pub participants: Vec<Value>,
    #[serde(default)]
    pub humans: Vec<Value>,
}

impl CanonicalMeeting {
    pub fn title(&self) -> &str {
        self.session["title"].as_str().unwrap_or("Untitled")
    }
}

pub async fn snapshot(runtime: &RuntimeHandle, id: SessionId) -> Result<CanonicalMeeting> {
    runtime.submit(move |services| async move {
        let session = services.executor.execute(
            "SELECT * FROM sessions WHERE id=? AND deleted_at IS NULL".into(), vec![json!(id)]
        ).await.map_err(failure)?.into_iter().next().ok_or(ServiceError::Conflict)?;
        let documents = services.executor.execute(
            "SELECT * FROM session_documents WHERE session_id=? AND deleted_at IS NULL ORDER BY sort_order,id".into(), vec![json!(id)]
        ).await.map_err(failure)?;
        let transcripts = services.executor.execute(
            "SELECT * FROM transcripts WHERE session_id=? AND deleted_at IS NULL ORDER BY started_at_ms,id".into(), vec![json!(id)]
        ).await.map_err(failure)?;
        let participants = services.executor.execute("SELECT * FROM session_participants WHERE session_id=? AND deleted_at IS NULL ORDER BY created_at,id".into(),vec![json!(id)]).await.map_err(failure)?;
        let humans = services.executor.execute("SELECT * FROM humans WHERE id IN (SELECT human_id FROM session_participants WHERE session_id=?) ORDER BY id".into(),vec![json!(id)]).await.map_err(failure)?;
        Ok(CanonicalMeeting { version: 1, session, documents, transcripts, participants, humans })
    })?.receive().await
}

pub async fn import_files(
    runtime: &RuntimeHandle,
    paths: Vec<PathBuf>,
    cancel: CancellationToken,
) -> Result<Vec<SessionId>> {
    if paths.is_empty() || paths.len() > 1000 {
        return Err(failure("Select between 1 and 1,000 files"));
    }
    let worker_cancel = cancel.clone();
    let meetings = runtime
        .service(move |_| async move {
            tokio::task::spawn_blocking(move || {
                let cancel = worker_cancel;
                let mut bytes = 0;
                let mut meetings = Vec::new();
                for path in paths {
                    if cancel.is_cancelled() {
                        return Err(ServiceError::Cancelled);
                    }
                    let metadata = std::fs::metadata(&path).map_err(failure)?;
                    bytes += metadata.len();
                    if !metadata.is_file()
                        || metadata.len() > 20 * 1024 * 1024
                        || bytes > 100 * 1024 * 1024
                    {
                        return Err(failure("Import exceeds 20 MB per file or 100 MB total"));
                    }
                    let mut content = String::new();
                    std::fs::File::open(&path)
                        .map_err(failure)?
                        .take(20 * 1024 * 1024 + 1)
                        .read_to_string(&mut content)
                        .map_err(failure)?;
                    if content.len() > 20 * 1024 * 1024 {
                        return Err(failure("File grew beyond import limit"));
                    }
                    let extension = path
                        .extension()
                        .and_then(|v| v.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    let title = path
                        .file_stem()
                        .and_then(|v| v.to_str())
                        .unwrap_or("Imported meeting");
                    meetings.extend(parse_cancellable(&extension, title, &content, &cancel)?);
                }
                if cancel.is_cancelled() {
                    return Err(ServiceError::Cancelled);
                }
                Ok(meetings)
            })
            .await
            .map_err(failure)?
        })?
        .receive()
        .await?;
    import_meetings(runtime, meetings, cancel).await
}

pub async fn import_meetings(
    runtime: &RuntimeHandle,
    meetings: Vec<CanonicalMeeting>,
    cancel: CancellationToken,
) -> Result<Vec<SessionId>> {
    if meetings.is_empty() || meetings.len() > 20_000 {
        return Err(failure("Import needs between 1 and 20,000 meetings"));
    }
    runtime
        .submit(move |services| async move {
            let library = Library::current(&services).await?;
            let mut statements = Vec::new();
            let mut ids = Vec::new();
            let mut humans = std::collections::BTreeMap::new();
            for meeting in meetings {
                if cancel.is_cancelled() {
                    return Err(ServiceError::Cancelled);
                }
                if meeting.version != 1 {
                    return Err(failure("Unknown canonical export version"));
                }
                let id = meeting.session["id"]
                    .as_str()
                    .filter(|id| !id.is_empty())
                    .ok_or_else(|| failure("Missing session id"))?
                    .to_owned();
                statements.push(
                    insert_row(&services, "sessions", meeting.session, Some(&library)).await?,
                );
                for doc in meeting.documents {
                    if doc["session_id"].as_str() != Some(&id) {
                        return Err(failure("Mismatched document session"));
                    }
                    if doc["body_format"].as_str() == Some("prosemirror_json") {
                        let value: Value = serde_json::from_str(
                            doc["body"]
                                .as_str()
                                .ok_or_else(|| failure("Invalid document body"))?,
                        )
                        .map_err(failure)?;
                        if value["type"] != "doc" {
                            return Err(failure("Invalid document root"));
                        }
                    }
                    statements.push(
                        insert_row(&services, "session_documents", doc, Some(&library)).await?,
                    );
                }
                for transcript in meeting.transcripts {
                    if transcript["session_id"].as_str() != Some(&id) {
                        return Err(failure("Mismatched transcript session"));
                    }
                    statements.push(
                        insert_row(&services, "transcripts", transcript, Some(&library)).await?,
                    );
                }
                for participant in meeting.participants {
                    if participant["session_id"].as_str() != Some(&id) {
                        return Err(failure("Mismatched participant session"));
                    }
                    statements.push(
                        insert_row(
                            &services,
                            "session_participants",
                            participant,
                            Some(&library),
                        )
                        .await?,
                    );
                }
                for human in meeting.humans {
                    let id = human["id"]
                        .as_str()
                        .ok_or_else(|| failure("Human lacks id"))?
                        .to_owned();
                    if let Some(previous) = humans.insert(id.clone(), human.clone()) {
                        if previous != human {
                            return Err(ServiceError::Conflict);
                        }
                        continue;
                    }
                    let existing = services
                        .executor
                        .execute("SELECT * FROM humans WHERE id=?".into(), vec![json!(id)])
                        .await
                        .map_err(failure)?;
                    if let Some(existing) = existing.first() {
                        if existing != &human {
                            return Err(ServiceError::Conflict);
                        }
                    } else {
                        statements.push(insert_row(&services, "humans", human, None).await?);
                    }
                }
                ids.push(SessionId(id.into()));
            }
            if statements.len() > 20_000 {
                return Err(failure("Import exceeds transaction limit"));
            }
            if cancel.is_cancelled() {
                return Err(ServiceError::Cancelled);
            }
            services
                .executor
                .execute_transaction(statements)
                .await
                .map_err(failure)?;
            Ok(ids)
        })?
        .receive()
        .await
}

/// The library imported rows join, chosen like shipping's `create_session`: the device's
/// workspace, owned by the active library connection.
struct Library {
    workspace: Value,
    owner: Value,
}

impl Library {
    async fn current(services: &Services) -> Result<Self> {
        let rows = services.executor.execute(
            "SELECT COALESCE((SELECT json_extract(value_json, '$.workspace_id') FROM app_settings WHERE id = 'cloudsync_workspace_binding'), '') AS workspace, COALESCE((SELECT library_workspace_id FROM local_library_connections WHERE active = 1), NULLIF((SELECT json_extract(value_json, '$.workspace_id') FROM app_settings WHERE id = 'cloudsync_workspace_binding'), ''), '') AS owner".into(),
            vec![],
        ).await.map_err(failure)?;
        let row = rows
            .first()
            .ok_or_else(|| failure("Library lookup returned no row"))?;
        Ok(Self {
            workspace: row["workspace"].clone(),
            owner: row["owner"].clone(),
        })
    }
}

async fn insert_row(
    services: &Services,
    table: &str,
    mut row: Value,
    library: Option<&Library>,
) -> Result<TransactionStatement> {
    let columns = services
        .executor
        .execute(format!("PRAGMA table_info(\"{table}\")"), vec![])
        .await
        .map_err(failure)?;
    let row = row
        .as_object_mut()
        .ok_or_else(|| failure("Expected canonical row object"))?;
    if let Some(library) = library {
        let has = |name: &str| columns.iter().any(|column| column["name"] == name);
        // Rows from another library or a generated import must publish with this one.
        for (column, value) in [
            ("workspace_id", &library.workspace),
            ("owner_user_id", &library.owner),
        ] {
            if has(column) {
                row.insert(column.into(), value.clone());
            }
        }
        for column in ["created_by", "updated_by"] {
            if has(column)
                && row
                    .get(column)
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty)
            {
                row.insert(column.into(), library.owner.clone());
            }
        }
    }
    if row.is_empty()
        || row.keys().any(|key| {
            !columns
                .iter()
                .any(|column| column["name"].as_str() == Some(key))
        })
    {
        return Err(failure(
            "Export contains unknown database columns; update the app before importing",
        ));
    }
    let id = row
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| failure("Missing canonical row id"))?;
    if !services
        .executor
        .execute(
            format!("SELECT id FROM {table} WHERE id=?"),
            vec![json!(id)],
        )
        .await
        .map_err(failure)?
        .is_empty()
    {
        return Err(ServiceError::Conflict);
    }
    let names = row
        .keys()
        .map(|key| format!("\"{}\"", key.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(",");
    let placeholders = vec!["?"; row.len()].join(",");
    Ok(statement(
        &format!("INSERT INTO {table} ({names}) VALUES ({placeholders})"),
        row.values().cloned().collect(),
    ))
}

pub fn parse(extension: &str, title: &str, content: &str) -> Result<Vec<CanonicalMeeting>> {
    parse_cancellable(extension, title, content, &CancellationToken::new())
}

fn parse_cancellable(
    extension: &str,
    title: &str,
    content: &str,
    cancel: &CancellationToken,
) -> Result<Vec<CanonicalMeeting>> {
    if cancel.is_cancelled() {
        return Err(ServiceError::Cancelled);
    }
    if extension == "json" {
        let value: Value = serde_json::from_str(content).map_err(failure)?;
        if value.get("version").is_some() && value.get("session").is_some() {
            return Ok(vec![serde_json::from_value(value).map_err(failure)?]);
        }
        let values = if let Value::Array(items) = value {
            items
        } else {
            [
                "meetings",
                "conversations",
                "documents",
                "notes",
                "recordings",
                "results",
                "items",
                "data",
            ]
            .iter()
            .find_map(|key| value[*key].as_array().cloned())
            .unwrap_or_else(|| vec![value])
        };
        return values.into_iter().map(|value| {
            if cancel.is_cancelled() {return Err(ServiceError::Cancelled);}
            if value.get("version").is_some() && value.get("session").is_some() {return serde_json::from_value(value).map_err(failure);}
            let title = first_text(&value,&["title","name","subject","meeting_title","meetingTitle","topic"]).unwrap_or(title);
            let notes = first_text(&value,&["notes","note","content","markdown","private_notes","enhanced_notes","meeting_notes"]);
            let summary = first_text(&value,&["summary","ai_summary","aiSummary","overview","synopsis"]);
            let mut meeting = from_markdown(title, notes.unwrap_or(""))?;
            if let Some(summary) = summary {
                meeting.documents.push(json!({"id":Uuid::new_v4().to_string(),"session_id":meeting.session["id"],"kind":"summary","body_format":"prosemirror_json","body":anlg_tiptap::md_to_tiptap_json(summary).map_err(failure)?.to_string()}));
            }
            let transcript = ["transcript","transcription","transcriptSegments","utterances","segments","sentences","dialogue"].iter().find_map(|key|value.get(*key));
            if let Some(transcript) = transcript {
                let segments = match transcript {
                    Value::String(text) => text_segments(text),
                    Value::Array(segments) => segments.clone(),
                    _ => return Err(failure("Transcript must be text or segments; original file is unchanged")),
                };
                let mut words = Vec::new();
                for (index,segment) in segments.iter().enumerate() {
                    if cancel.is_cancelled() {return Err(ServiceError::Cancelled);}
                    let text = first_text(segment,&["text","content","sentence"]).ok_or_else(|| failure("Transcript segment lacks text"))?;
                    let start = segment["start_ms"].as_u64().or(segment["startMs"].as_u64()).unwrap_or(index as u64);
                    let end = segment["end_ms"].as_u64().or(segment["endMs"].as_u64()).unwrap_or(start + 1);
                    words.push(json!({"id":Uuid::new_v4().to_string(),"text":text,"start_ms":start,"end_ms":end,"channel":0,"speaker":segment["speaker"]}));
                }
                let ended_at=words.iter().filter_map(|word|word["end_ms"].as_u64()).max().unwrap_or(0);
                meeting.transcripts.push(json!({"id":Uuid::new_v4().to_string(),"session_id":meeting.session["id"],"source":"import","started_at_ms":0,"ended_at_ms":ended_at,"words_json":serde_json::to_string(&words).map_err(failure)?,"metadata_json":value.to_string()}));
            }
            if notes.is_none() && summary.is_none() && transcript.is_none() {
                meeting = from_markdown(title,&format!("```json\n{}\n```",serde_json::to_string_pretty(&value).map_err(failure)?))?;
            }
            if let Some(attendees) = value["attendees"].as_array().or(value["participants"].as_array()) {
                for person in attendees { meeting.participants.push(json!({"id":Uuid::new_v4().to_string(),"session_id":meeting.session["id"],"display_name":person["name"].as_str().or(person.as_str()).unwrap_or(""),"email":person["email"].as_str().unwrap_or(""),"source":"auto"})); }
            }
            // Shipping's `parseJsonExport`: the meeting keeps its own dates.
            let date = |keys: &[&str]| keys.iter().find_map(|key| iso_date(&value[*key]));
            if let Some(started) = date(&["started_at","startedAt","start_time","startTime","meeting_date","meetingDate","recording_date","recordingDate","date","created_at","createdAt"]) {
                meeting.session["created_at"] = json!(started);
                meeting.session["started_at"] = json!(started);
                for row in meeting.documents.iter_mut().chain(&mut meeting.transcripts).chain(&mut meeting.participants) {
                    row["created_at"] = json!(started);
                }
            }
            if let Some(ended) = date(&["ended_at","endedAt","end_time","endTime"]) {
                meeting.session["ended_at"] = json!(ended);
            }
            meeting.session["metadata_json"] = json!(json!({"import_original":value}).to_string());
            Ok(meeting)
        }).collect();
    }
    match extension {
        "md" | "markdown" => Ok(vec![from_markdown(title, content)?]),
        "txt" => parse(
            "json",
            title,
            &json!({"title":title,"transcript":text_segments(content),"import_original":content})
                .to_string(),
        ),
        "csv" => {
            let mut reader = csv::Reader::from_reader(content.as_bytes());
            let headers = reader.headers().map_err(failure)?.clone();
            let values = reader
                .records()
                .map(|record| {
                    let record = record.map_err(failure)?;
                    Ok(Value::Object(
                        headers
                            .iter()
                            .zip(record.iter())
                            .map(|(key, value)| (key.to_string(), json!(value)))
                            .collect(),
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            parse(
                "json",
                title,
                &serde_json::to_string(&values).map_err(failure)?,
            )
        }
        "vtt" | "srt" => {
            let segments = anlg_meeting_import::parse_vtt(content);
            if segments.is_empty() {
                return Err(failure("No valid caption segments found"));
            }
            let value = json!({"title":title,"transcript":segments,"import_original":content});
            parse("json", title, &value.to_string())
        }
        _ => Err(failure(
            "Choose CSV, Markdown, text, SRT, VTT or meeting JSON",
        )),
    }
}

fn from_markdown(title: &str, content: &str) -> Result<CanonicalMeeting> {
    let id = Uuid::new_v4().to_string();
    let mut body = anlg_tiptap::md_to_tiptap_json(content).map_err(failure)?;
    if body["content"].as_array().is_some_and(Vec::is_empty) {
        body["content"] = json!([{"type": "paragraph"}]);
    }
    Ok(CanonicalMeeting {
        version: 1,
        session: json!({"id":id,"title":title,"kind":"meeting"}),
        documents: vec![
            json!({"id":id,"session_id":id,"body_format":"prosemirror_json","body":body.to_string()}),
        ],
        transcripts: vec![],
        participants: vec![],
        humans: vec![],
    })
}

/// Shipping's `toIsoDate`: epoch seconds or milliseconds, or a date string, as an ISO instant.
fn iso_date(value: &Value) -> Option<String> {
    use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
    let date = match value {
        Value::Number(number) => {
            let number = number.as_f64()?;
            let millis = if number < 10_000_000_000. {
                number * 1000.
            } else {
                number
            };
            DateTime::from_timestamp_millis(millis as i64)?
        }
        Value::String(text) => {
            let text = text.trim();
            DateTime::parse_from_rfc3339(text)
                .map(|date| date.with_timezone(&Utc))
                .ok()
                .or_else(|| {
                    // Like `Date`, a time without an offset is local and a bare date is UTC.
                    [
                        "%Y-%m-%dT%H:%M:%S%.f",
                        "%Y-%m-%d %H:%M:%S%.f",
                        "%Y-%m-%dT%H:%M",
                        "%Y-%m-%d %H:%M",
                    ]
                    .iter()
                    .find_map(|format| NaiveDateTime::parse_from_str(text, format).ok())
                    .and_then(|date| Local.from_local_datetime(&date).earliest())
                    .map(|date| date.with_timezone(&Utc))
                })
                .or_else(|| {
                    NaiveDate::parse_from_str(text, "%Y-%m-%d")
                        .ok()?
                        .and_hms_opt(0, 0, 0)
                        .map(|date| date.and_utc())
                })?
        }
        _ => return None,
    };
    Some(date.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
}

fn first_text<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| value[*key].as_str())
}

fn text_segments(content: &str) -> Vec<Value> {
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
        .map(|(index, line)| {
            let (speaker, text) = line
                .split_once(": ")
                .filter(|(speaker, _)| speaker.len() < 60)
                .unwrap_or(("", line));
            json!({"speaker":speaker,"text":text,"start_ms":index,"end_ms":index+1})
        })
        .collect()
}

pub async fn setting_value(runtime: &RuntimeHandle, key: Arc<str>) -> Result<Option<Value>> {
    runtime
        .submit(move |services| async move {
            let rows = services
                .executor
                .execute(
                    "SELECT value_json FROM app_settings WHERE id=?".into(),
                    vec![json!(key)],
                )
                .await
                .map_err(failure)?;
            rows.first()
                .map(|row| {
                    serde_json::from_str(row["value_json"].as_str().unwrap_or("null"))
                        .map_err(failure)
                })
                .transpose()
        })?
        .receive()
        .await
}
