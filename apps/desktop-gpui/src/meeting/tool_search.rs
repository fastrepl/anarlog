//! Shipping's read-only chat search tools: `search-meetings.ts`, `note-files.ts`,
//! `search-contacts.ts`, `search-calendar-events.ts` and `web-search.ts`.

use std::collections::HashSet;

use chrono::{Local, TimeZone};
use desktop_runtime::{CancellationToken, Result, RuntimeHandle, ServiceError};
use serde_json::{Value, json};

use super::{config::CloudResolver, model::failure};
use crate::chat::search::{SearchIndex, normalize, session_timestamp};

const DEFAULT_CONTENT_LIMIT: u32 = 5;
const MAX_CONTENT_LIMIT: u32 = 10;
const SNIPPET_RADIUS: usize = 180;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;

async fn rows(
    runtime: &RuntimeHandle,
    cancellation: &CancellationToken,
    sql: &'static str,
    params: Vec<Value>,
) -> Result<Vec<Value>> {
    runtime
        .read(cancellation.clone(), move |services| async move {
            services
                .executor
                .execute(sql.into(), params)
                .await
                .map_err(failure)
        })?
        .receive()
        .await
}

fn text(row: &Value, key: &str) -> String {
    row[key].as_str().unwrap_or_default().to_owned()
}

/// Sessions in the mentioned folders and their subfolders, newest first.
async fn folder_sessions(
    runtime: &RuntimeHandle,
    folders: &[String],
    cancellation: &CancellationToken,
) -> Result<Vec<Value>> {
    let mut sessions = Vec::new();
    let mut seen = HashSet::new();
    for folder in folders {
        for row in rows(
            runtime,
            cancellation,
            "SELECT id, title, created_at, event_json FROM sessions WHERE deleted_at IS NULL AND (folder_path = ?1 OR folder_path LIKE ?1 || '/%') ORDER BY created_at DESC",
            vec![json!(folder)],
        )
        .await?
        {
            if seen.insert(text(&row, "id")) {
                sessions.push(row);
            }
        }
    }
    Ok(sessions)
}

/// The `created_at` bounds of a `search_meetings` filter, in Unix milliseconds.
#[derive(Default)]
struct Bounds {
    gte: Option<i64>,
    lte: Option<i64>,
    gt: Option<i64>,
    lt: Option<i64>,
    eq: Option<i64>,
}

impl Bounds {
    fn parse(filters: Option<&Value>) -> Option<Self> {
        let filter = filters?.get("created_at")?;
        let number = |key: &str| filter[key].as_f64().map(|value| value as i64);
        match filter["kind"].as_str()? {
            "relative" => {
                let days = filter["recent_days"].as_i64()?.max(1);
                let today = Local::now().date_naive();
                let start = Local
                    .from_local_datetime(&today.and_hms_opt(0, 0, 0)?)
                    .earliest()?
                    .timestamp_millis();
                let end = Local
                    .from_local_datetime(&today.and_hms_milli_opt(23, 59, 59, 999)?)
                    .latest()?
                    .timestamp_millis();
                Some(Self {
                    gte: Some(start - (days - 1) * DAY_MS),
                    lte: Some(end),
                    ..Self::default()
                })
            }
            "absolute" => Some(Self {
                gte: number("gte"),
                lte: number("lte"),
                gt: number("gt"),
                lt: number("lt"),
                eq: number("eq"),
            }),
            _ => None,
        }
    }

    /// Shipping's `sessionMatchesCreatedAt`.
    fn matches(&self, at: i64) -> bool {
        at > 0
            && self.eq.is_none_or(|eq| at == eq)
            && self.gte.is_none_or(|gte| at >= gte)
            && self.lte.is_none_or(|lte| at <= lte)
            && self.gt.is_none_or(|gt| at > gt)
            && self.lt.is_none_or(|lt| at < lt)
    }
}

