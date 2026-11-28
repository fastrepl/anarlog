//! Full-text search for the chat's `@` mentions and its meeting-search tool. Shipping projects
//! the library into Tantivy (`apps/desktop/src-tauri/src/search_index.rs`) and queries it with
//! the `multilang` n-gram tokenizer (`plugins/tantivy`); the native app builds the same
//! documents from SQLite and matches the way that index does: lowercased, ASCII-folded, any
//! query word found in the title or the content, title hits ranked first.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use chrono::DateTime;
use desktop_runtime::{CancellationToken, Reply, Result, RuntimeHandle, ServiceError};
use serde_json::{Map, Value, json};

use crate::meeting::model::failure;

/// Shipping's search request limit.
const LIMIT: usize = 100;
const MAX_RECORDS: usize = 20_000;
/// Per-field cap; a single huge transcript cannot take over the cache.
const FIELD_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub score: u32,
    pub id: String,
    /// `session`, `human` or `organization`, like shipping's `doc_type`.
    pub kind: &'static str,
    pub title: String,
    pub content: String,
    pub created_at: i64,
}

struct Entry {
    revision: String,
    hit: Hit,
    title: String,
    content: String,
}

/// The projected documents, reused while their source rows are unchanged.
#[derive(Clone, Default)]
pub struct SearchIndex(Arc<Mutex<HashMap<String, Entry>>>);

impl SearchIndex {
    pub fn search(
        &self,
        runtime: &RuntimeHandle,
        query: String,
        kind: Option<&'static str>,
        cancel: CancellationToken,
    ) -> Result<Reply<Vec<Hit>>> {
        let cache = self.0.clone();
        let check = cancel.clone();
        runtime.read(cancel, move |services| async move {
            let started = std::time::Instant::now();
            let filter_count = usize::from(kind.is_some());
            let result: Result<Vec<Hit>> = async {
                let rows = services
                    .executor
                    .execute(REVISIONS.into(), vec![])
                    .await
                    .map_err(failure)?;
                let mut stale = Vec::new();
                {
                    let cache = cache.lock().unwrap_or_else(|poison| poison.into_inner());
                    for row in rows.iter().take(MAX_RECORDS) {
                        let key = key(row);
                        if cache.get(&key).is_none_or(|entry| {
                            entry.revision != row["revision"].as_str().unwrap_or("")
                        }) {
                            stale.push(row.clone());
                        }
                    }
                }
                let mut fresh = Vec::with_capacity(stale.len());
                for row in stale {
                    if check.is_cancelled() {
                        return Err(ServiceError::Cancelled);
                    }
                    fresh.push((key(&row), project(&services.executor, &row).await?));
                }
                let mut cache = cache.lock().unwrap_or_else(|poison| poison.into_inner());
                let live = rows
                    .iter()
                    .take(MAX_RECORDS)
                    .map(key)
                    .collect::<std::collections::HashSet<_>>();
                cache.retain(|key, _| live.contains(key));
                cache.extend(fresh);
                Ok(rank(cache.values(), &query, kind))
            }
            .await;
            let latency_ms = started.elapsed().as_millis() as u64;
            match &result {
                Ok(hits) => {
                    crate::telemetry::track(
                        "search_performed",
                        &[
                            ("outcome", json!("succeeded")),
                            ("latency_ms", json!(latency_ms)),
                            ("filter_count", json!(filter_count)),
                            ("result_count", json!(hits.len())),
                        ],
                    );
                }
                Err(error) if !matches!(error, ServiceError::Cancelled) => {
                    crate::telemetry::track(
                        "search_performed",
                        &[
                            ("outcome", json!("failed")),
                            ("latency_ms", json!(latency_ms)),
                            ("filter_count", json!(filter_count)),
                        ],
                    );
                    crate::telemetry::capture_operational_error(
                        "search",
                        Some("query"),
                        &error.to_string(),
                        None,
                        None,
                    );
                }
                Err(_) => {}
            }
            result
        })
    }
}

const REVISIONS: &str = "SELECT 'session' AS kind, s.id, s.updated_at || '|' || COALESCE((SELECT max(updated_at) FROM session_documents WHERE session_id = s.id), '') || '|' || COALESCE((SELECT max(updated_at) FROM transcripts WHERE session_id = s.id), '') AS revision FROM sessions AS s WHERE s.deleted_at IS NULL UNION ALL SELECT 'human', id, updated_at FROM humans WHERE deleted_at IS NULL UNION ALL SELECT 'organization', id, updated_at FROM organizations WHERE deleted_at IS NULL";

fn key(row: &Value) -> String {
    format!(
        "{}:{}",
        row["kind"].as_str().unwrap_or(""),
        row["id"].as_str().unwrap_or("")
    )
}

async fn project(executor: &anlg_db_execute::DbExecutor, row: &Value) -> Result<Entry> {
    let id = row["id"].as_str().unwrap_or_default().to_owned();
    let query = |sql: &str| executor.execute(sql.into(), vec![json!(id), json!(FIELD_BYTES)]);
    let text = |row: &Value, key: &str| row[key].as_str().unwrap_or_default().to_owned();
    let (kind, title, content, created_at) = match row["kind"].as_str() {
        Some("session") => {
            let session = query("SELECT title, created_at, event_json, source_apps_json, locked, ?2 AS cap FROM sessions WHERE id = ?1").await.map_err(failure)?;
            let session = session.first().cloned().unwrap_or_default();
            let created_at =
                session_timestamp(&text(&session, "event_json"), &text(&session, "created_at"));
            let mut parts = Vec::new();
            if session["locked"].as_i64().unwrap_or(0) == 0 {
                let documents = query("SELECT kind, substr(body, 1, ?2) AS body FROM session_documents WHERE session_id = ?1 AND deleted_at IS NULL AND kind IN ('note', 'summary', 'template_output', 'meeting_chat') ORDER BY CASE kind WHEN 'note' THEN 0 WHEN 'meeting_chat' THEN 2 ELSE 1 END, CASE WHEN id = ?1 THEN 0 ELSE 1 END, sort_order, created_at, id").await.map_err(failure)?;
                let mut note = false;
                for document in &documents {
                    let body = text(document, "body");
                    match document["kind"].as_str() {
                        // Only the session's first note, like shipping's projection.
                        Some("note") if !std::mem::replace(&mut note, true) => {
                            parts.push(plain_text(&body))
                        }
                        Some("summary" | "template_output") => parts.push(plain_text(&body)),
                        Some("meeting_chat") => parts.push(meeting_chat(&body)),
                        _ => {}
                    }
                }
                let transcripts = query("SELECT substr(words_json, 1, ?2) AS words FROM transcripts WHERE session_id = ?1 AND deleted_at IS NULL ORDER BY started_at_ms, created_at, id").await.map_err(failure)?;
                parts.extend(
                    transcripts
                        .iter()
                        .map(|row| transcript(&text(row, "words"))),
                );
                parts.push(source_apps(&text(&session, "source_apps_json")));
            }
            (
                "session",
                fallback(&text(&session, "title"), "Untitled"),
                merge(&parts),
                created_at,
            )
        }
        Some("human") => {
            let human = query("SELECT name, email, job_title, linkedin_username, substr(memo, 1, ?2) AS memo, created_at FROM humans WHERE id = ?1").await.map_err(failure)?;
            let human = human.first().cloned().unwrap_or_default();
            let parts =
                ["email", "job_title", "linkedin_username", "memo"].map(|key| text(&human, key));
            (
                "human",
                fallback(&text(&human, "name"), "Unknown"),
                merge(&parts),
                timestamp(&text(&human, "created_at")),
            )
        }
        _ => {
            let organization =
                query("SELECT name, created_at, ?2 AS cap FROM organizations WHERE id = ?1")
                    .await
                    .map_err(failure)?;
            let organization = organization.first().cloned().unwrap_or_default();
            (
                "organization",
                fallback(&text(&organization, "name"), "Unknown Organization"),
                String::new(),
                timestamp(&text(&organization, "created_at")),
            )
        }
    };
    Ok(Entry {
        revision: text(row, "revision"),
        title: fold(&title),
        content: fold(&content),
        hit: Hit {
            score: 0,
            id,
            kind,
            title,
            content,
            created_at,
        },
    })
}