pub async fn search_meetings(
    runtime: &RuntimeHandle,
    index: &SearchIndex,
    query: String,
    filters: Option<Value>,
    limit: usize,
    folders: &[String],
    cancellation: CancellationToken,
) -> Result<Value> {
    let bounds = Bounds::parse(filters.as_ref());
    let matches = |at: i64| bounds.as_ref().is_none_or(|bounds| bounds.matches(at));
    if !folders.is_empty() {
        let sessions = folder_sessions(runtime, folders, &cancellation)
            .await?
            .into_iter()
            .map(|row| {
                let at = session_timestamp(&text(&row, "event_json"), &text(&row, "created_at"));
                (row, at)
            })
            .filter(|(_, at)| matches(*at))
            .collect::<Vec<_>>();
        let mut results = if query.trim().is_empty() {
            sessions
                .iter()
                .take(limit)
                .map(|(row, at)| json!({"id": row["id"], "title": row["title"], "excerpt": "", "score": 0, "created_at": at}))
                .collect::<Vec<_>>()
        } else {
            let ids = sessions.iter().map(|(row, _)| text(row, "id")).collect();
            let notes = load_notes(runtime, ids, &cancellation).await?;
            let mut found = notes
                .iter()
                .filter_map(|note| search_note(note, query.trim()))
                .collect::<Vec<_>>();
            found.sort_by_key(|found| std::cmp::Reverse(found.score));
            found
                .into_iter()
                .take(limit)
                .map(|found| {
                    let at = sessions
                        .iter()
                        .find(|(row, _)| row["id"] == found.session_id.as_str())
                        .map_or(0, |(_, at)| *at);
                    json!({
                        "id": found.session_id,
                        "title": found.title,
                        "excerpt": found.snippets.first().map(|snippet| snippet.1.clone()).unwrap_or_default(),
                        "score": found.score,
                        "created_at": at,
                    })
                })
                .collect()
        };
        results.sort_by(|a, b| {
            b["score"]
                .as_i64()
                .cmp(&a["score"].as_i64())
                .then(b["created_at"].as_i64().cmp(&a["created_at"].as_i64()))
        });
        results.truncate(limit);
        return Ok(json!({"results": results}));
    }
    let hits = index
        .search(runtime, normalize(&query), Some("session"), cancellation)?
        .receive()
        .await?;
    let results = hits
        .into_iter()
        .filter(|hit| matches(hit.created_at))
        .take(limit)
        .map(|hit| {
            json!({
                "id": hit.id,
                "title": hit.title,
                "excerpt": hit.content.chars().take(180).collect::<String>(),
                "score": hit.score,
                "created_at": hit.created_at,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({"results": results}))
}

/// A note's searchable text, like shipping's `loadNoteFile`.
struct NoteFile {
    session_id: String,
    title: String,
    date: Option<String>,
    event_name: Option<String>,
    event_id: Option<String>,
    participants: Vec<(String, String)>,
    sections: Vec<(String, String)>,
}

async fn load_notes(
    runtime: &RuntimeHandle,
    ids: Vec<String>,
    cancellation: &CancellationToken,
) -> Result<Vec<NoteFile>> {
    let check = cancellation.clone();
    runtime
        .read(cancellation.clone(), move |services| async move {
            let mut notes = Vec::new();
            for id in ids {
                if check.is_cancelled() {
                    return Err(ServiceError::Cancelled);
                }
                let meeting = match anlg_agent_access::get_meeting(
                    services.db.pool(),
                    anlg_agent_access::GetMeetingInput {
                        meeting_id: id.clone(),
                    },
                )
                .await
                {
                    Ok(meeting) => meeting,
                    Err(anlg_agent_access::Error::NotFound(_)) => continue,
                    Err(error) => return Err(failure(error)),
                };
                let session = services
                    .executor
                    .execute(
                        "SELECT event_json, event_id FROM sessions WHERE id = ?".into(),
                        vec![json!(id)],
                    )
                    .await
                    .map_err(failure)?;
                let transcripts = services
                    .executor
                    .execute(
                        "SELECT memo, words_json FROM transcripts WHERE session_id = ? AND deleted_at IS NULL ORDER BY created_at, id".into(),
                        vec![json!(id)],
                    )
                    .await
                    .map_err(failure)?;
                let session = session.first().cloned().unwrap_or_default();
                let event = serde_json::from_str::<Value>(&text(&session, "event_json"))
                    .unwrap_or_default();
                notes.push(note_file(meeting, &event, text(&session, "event_id"), &transcripts));
            }
            Ok(notes)
        })?
        .receive()
        .await
}

fn note_file(
    meeting: anlg_agent_access::Meeting,
    event: &Value,
    event_id: String,
    transcripts: &[Value],
) -> NoteFile {
    let trimmed = |value: &str| Some(value.trim().to_owned()).filter(|value| !value.is_empty());
    let mut sections = Vec::new();
    if let Some(memo) = meeting
        .note
        .as_ref()
        .and_then(|note| trimmed(&note.markdown))
    {
        sections.push(("Raw note".to_owned(), memo));
    }
    for summary in &meeting.summaries {
        if let Some(markdown) = trimmed(&summary.markdown) {
            let title = trimmed(&summary.title).unwrap_or_else(|| "Enhanced note".into());
            sections.push((title, markdown));
        }
    }
    let chunks = transcripts
        .iter()
        .filter_map(|transcript| {
            trimmed(&text(transcript, "memo")).or_else(|| {
                let words = serde_json::from_str::<Vec<Value>>(&text(transcript, "words_json"))
                    .unwrap_or_default();
                trimmed(&whitespace(
                    &words
                        .iter()
                        .map(|word| word["text"].as_str().unwrap_or_default())
                        .collect::<Vec<_>>()
                        .join(" "),
                ))
            })
        })
        .collect::<Vec<_>>();
    if !chunks.is_empty() {
        sections.push(("Transcript".into(), chunks.join("\n\n")));
    }
    NoteFile {
        session_id: meeting.id,
        title: trimmed(&meeting.title).unwrap_or_else(|| "Untitled".into()),
        date: Some(meeting.created_at).filter(|date| !date.is_empty()),
        event_name: ["name", "title"]
            .iter()
            .find_map(|key| event[*key].as_str().and_then(trimmed)),
        event_id: Some(event_id).filter(|id| !id.is_empty()),
        participants: meeting
            .participants
            .into_iter()
            .map(|participant| {
                (
                    participant.human_id,
                    participant.display_name.trim().to_owned(),
                )
            })
            .collect(),
        sections,
    }
}

fn whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn lower(text: &str) -> Vec<char> {
    text.chars()
        .map(|character| character.to_lowercase().next().unwrap_or(character))
        .collect()
}

fn find(haystack: &[char], needle: &[char]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Shipping's `queryTerms`: ASCII words, `@`, `-` and `.` kept, two characters or more.
fn query_terms(query: &str) -> Vec<Vec<char>> {
    let mut terms: Vec<Vec<char>> = Vec::new();
    for term in query
        .to_lowercase()
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '@' | '-' | '.')))
        .filter(|term| term.chars().count() >= 2)
    {
        let term = term.chars().collect::<Vec<_>>();
        if !terms.contains(&term) {
            terms.push(term);
        }
    }
    terms
}

fn snippet(text: &[char], index: usize, length: usize) -> String {
    let start = index.saturating_sub(SNIPPET_RADIUS);
    let end = (index + length + SNIPPET_RADIUS).min(text.len());
    format!(
        "{}{}{}",
        if start > 0 { "..." } else { "" },
        whitespace(&text[start..end].iter().collect::<String>()),
        if end < text.len() { "..." } else { "" }
    )
}

struct Found {
    session_id: String,
    title: String,
    date: Option<String>,
    score: i64,
    snippets: Vec<(String, String)>,
}

/// Shipping's `searchNote`: exact phrase, title, participant and event hits, then terms.
fn search_note(note: &NoteFile, query: &str) -> Option<Found> {
    let terms = query_terms(query);
    let needle = lower(query);
    let contains = |value: &str| find(&lower(value), &needle).is_some();
    let mut score = 0;
    let mut snippets = Vec::new();
    if contains(&note.title) {
        score += 8;
        snippets.push(("Title".to_owned(), note.title.clone()));
    }
    let people = note
        .participants
        .iter()
        .map(|(_, name)| name.as_str())
        .filter(|name| !name.is_empty() && contains(name))
        .collect::<Vec<_>>();
    if !people.is_empty() {
        score += 8;
        snippets.push(("Participants".to_owned(), people.join(", ")));
    }
    if let Some(event) = note.event_name.as_deref().filter(|event| contains(event)) {
        score += 8;
        snippets.push(("Event".to_owned(), event.to_owned()));
    }
    for (title, body) in &note.sections {
        let original = body.chars().collect::<Vec<_>>();
        let lowered = lower(body);
        let mut local = Vec::new();
        if let Some(index) = find(&lowered, &needle) {
            score += 20;
            local.push((title.clone(), snippet(&original, index, needle.len())));
        }
        for term in &terms {
            let Some(index) = find(&lowered, term) else {
                continue;
            };
            score += 3;
            if local.len() < 3 {
                local.push((title.clone(), snippet(&original, index, term.len())));
            }
        }
        snippets.extend(local);
    }
    (score > 0 && !snippets.is_empty()).then(|| Found {
        session_id: note.session_id.clone(),
        title: note.title.clone(),
        date: note.date.clone(),
        score,
        snippets: snippets.into_iter().take(3).collect(),
    })
}

async fn active_sessions(
    runtime: &RuntimeHandle,
    cancellation: &CancellationToken,
) -> Result<Vec<String>> {
    Ok(rows(
        runtime,
        cancellation,
        "SELECT id FROM sessions WHERE deleted_at IS NULL ORDER BY created_at DESC, id",
        vec![],
    )
    .await?
    .iter()
    .map(|row| text(row, "id"))
    .collect())
}

pub async fn search_meeting_content(
    runtime: &RuntimeHandle,
    query: String,
    meeting_ids: Option<Vec<String>>,
    limit: Option<u32>,
    folders: &[String],
    cancellation: CancellationToken,
) -> Result<Value> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(json!({"query": query, "results": [], "message": "Query is empty"}));
    }
    let limit = limit
        .unwrap_or(DEFAULT_CONTENT_LIMIT)
        .min(MAX_CONTENT_LIMIT) as usize;
    let ids = if folders.is_empty() {
        meeting_ids
    } else {
        let in_folders = folder_sessions(runtime, folders, &cancellation)
            .await?
            .iter()
            .map(|row| text(row, "id"))
            .collect::<Vec<_>>();
        Some(match meeting_ids.filter(|ids| !ids.is_empty()) {
            Some(ids) => ids
                .into_iter()
                .filter(|id| in_folders.contains(id))
                .collect(),
            None => in_folders,
        })
    };
    let ids = match ids {
        Some(ids) => ids,
        None => active_sessions(runtime, &cancellation).await?,
    };
    let scanned = ids.len();
    let notes = load_notes(runtime, ids, &cancellation).await?;
    let mut found = notes
        .iter()
        .filter_map(|note| search_note(note, trimmed))
        .collect::<Vec<_>>();
    found.sort_by_key(|found| std::cmp::Reverse(found.score));
    let results = found
        .into_iter()
        .take(limit)
        .map(|found| {
            json!({
                "meeting_id": found.session_id,
                "title": found.title,
                "date": found.date,
                "score": found.score,
                "snippets": found.snippets.into_iter().map(|(section, text)| json!({"section": section, "text": text})).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({"query": trimmed, "scanned": scanned, "results": results}))
}

/// Shipping's `listRelatedNotes`: same event, shared participants and nearby dates.
pub async fn find_related(
    runtime: &RuntimeHandle,
    session: String,
    limit: Option<u32>,
    cancellation: CancellationToken,
) -> Result<Value> {
    let limit = limit
        .unwrap_or(DEFAULT_CONTENT_LIMIT)
        .min(MAX_CONTENT_LIMIT) as usize;
    let Some(base) = load_notes(runtime, vec![session.clone()], &cancellation)
        .await?
        .pop()
    else {
        return Ok(json!({
            "status": "error",
            "message": format!("Could not read note {session}"),
            "meeting_id": session,
            "results": [],
        }));
    };
    let others = active_sessions(runtime, &cancellation)
        .await?
        .into_iter()
        .filter(|id| *id != session)
        .collect();
    let date = |note: &NoteFile| {
        note.date
            .as_deref()
            .and_then(|date| chrono::DateTime::parse_from_rfc3339(date).ok())
            .map(|date| date.timestamp_millis())
    };
    let base_date = date(&base);
    let mut results = Vec::new();
    for candidate in load_notes(runtime, others, &cancellation).await? {
        let mut reasons = Vec::new();
        let mut score = 0;
        if base.event_id.is_some() && candidate.event_id == base.event_id {
            reasons.push("same calendar event".to_owned());
            score += 20;
        }
        for (id, name) in &candidate.participants {
            if base.participants.iter().any(|(base, _)| base == id) {
                reasons.push(if name.is_empty() {
                    "shared participant".to_owned()
                } else {
                    format!("shared participant: {name}")
                });
                score += 8;
            }
        }
        if let (Some(base), Some(other)) = (base_date, date(&candidate)) {
            let days = (base - other).abs() as f64 / DAY_MS as f64;
            if days <= 7. {
                reasons.push("nearby date".to_owned());
                score += (7 - days.floor() as i64).max(1);
            }
        }
        if score > 0 {
            results.push((score, candidate, reasons));
        }
    }
    results.sort_by_key(|(score, _, _)| std::cmp::Reverse(*score));
    Ok(json!({
        "status": "ok",
        "meeting_id": session,
        "title": base.title,
        "results": results.into_iter().take(limit).map(|(score, note, reasons)| json!({
            "meeting_id": note.session_id,
            "title": note.title,
            "date": note.date,
            "score": score,
            "reasons": reasons,
        })).collect::<Vec<_>>(),
    }))
}

fn optional(row: &Value, key: &str) -> Value {
    Some(text(row, key))
        .filter(|value| !value.is_empty())
        .map_or(Value::Null, Value::String)
}

pub async fn contacts(
    runtime: &RuntimeHandle,
    query: String,
    limit: u32,
    cancellation: CancellationToken,
) -> Result<Value> {
    let needle = query.trim().to_lowercase();
    let found = rows(
        runtime,
        &cancellation,
        "SELECT humans.id, humans.name, humans.email, humans.phone, humans.job_title, COALESCE(organizations.name, '') AS organization_name, humans.memo FROM humans LEFT JOIN organizations ON organizations.id = humans.organization_id AND organizations.deleted_at IS NULL WHERE humans.deleted_at IS NULL AND (?1 = '' OR lower(humans.name || char(10) || humans.email || char(10) || humans.phone || char(10) || humans.job_title || char(10) || humans.memo || char(10) || COALESCE(organizations.name, '') || char(10) || COALESCE((SELECT group_concat(additional_email.value, char(10)) FROM json_each(CASE WHEN json_valid(humans.metadata_json) AND json_type(humans.metadata_json, '$.additionalEmails') = 'array' THEN json_extract(humans.metadata_json, '$.additionalEmails') ELSE '[]' END) AS additional_email), '')) LIKE '%' || ?1 || '%') ORDER BY humans.created_at DESC, humans.id LIMIT ?2",
        vec![json!(needle), json!(limit)],
    )
    .await?;
    let results = found
        .iter()
        .map(|row| {
            json!({
                "id": row["id"],
                "name": row["name"],
                "email": optional(row, "email"),
                "phone": optional(row, "phone"),
                "jobTitle": optional(row, "job_title"),
                "organization": optional(row, "organization_name"),
                "memo": optional(row, "memo"),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({"query": query, "results": results}))
}

pub async fn calendar_events(
    runtime: &RuntimeHandle,
    query: String,
    limit: u32,
    cancellation: CancellationToken,
) -> Result<Value> {
    let needle = query.trim().to_lowercase();
    let found = rows(
        runtime,
        &cancellation,
        "SELECT event.id, event.title, event.started_at, event.ended_at, event.location, event.meeting_link, event.description, CASE WHEN json_valid(event.participants_json) AND json_type(event.participants_json) = 'array' THEN json_array_length(event.participants_json) ELSE 0 END AS participant_count, COALESCE((SELECT session.id FROM sessions AS session WHERE session.deleted_at IS NULL AND (session.event_id = event.id OR (event.tracking_id_event <> '' AND session.external_event_id = event.tracking_id_event) OR (json_valid(session.event_json) AND json_extract(session.event_json, '$.tracking_id') = event.tracking_id_event AND json_extract(session.event_json, '$.calendar_id') = event.calendar_id)) ORDER BY session.created_at, session.id LIMIT 1), '') AS linked_session_id FROM events AS event WHERE event.deleted_at IS NULL AND (?1 = '' OR instr(lower(event.title || char(10) || event.location || char(10) || event.meeting_link || char(10) || event.description), ?1) > 0) ORDER BY julianday(event.started_at) DESC, event.id LIMIT ?2",
        vec![json!(needle), json!(limit)],
    )
    .await?;
    let results = found
        .iter()
        .map(|row| {
            json!({
                "id": row["id"],
                "title": Some(text(row, "title")).filter(|title| !title.is_empty()).unwrap_or_else(|| "Untitled event".into()),
                "startedAt": optional(row, "started_at"),
                "endedAt": optional(row, "ended_at"),
                "location": optional(row, "location"),
                "meetingLink": optional(row, "meeting_link"),
                "description": optional(row, "description"),
                "participantCount": row["participant_count"].as_i64().unwrap_or(0),
                "linkedSessionId": optional(row, "linked_session_id"),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({"query": query, "results": results}))
}

/// The signed-in account's API access for Anarlog's research endpoint.
#[derive(Clone)]
pub struct WebAccess {
    pub api_url: String,
    pub cloud: CloudResolver,
}

/// Shipping's `runWebSearch`: `POST /research/search` with the account's token.
pub async fn web_search(web: Option<&WebAccess>, body: Value) -> Result<Value> {
    let query = body["query"].clone();
    let error = |message: String| {
        Ok(json!({"status": "error", "message": message, "query": query, "results": []}))
    };
    let Some(web) = web else {
        return error("Sign in to use web search.".into());
    };
    let Ok(access) = (web.cloud)().await else {
        return error("Sign in to use web search.".into());
    };
    let url = format!("{}/research/search", web.api_url.trim_end_matches('/'));
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(failure)?
        .post(url)
        .bearer_auth(access.access_token)
        .json(&body)
        .send()
        .await
        .map_err(|_| failure("Web search could not be reached."))?;
    if !response.status().is_success() {
        return error(format!(
            "Web search failed with HTTP {}.",
            response.status().as_u16()
        ));
    }
    let mut result: Value = response
        .json()
        .await
        .map_err(|_| failure("Web search returned an invalid response."))?;
    if let Some(object) = result.as_object_mut() {
        object.insert("status".into(), json!("ok"));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(title: &str, sections: &[(&str, &str)]) -> NoteFile {
        NoteFile {
            session_id: "s".into(),
            title: title.into(),
            date: None,
            event_name: None,
            event_id: None,
            participants: vec![("h".into(), "Ana Kim".into())],
            sections: sections
                .iter()
                .map(|(title, text)| ((*title).into(), (*text).into()))
                .collect(),
        }
    }

    #[test]
    fn content_search_scores_like_shipping_note_files() {
        let found = search_note(
            &note(
                "Weekly product sync",
                &[
                    ("Raw note", "We will ship the beta on Friday."),
                    ("Transcript", "Ana: beta beta"),
                ],
            ),
            "ship beta",
        )
        .unwrap();
        // Two terms in the note (3 each) and one in the transcript; no exact phrase.
        assert_eq!(found.score, 9);
        assert_eq!(found.snippets[0].0, "Raw note");
        assert_eq!(found.snippets.len(), 3);
        let participant = search_note(&note("Sync", &[]), "ana").unwrap();
        assert_eq!(participant.score, 8);
        assert_eq!(
            participant.snippets[0],
            ("Participants".into(), "Ana Kim".into())
        );
        assert!(search_note(&note("Sync", &[("Raw note", "nothing")]), "budget").is_none());
    }
}