fn rank<'a>(entries: impl Iterator<Item = &'a Entry>, query: &str, kind: Option<&str>) -> Vec<Hit> {
    let terms = query
        .split_whitespace()
        .take(32)
        .map(fold)
        .collect::<Vec<_>>();
    let mut hits = entries
        .filter(|entry| kind.is_none_or(|kind| entry.hit.kind == kind))
        .filter_map(|entry| {
            // An empty query lists everything, like Tantivy's `AllQuery`.
            let score = terms
                .iter()
                .map(|term| {
                    2 * u32::from(entry.title.contains(term.as_str()))
                        + u32::from(entry.content.contains(term.as_str()))
                })
                .sum::<u32>();
            (terms.is_empty() || score > 0).then_some((score, entry))
        })
        .collect::<Vec<_>>();
    hits.sort_by(|(a, left), (b, right)| {
        b.cmp(a)
            .then(right.hit.created_at.cmp(&left.hit.created_at))
            .then(left.hit.id.cmp(&right.hit.id))
    });
    hits.into_iter()
        .take(LIMIT)
        .map(|(score, entry)| Hit {
            score,
            ..entry.hit.clone()
        })
        .collect()
}

/// The `multilang` tokenizer's `LowerCaser` + `AsciiFoldingFilter`, for the common accents.
pub fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|character| match character {
            'à'..='å' | 'ā' | 'ă' | 'ą' => 'a',
            'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
            'ď' | 'đ' => 'd',
            'è'..='ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
            'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
            'ì'..='ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => 'i',
            'ñ' | 'ń' | 'ņ' | 'ň' => 'n',
            'ò'..='ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => 'o',
            'ś' | 'ŝ' | 'ş' | 'š' => 's',
            'ţ' | 'ť' => 't',
            'ù'..='ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
            'ý' | 'ÿ' => 'y',
            'ź' | 'ż' | 'ž' => 'z',
            character => character,
        })
        .collect()
}

fn fallback(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.into()
    } else {
        value.into()
    }
}

fn merge<S: AsRef<str>>(parts: &[S]) -> String {
    parts
        .iter()
        .map(|part| part.as_ref().trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// TipTap JSON to its words; other bodies are already text.
fn plain_text(body: &str) -> String {
    fn text(node: &Value) -> String {
        match node["text"].as_str() {
            Some(text) if !text.is_empty() => text.to_owned(),
            _ => node["content"]
                .as_array()
                .map(|children| children.iter().map(text).collect::<Vec<_>>().join(" "))
                .unwrap_or_default(),
        }
    }
    let trimmed = body.trim();
    match serde_json::from_str::<Value>(trimmed) {
        Ok(doc)
            if trimmed.starts_with('{') && doc["type"] == "doc" && doc["content"].is_array() =>
        {
            text(&doc).split_whitespace().collect::<Vec<_>>().join(" ")
        }
        _ => trimmed.to_owned(),
    }
}

fn meeting_chat(body: &str) -> String {
    let Ok(Value::Object(record)) = serde_json::from_str::<Value>(body) else {
        return plain_text(body);
    };
    let mut parts = ["platform", "sender", "timestamp", "text"]
        .iter()
        .filter_map(|key| record.get(*key).and_then(Value::as_str))
        .collect::<Vec<_>>();
    parts.extend(
        record
            .get("links")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str),
    );
    merge(&parts)
}

fn transcript(words: &str) -> String {
    fn flatten(value: &Value) -> String {
        match value {
            Value::String(text) => text.clone(),
            Value::Array(items) => merge(
                &items
                    .iter()
                    .filter_map(|item| match item {
                        Value::String(text) => Some(text.clone()),
                        Value::Object(record) => Some(record_text(record)),
                        Value::Array(_) => Some(flatten(item)),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
            ),
            Value::Object(record) => merge(&record.values().map(nested).collect::<Vec<_>>()),
            _ => String::new(),
        }
    }
    fn record_text(record: &Map<String, Value>) -> String {
        let preferred = record
            .get("text")
            .filter(|value| !value.is_null())
            .or_else(|| record.get("content"));
        match preferred.and_then(Value::as_str) {
            Some(text) => text.to_owned(),
            None => merge(&record.values().map(nested).collect::<Vec<_>>()),
        }
    }
    fn nested(value: &Value) -> String {
        match value {
            Value::String(text) => {
                flatten(&serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.clone())))
            }
            value => flatten(value),
        }
    }
    flatten(&serde_json::from_str(words).unwrap_or_else(|_| Value::String(words.into())))
}

fn source_apps(json: &str) -> String {
    let Ok(Value::Array(apps)) = serde_json::from_str::<Value>(json) else {
        return String::new();
    };
    merge(
        &apps
            .iter()
            .filter_map(Value::as_object)
            .flat_map(|app| {
                ["app", "name", "platform"]
                    .into_iter()
                    .filter_map(|key| app.get(key).and_then(Value::as_str))
            })
            .collect::<Vec<_>>(),
    )
}

fn timestamp(value: &str) -> i64 {
    DateTime::parse_from_rfc3339(value)
        .map(|at| at.timestamp_millis())
        .or_else(|_| value.parse::<f64>().map(|ms| ms as i64))
        .unwrap_or(0)
}

/// An event's start, else the session's creation, like shipping's `sessionSearchTimestamp`.
pub(crate) fn session_timestamp(event_json: &str, created_at: &str) -> i64 {
    serde_json::from_str::<Value>(event_json)
        .ok()
        .and_then(|event| match &event["started_at"] {
            Value::Number(ms) => ms.as_f64().map(|ms| ms as i64),
            Value::String(at) => Some(timestamp(at)),
            _ => None,
        })
        .filter(|ms| *ms > 0)
        .unwrap_or_else(|| timestamp(created_at))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MentionKind {
    Session,
    Human,
    Organization,
    Folder,
}

impl MentionKind {
    pub fn key(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Human => "human",
            Self::Organization => "organization",
            Self::Folder => "folder",
        }
    }

    pub fn parse(key: &str) -> Option<Self> {
        Some(match key {
            "session" => Self::Session,
            "human" => Self::Human,
            "organization" => Self::Organization,
            "folder" => Self::Folder,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MentionItem {
    pub id: String,
    pub kind: MentionKind,
    pub label: String,
}

/// Round-robin over the lists, like shipping's `interleave`.
fn interleave(lists: Vec<Vec<MentionItem>>) -> Vec<MentionItem> {
    let mut items = Vec::new();
    for index in 0.. {
        let before = items.len();
        items.extend(lists.iter().filter_map(|list| list.get(index).cloned()));
        if items.len() == before {
            break;
        }
    }
    items
}

/// Shipping's `useMentionConfig().handleSearch`: with a query, matching folder paths then the
/// index hits grouped by type in first-seen order; without one, the first notes, people,
/// organizations and folders. Five at most.
pub fn mentions(
    index: &SearchIndex,
    runtime: &RuntimeHandle,
    query: String,
    cancel: CancellationToken,
) -> futures::future::BoxFuture<'static, Result<Vec<MentionItem>>> {
    let runtime = runtime.clone();
    let index = index.clone();
    Box::pin(async move {
        let folders = runtime
            .read(cancel.clone(), |services| async move {
                services.executor.execute("SELECT folder_path FROM sessions WHERE deleted_at IS NULL AND folder_path != '' UNION SELECT folder_path FROM folder_attachments WHERE deleted_at IS NULL AND folder_path != '' UNION SELECT path FROM folders WHERE deleted_at IS NULL AND path != ''".into(), vec![]).await.map_err(failure)
            })?
            .receive()
            .await?;
        let folders = folder_paths(
            folders
                .iter()
                .filter_map(|row| row.as_object()?.values().next()?.as_str()),
        );
        let needle = query.trim().to_lowercase();
        if !needle.is_empty() {
            let folders = folders
                .into_iter()
                .filter(|path| path.to_lowercase().contains(&needle))
                .map(|path| MentionItem {
                    id: path.clone(),
                    kind: MentionKind::Folder,
                    label: path,
                })
                .collect();
            let hits = index
                .search(&runtime, normalize(&query), None, cancel)?
                .receive()
                .await?;
            let mut groups: Vec<Vec<MentionItem>> = Vec::new();
            let mut order: Vec<&'static str> = Vec::new();
            for hit in hits {
                let Some(kind) = MentionKind::parse(hit.kind) else {
                    continue;
                };
                let position = order
                    .iter()
                    .position(|seen| *seen == hit.kind)
                    .unwrap_or_else(|| {
                        order.push(hit.kind);
                        groups.push(Vec::new());
                        order.len() - 1
                    });
                groups[position].push(MentionItem {
                    id: hit.id,
                    kind,
                    label: hit.title,
                });
            }
            let mut lists = vec![folders];
            lists.extend(groups);
            let mut items = interleave(lists);
            items.truncate(5);
            return Ok(items);
        }
        let rows = runtime
            .read(cancel, |services| async move {
                services.executor.execute("SELECT * FROM (SELECT 'session' AS kind, id, title AS label FROM sessions WHERE deleted_at IS NULL AND title != '' ORDER BY created_at, id LIMIT 5) UNION ALL SELECT * FROM (SELECT 'human', id, name FROM humans WHERE deleted_at IS NULL AND name != '' ORDER BY name, email, id LIMIT 5) UNION ALL SELECT * FROM (SELECT 'organization', id, name FROM organizations WHERE deleted_at IS NULL AND name != '' ORDER BY name, id LIMIT 5)".into(), vec![]).await.map_err(failure)
            })?
            .receive()
            .await?;
        let mut lists = vec![Vec::new(), Vec::new(), Vec::new()];
        for row in &rows {
            let Some(kind) = row["kind"].as_str().and_then(MentionKind::parse) else {
                continue;
            };
            lists[kind as usize].push(MentionItem {
                id: row["id"].as_str().unwrap_or_default().into(),
                kind,
                label: row["label"].as_str().unwrap_or_default().into(),
            });
        }
        lists.push(
            folders
                .into_iter()
                .map(|path| MentionItem {
                    id: path.clone(),
                    kind: MentionKind::Folder,
                    label: path,
                })
                .collect(),
        );
        let mut items = interleave(lists);
        items.truncate(5);
        Ok(items)
    })
}

/// Shipping's `search_meetings` results: the best five notes with a 180-character excerpt.
pub async fn meetings(
    index: &SearchIndex,
    runtime: &RuntimeHandle,
    query: &str,
    cancel: CancellationToken,
) -> Result<Value> {
    let hits = index
        .search(runtime, normalize(query), Some("session"), cancel)?
        .receive()
        .await?;
    let results = hits
        .into_iter()
        .take(5)
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
    Ok(json!({ "results": results }))
}

/// `normalizeQuery`: trimmed, whitespace runs collapsed.
pub fn normalize(query: &str) -> String {
    query.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `collectFolderPaths`: every path and its ancestors, sorted.
fn folder_paths<'a>(paths: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut collected = std::collections::BTreeSet::new();
    for path in paths {
        let segments = path
            .split('/')
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        for count in 1..=segments.len() {
            collected.insert(segments[..count].join("/"));
        }
    }
    collected.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: &'static str, id: &str, title: &str, content: &str, created_at: i64) -> Entry {
        Entry {
            revision: String::new(),
            title: fold(title),
            content: fold(content),
            hit: Hit {
                score: 0,
                id: id.into(),
                kind,
                title: title.into(),
                content: content.into(),
                created_at,
            },
        }
    }

    #[test]
    fn matches_any_word_in_title_or_content_with_title_hits_first() {
        let entries = [
            entry(
                "session",
                "body",
                "Retro",
                "We talked about the Café launch",
                3,
            ),
            entry("session", "title", "Launch review", "", 1),
            entry("human", "maya", "Maya Chen", "maya@example.com", 2),
            entry("session", "none", "Standup", "nothing here", 4),
        ];
        let ids = |hits: Vec<Hit>| hits.into_iter().map(|hit| hit.id).collect::<Vec<_>>();
        assert_eq!(ids(rank(entries.iter(), "LAUNCH", None)), ["title", "body"]);
        assert_eq!(
            ids(rank(entries.iter(), "cafe maya", None)),
            ["maya", "body"]
        );
        assert_eq!(
            ids(rank(entries.iter(), "maya", Some("session"))),
            Vec::<String>::new()
        );
        assert_eq!(rank(entries.iter(), "", None).len(), 4);
    }

    #[test]
    fn projection_reads_tiptap_transcripts_and_meeting_chat_like_shipping() {
        assert_eq!(
            plain_text(
                r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Ship"},{"type":"text","text":"  it"}]}]}"#
            ),
            "Ship it"
        );
        assert_eq!(plain_text(" plain "), "plain");
        assert_eq!(
            transcript(r#"[{"text":" hello"},{"text":"world","speaker":1}]"#),
            "hello world"
        );
        assert_eq!(
            meeting_chat(
                r#"{"platform":"Slack","sender":"Ada","text":"Review","links":["https://x.test"]}"#
            ),
            "Slack Ada Review https://x.test"
        );
        assert_eq!(
            session_timestamp(
                r#"{"started_at":"2026-01-01T00:00:00Z"}"#,
                "2025-01-01T00:00:00Z"
            ),
            1_767_225_600_000
        );
    }

    #[test]
    fn mentions_interleave_groups_and_folder_paths_include_ancestors() {
        let item = |id: &str, kind| MentionItem {
            id: id.into(),
            kind,
            label: id.into(),
        };
        let items = interleave(vec![
            vec![
                item("s1", MentionKind::Session),
                item("s2", MentionKind::Session),
            ],
            vec![item("h1", MentionKind::Human)],
            vec![],
        ]);
        assert_eq!(
            items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["s1", "h1", "s2"]
        );
        assert_eq!(
            folder_paths(["CS 101/Week 1", "Research", "CS 101"].into_iter()),
            ["CS 101", "CS 101/Week 1", "Research"]
        );
    }

    #[test]
    fn index_reprojects_changed_rows_and_drops_deleted_ones() {
        futures::executor::block_on(async {
            let directory = tempfile::tempdir().unwrap();
            let (runtime, ready) = RuntimeHandle::start(desktop_runtime::Profile {
                database: directory.path().join("search.sqlite"),
            })
            .unwrap();
            ready.receive().await.unwrap();
            let run = |sql: &'static str| {
                runtime
                    .submit(move |services| async move {
                        services
                            .executor
                            .execute(sql.into(), vec![])
                            .await
                            .map_err(failure)
                    })
                    .unwrap()
                    .receive()
            };
            run("INSERT INTO sessions (id, title, created_at, updated_at) VALUES ('s', 'Weekly sync', '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z')").await.unwrap();
            run("INSERT INTO session_documents (id, session_id, kind, body_format, body) VALUES ('s', 's', 'note', 'prosemirror_json', '{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\",\"content\":[{\"type\":\"text\",\"text\":\"Budget approved\"}]}]}')").await.unwrap();
            run("INSERT INTO humans (id, name, email) VALUES ('h', 'Maya Chen', 'maya@example.com')").await.unwrap();
            let index = SearchIndex::default();
            let search = |query: &str| {
                index
                    .search(&runtime, query.into(), None, CancellationToken::new())
                    .unwrap()
                    .receive()
            };
            let hits = search("budget").await.unwrap();
            assert_eq!(hits.len(), 1);
            assert_eq!((hits[0].id.as_str(), hits[0].kind), ("s", "session"));
            assert_eq!(search("example").await.unwrap()[0].id, "h");
            run("UPDATE session_documents SET body = 'Forecast moved', updated_at = '2027-01-01T00:00:00.000Z' WHERE id = 's'").await.unwrap();
            assert!(search("budget").await.unwrap().is_empty());
            assert_eq!(search("forecast").await.unwrap().len(), 1);
            run("UPDATE humans SET deleted_at = '2027-01-01T00:00:00.000Z' WHERE id = 'h'")
                .await
                .unwrap();
            assert!(search("maya").await.unwrap().is_empty());
            runtime.shutdown().await.unwrap();
        });
    }
}
