//! Shipping's pre-meeting brief (`session/insights/pre-meeting.ts`, `past-notes.ts`,
//! `hooks/useCreatePreMeetingBrief.ts` and `pre-meeting-brief-job.ts`): earlier notes with
//! the same series, title or people, their saved `key_facts`, and the brief the configured
//! model streams into an upcoming meeting's empty memo.

use std::{
    collections::HashMap,
    sync::{Arc, LazyLock},
    time::Duration,
};

use chrono::{DateTime, Local, Utc};
use desktop_runtime::{CancellationToken, Result, RuntimeHandle, ServiceError, SessionId};
use futures::StreamExt;
use gpui::{AnyElement, Context, Entity, SharedString, div, prelude::*, px};
use regex::Regex;
use serde_json::{Value, json};

use super::{NoteView, icon};
use crate::ui::a11y::Accessible;
use crate::{
    application::toasts,
    editor::{EditorPane, holds::NoteHolds},
    meeting::{
        ai::{AiServices, StreamObserver, Structured},
        ai_view::AiViewServices,
        live::SessionMode,
        model::failure,
    },
    ui::theme::Theme,
    workspace::calendar::parse_instant,
};

const MAX_PAST_NOTES: usize = 8;
const MAX_SOURCE_LENGTH: usize = 6000;
const MAX_BRIEF_MEETINGS: usize = 5;
const AFTER_START_GRACE_MS: i64 = 5 * 60 * 1000;
const MAX_FACTS: usize = 3;
const MAX_PROMPT_FACTS: usize = 4;
const MAX_BRIEF_BULLETS: usize = 3;
const MAX_SUMMARY_LENGTH: usize = 320;
const BRIEF_TIMEOUT: Duration = Duration::from_secs(45);
const BRIEF_MAX_OUTPUT_TOKENS: u32 = 512;
const ERROR: &str = "Could not create the pre-meeting brief. Try again.";

/// Shipping's `useSessionCalendarEvent` (`SESSION_EVENT_MATCH`, `SESSION_EVENT_ORDER`).
pub(super) const SESSION_EVENT_SQL: &str = "SELECT event.title, event.started_at, event.ended_at,
       event.is_all_day, event.location, event.description, event.participants_json
     FROM sessions AS session
     JOIN events AS event ON event.deleted_at IS NULL AND (event.id = session.event_id OR (
       event.tracking_id_event <> ''
       AND event.tracking_id_event = CASE WHEN json_valid(session.event_json)
         THEN json_extract(session.event_json, '$.tracking_id') ELSE '' END
       AND event.calendar_id = CASE WHEN json_valid(session.event_json)
         THEN json_extract(session.event_json, '$.calendar_id') ELSE '' END))
     WHERE session.id = ? AND session.deleted_at IS NULL
     ORDER BY CASE WHEN event.id = session.event_id THEN 0 ELSE 1 END, event.started_at, event.id
     LIMIT 1";

const SESSIONS_SQL: &str = "SELECT id, owner_user_id AS user_id, title, created_at, event_json
     FROM sessions WHERE deleted_at IS NULL AND locked = 0 ORDER BY created_at, id";

const PARTICIPANTS_SQL: &str = "SELECT participant.session_id, participant.human_id,
       participant.owner_user_id AS user_id, participant.source,
       COALESCE(NULLIF(human.name, ''), NULLIF(participant.display_name, ''), participant.human_id) AS name,
       COALESCE(NULLIF(human.name, ''), participant.display_name) AS display_name,
       COALESCE(NULLIF(human.email, ''), participant.email) AS email
     FROM session_participants AS participant
     LEFT JOIN humans AS human ON human.id = participant.human_id AND human.deleted_at IS NULL
     WHERE participant.deleted_at IS NULL
     ORDER BY participant.session_id, participant.created_at, participant.id";

const DOCUMENTS_SQL: &str = "SELECT session_id, kind, body, sort_order, source_hash
     FROM session_documents
     WHERE kind IN ('summary', 'template_output', 'key_facts') AND deleted_at IS NULL
       AND session_id IN (SELECT value FROM json_each(?))
     ORDER BY session_id, CASE WHEN kind = 'key_facts' THEN updated_at END, sort_order, created_at, id";

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct EventParticipant {
    name: String,
    email: String,
    is_current_user: bool,
}

/// Shipping's `PreMeetingBriefEvent`.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct BriefEvent {
    title: String,
    started_at: String,
    ended_at: String,
    is_all_day: bool,
    location: String,
    description: String,
    participants: Vec<EventParticipant>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Relationship {
    SharedParticipants,
    MatchingTitle,
    SameSeries,
}

/// Shipping's `PastSessionNote`.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct PastNote {
    title: String,
    date_label: String,
    participant_names: Vec<String>,
    source_summary: String,
    relationship: Relationship,
    /// The saved `key_facts` document, when it was made from the current summaries.
    summary: Option<String>,
    date_ms: i64,
}

/// What the brief needs about the open note, loaded when it opens.
#[derive(Clone, Debug, Default)]
pub(super) struct Brief {
    /// The linked calendar event (`useSessionCalendarEvent`).
    event: Option<BriefEvent>,
    /// The note's title and people, briefed when there is no calendar event.
    fallback: BriefEvent,
    has_participants: bool,
    notes: Vec<PastNote>,
}

impl Brief {
    /// Shipping's `canCreatePreMeetingBrief`.
    pub(super) fn available(&self, now: DateTime<Utc>) -> bool {
        (match &self.event {
            Some(event) => should_show(event, now),
            None => self.has_participants,
        }) && !source_notes(&self.notes).is_empty()
    }
}

/// A brief streaming into one session's memo.
pub(super) struct BriefJob {
    pub(super) session: SessionId,
    pub(super) editor: Entity<EditorPane>,
    cancellation: CancellationToken,
}

impl Drop for BriefJob {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

struct SessionRow {
    id: String,
    user_id: String,
    title: String,
    created_at: String,
    event: Value,
}

struct ParticipantRow {
    session_id: String,
    human_id: String,
    user_id: String,
    source: String,
    name: String,
}

fn text(row: &Value, key: &str) -> String {
    row[key].as_str().unwrap_or_default().to_owned()
}

fn event_value(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or(Value::Null)
}

fn parse_participants(raw: &str) -> Vec<EventParticipant> {
    serde_json::from_str::<Vec<Value>>(raw)
        .unwrap_or_default()
        .iter()
        .filter(|participant| participant.is_object())
        .map(|participant| EventParticipant {
            name: text(participant, "name"),
            email: text(participant, "email"),
            is_current_user: participant["is_current_user"] == true,
        })
        .collect()
}

pub(super) async fn load(runtime: &RuntimeHandle, session: SessionId) -> Result<Brief> {
    runtime
        .read(CancellationToken::new(), move |services| async move {
            let executor = &services.executor;
            let rows = executor
                .execute(
                    "SELECT owner_user_id, title FROM sessions WHERE id = ? AND deleted_at IS NULL"
                        .into(),
                    vec![json!(session)],
                )
                .await
                .map_err(failure)?;
            let current = rows.first().ok_or(ServiceError::Conflict)?;
            let owner = text(current, "owner_user_id");
            let event = executor
                .execute(SESSION_EVENT_SQL.into(), vec![json!(session)])
                .await
                .map_err(failure)?
                .into_iter()
                .next()
                .map(|event| BriefEvent {
                    title: text(&event, "title"),
                    started_at: text(&event, "started_at"),
                    ended_at: text(&event, "ended_at"),
                    is_all_day: event["is_all_day"].as_i64().unwrap_or(0) != 0
                        || event["is_all_day"] == true,
                    location: text(&event, "location"),
                    description: text(&event, "description"),
                    participants: parse_participants(
                        event["participants_json"].as_str().unwrap_or("[]"),
                    ),
                });
            let participants = executor
                .execute(PARTICIPANTS_SQL.into(), vec![])
                .await
                .map_err(failure)?;
            let own: Vec<&Value> = participants
                .iter()
                .filter(|row| row["session_id"] == json!(session) && row["source"] != "excluded")
                .collect();
            let fallback = BriefEvent {
                title: text(current, "title"),
                participants: own
                    .iter()
                    .filter(|row| text(row, "human_id") != owner)
                    .map(|row| EventParticipant {
                        name: text(row, "display_name"),
                        email: text(row, "email"),
                        is_current_user: false,
                    })
                    .collect(),
                ..BriefEvent::default()
            };
            let has_participants = own.iter().any(|row| {
                let human = text(row, "human_id");
                !human.is_empty() && human != owner
            });
            let sessions: Vec<SessionRow> = executor
                .execute(SESSIONS_SQL.into(), vec![])
                .await
                .map_err(failure)?
                .iter()
                .map(|row| SessionRow {
                    id: text(row, "id"),
                    user_id: text(row, "user_id"),
                    title: text(row, "title"),
                    created_at: text(row, "created_at"),
                    event: event_value(row["event_json"].as_str().unwrap_or_default()),
                })
                .collect();
            let participants: Vec<ParticipantRow> = participants
                .iter()
                .map(|row| ParticipantRow {
                    session_id: text(row, "session_id"),
                    human_id: text(row, "human_id"),
                    user_id: text(row, "user_id"),
                    source: text(row, "source"),
                    name: text(row, "name"),
                })
                .collect();
            let related: Vec<&str> = candidates(&sessions, &participants, &session.0)
                .1
                .iter()
                .map(|candidate| candidate.row.id.as_str())
                .collect();
            let documents = executor
                .execute(
                    DOCUMENTS_SQL.into(),
                    vec![json!(json!(related).to_string())],
                )
                .await
                .map_err(failure)?;
            let mut summaries: HashMap<String, Vec<(i64, String)>> = HashMap::new();
            let mut key_facts: HashMap<String, (String, String)> = HashMap::new();
            for row in &documents {
                let id = text(row, "session_id");
                if row["kind"] == "key_facts" {
                    key_facts.insert(id, (text(row, "body"), text(row, "source_hash")));
                } else {
                    summaries
                        .entry(id)
                        .or_default()
                        .push((row["sort_order"].as_i64().unwrap_or(0), text(row, "body")));
                }
            }
            Ok(Brief {
                event,
                fallback,
                has_participants,
                notes: past_notes(&sessions, &participants, &summaries, &key_facts, &session.0),
            })
        })?
        .receive()
        .await
}

fn series_id(event: &Value) -> Option<String> {
    event["recurrence_series_id"]
        .as_str()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
}

fn session_title(row: &SessionRow) -> String {
    match row.title.trim() {
        "" => "Untitled".into(),
        title => title.to_owned(),
    }
}

fn title_key(row: &SessionRow) -> String {
    let key = spaces(&session_title(row).to_lowercase(), " ");
    if matches!(key.as_str(), "new note" | "untitled") {
        String::new()
    } else {
        key
    }
}

/// `event.started_at || created_at` as epoch milliseconds, or 0.
fn timestamp(row: &SessionRow) -> i64 {
    let value = row.event["started_at"]
        .as_str()
        .filter(|value| !value.is_empty())
        .unwrap_or(&row.created_at);
    parse_instant(value).map_or(0, |at| at.timestamp_millis())
}

/// date-fns `MMM d, yyyy` in local time.
fn date_label(row: &SessionRow) -> String {
    let value = row.event["started_at"]
        .as_str()
        .filter(|value| !value.is_empty())
        .unwrap_or(&row.created_at);
    parse_instant(value)
        .map(|at| at.with_timezone(&Local).format("%b %-d, %Y").to_string())
        .unwrap_or_default()
}

/// Adds `id` unless present, keeping first-seen order like the JavaScript `Set`s shipping uses.
fn insert<'a>(ids: &mut Vec<&'a str>, id: &'a str) {
    if !ids.contains(&id) {
        ids.push(id);
    }
}

/// Shipping's `getSessionParticipantIds`: the people other than the note's owner.
fn participant_ids<'a>(rows: &[&'a ParticipantRow], user: Option<&str>) -> Vec<&'a str> {
    let mut ids = Vec::new();
    for row in rows {
        if row.source == "excluded" || row.human_id.is_empty() {
            continue;
        }
        let owner = Some(row.user_id.trim()).filter(|owner| !owner.is_empty());
        let current = match user {
            Some(user) => row.human_id == user,
            None => owner.is_some_and(|owner| row.human_id == owner),
        };
        if !current {
            insert(&mut ids, &row.human_id);
        }
    }
    ids
}

fn relationship(
    current: (&[&str], Option<&str>, &str),
    candidate: (&[&str], Option<&str>, &str),
) -> Option<Relationship> {
    let (current_ids, current_series, current_title) = current;
    let (candidate_ids, candidate_series, candidate_title) = candidate;
    if current_series.is_some() && candidate_series == current_series {
        return Some(Relationship::SameSeries);
    }
    let shared = !current_ids.is_empty()
        && !candidate_ids.is_empty()
        && candidate_ids.iter().any(|id| current_ids.contains(id));
    if !current_title.is_empty() && current_title == candidate_title && shared {
        Some(Relationship::MatchingTitle)
    } else if shared {
        Some(Relationship::SharedParticipants)
    } else {
        None
    }
}

struct Candidate<'a> {
    row: &'a SessionRow,
    relationship: Relationship,
    ids: Vec<&'a str>,
}

/// The earlier notes related to `session` by series, title and people, before their summaries
/// are known (shipping's candidate loop up to `getSessionKeyFactsSource`).
fn candidates<'a>(
    sessions: &'a [SessionRow],
    participants: &'a [ParticipantRow],
    session: &str,
) -> (Vec<&'a str>, Vec<Candidate<'a>>) {
    let Some(current) = sessions.iter().find(|row| row.id == session) else {
        return Default::default();
    };
    let mut by_session: HashMap<&str, Vec<&ParticipantRow>> = HashMap::new();
    for row in participants {
        by_session.entry(&row.session_id).or_default().push(row);
    }
    let user = Some(current.user_id.as_str()).filter(|user| !user.is_empty());
    let ids_for = |id: &str| participant_ids(by_session.get(id).map_or(&[], Vec::as_slice), user);
    let current_ids = ids_for(session);
    let current_series = series_id(&current.event);
    let current_title = title_key(current);
    if current_series.is_none() && current_title.is_empty() && current_ids.is_empty() {
        return Default::default();
    }
    let current_time = timestamp(current);
    let mut found = Vec::new();
    for row in sessions.iter().filter(|row| row.id != session) {
        let time = timestamp(row);
        if current_time > 0 && time > 0 && time >= current_time {
            continue;
        }
        let ids = ids_for(&row.id);
        let Some(relationship) = relationship(
            (&current_ids, current_series.as_deref(), &current_title),
            (&ids, series_id(&row.event).as_deref(), &title_key(row)),
        ) else {
            continue;
        };
        found.push(Candidate {
            row,
            relationship,
            ids,
        });
    }
    (current_ids, found)
}

/// Shipping's `buildPastSessionNotes`: up to eight related earlier notes with summaries, by
/// relationship then most recent.
fn past_notes(
    sessions: &[SessionRow],
    participants: &[ParticipantRow],
    summaries: &HashMap<String, Vec<(i64, String)>>,
    key_facts: &HashMap<String, (String, String)>,
    session: &str,
) -> Vec<PastNote> {
    let (current_ids, candidates) = candidates(sessions, participants, session);
    let mut names: HashMap<&str, &str> = HashMap::new();
    for row in participants {
        if !row.name.trim().is_empty() {
            names.insert(&row.human_id, row.name.trim());
        }
    }
    let mut notes: Vec<PastNote> = candidates
        .into_iter()
        .filter_map(|candidate| {
            let source = key_facts_source(summaries.get(&candidate.row.id)?)?;
            let title = session_title(candidate.row);
            let date_label = date_label(candidate.row);
            let hash = source_hash(&[title.as_str(), &date_label, &source].join("\n"));
            let summary = key_facts
                .get(&candidate.row.id)
                .filter(|(body, saved)| saved == &hash && !body.trim().is_empty())
                .map(|(body, _)| body.trim().to_owned());
            let mut all = Vec::new();
            for id in current_ids.iter().chain(&candidate.ids) {
                insert(&mut all, id);
            }
            let mut participant_names: Vec<String> = Vec::new();
            for id in all {
                let name = names.get(id).copied().unwrap_or(id);
                if !participant_names
                    .iter()
                    .any(|seen| seen.to_lowercase() == name.to_lowercase())
                {
                    participant_names.push(name.to_owned());
                }
            }
            participant_names
                .sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then(a.cmp(b)));
            Some(PastNote {
                title,
                date_label,
                participant_names,
                source_summary: source,
                relationship: candidate.relationship,
                summary,
                date_ms: timestamp(candidate.row),
            })
        })
        .collect();
    notes.sort_by(|a, b| {
        (b.relationship as u8)
            .cmp(&(a.relationship as u8))
            .then(b.date_ms.cmp(&a.date_ms))
    });
    notes.truncate(MAX_PAST_NOTES);
    notes
}

fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("brief pattern")
}

static IMAGE: LazyLock<Regex> = LazyLock::new(|| regex(r"!\[[^\]]*]\([^)]+\)"));
static LINK: LazyLock<Regex> = LazyLock::new(|| regex(r"\[([^\]]+)]\([^)]+\)"));
static SPACE: LazyLock<Regex> = LazyLock::new(|| regex(r"\s+"));

fn spaces(text: &str, with: &str) -> String {
    SPACE.replace_all(text, with).into_owned()
}

/// Shipping's `extractPlainText`: TipTap JSON's text joined by spaces, other text as is.
fn plain_text(value: &str) -> String {
    fn node_text(node: &Value) -> String {
        match node["text"].as_str().filter(|text| !text.is_empty()) {
            Some(text) => text.to_owned(),
            None => node["content"]
                .as_array()
                .map(|children| children.iter().map(node_text).collect::<Vec<_>>().join(" "))
                .unwrap_or_default(),
        }
    }
    let trimmed = value.trim();
    if !trimmed.starts_with('{') {
        return trimmed.to_owned();
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(doc) if doc["type"] == "doc" && doc["content"].is_array() => {
            spaces(node_text(&doc).trim(), " ")
        }
        _ => trimmed.to_owned(),
    }
}

/// JavaScript's `slice`-based word truncation, counted in UTF-16 units like shipping.
fn truncate(text: &str, max: usize, ellipsis: &str) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    if units.len() <= max {
        return text.to_owned();
    }
    let slice = &units[..(max + 1).min(units.len())];
    let last_space = slice.iter().rposition(|unit| *unit == u16::from(b' '));
    let end = match last_space {
        Some(space) if space as f64 > max as f64 * 0.6 => space,
        _ => max,
    };
    format!(
        "{}{ellipsis}",
        String::from_utf16_lossy(&slice[..end.min(slice.len())]).trim()
    )
}

/// Shipping's `getSessionKeyFactsSource`: the note's summaries as one plain text.
fn key_facts_source(summaries: &[(i64, String)]) -> Option<String> {
    static MARKS: LazyLock<Regex> = LazyLock::new(|| regex(r"[`*_~>#]"));
    static LIST: LazyLock<Regex> = LazyLock::new(|| regex(r"(^|\s)([-+]|[0-9]+[.)])\s+"));
    let mut summaries: Vec<&(i64, String)> = summaries
        .iter()
        .filter(|(_, body)| !body.trim().is_empty())
        .collect();
    summaries.sort_by_key(|(position, _)| *position);
    let joined = summaries
        .iter()
        .map(|(_, body)| plain_text(body))
        .collect::<Vec<_>>()
        .join("\n\n");
    let text = IMAGE.replace_all(&joined, "");
    let text = LINK.replace_all(&text, "$1");
    let text = MARKS.replace_all(&text, "");
    let text = LIST.replace_all(&text, " ");
    let text = spaces(&text, " ");
    let text = text.trim();
    (!text.is_empty()).then(|| truncate(text, MAX_SOURCE_LENGTH, "..."))
}

/// Shipping's `createSourceHash`: 32-bit FNV-1a over UTF-16 units, in hex.
fn source_hash(text: &str) -> String {
    let mut hash: u32 = 0x811c9dc5;
    for unit in text.encode_utf16() {
        hash ^= u32::from(unit);
        hash = hash.wrapping_mul(0x01000193);
    }
    format!("{hash:x}")
}

/// Shipping's `compactBriefText`.
fn compact(value: &str, max: usize) -> String {
    static TAG: LazyLock<Regex> = LazyLock::new(|| regex(r"<[^>]*>"));
    static URL: LazyLock<Regex> = LazyLock::new(|| regex(r"(?i)https?://\S+"));
    static LIST: LazyLock<Regex> = LazyLock::new(|| regex(r"(^|\s)([-*+]|\d+[.)])\s+"));
    static MARKS: LazyLock<Regex> = LazyLock::new(|| regex(r"[`_~>#]"));
    let text = plain_text(value);
    let text = TAG.replace_all(&text, " ");
    let text = IMAGE.replace_all(&text, " ");
    let text = LINK.replace_all(&text, "$1");
    let text = URL.replace_all(&text, " ");
    let text = LIST.replace_all(&text, " ");
    let text = MARKS.replace_all(&text, "");
    truncate(spaces(&text, " ").trim(), max, "…")
}

/// Shipping's `getPreMeetingBriefFacts`: saved key facts, else the summary itself.
fn facts(note: &PastNote) -> Vec<String> {
    let mut generated: Vec<String> = Vec::new();
    if let Some(summary) = &note.summary {
        for fact in summary
            .split('\n')
            .map(|line| compact(line, MAX_SUMMARY_LENGTH))
            .filter(|line| !line.is_empty())
            .take(MAX_FACTS)
        {
            if !generated.contains(&fact) {
                generated.push(fact);
            }
        }
    }
    if !generated.is_empty() {
        return generated;
    }
    let source = compact(&note.source_summary, MAX_SUMMARY_LENGTH);
    if source.is_empty() {
        Vec::new()
    } else {
        vec![source]
    }
}

/// Shipping's `selectBriefSourceNotes`.
fn source_notes(notes: &[PastNote]) -> Vec<&PastNote> {
    notes
        .iter()
        .filter(|note| !facts(note).is_empty())
        .take(MAX_BRIEF_MEETINGS)
        .collect()
}

/// Shipping's `shouldShowPreMeetingBrief`: before the start, then until the end (or five
/// minutes after a start without an end).
fn should_show(event: &BriefEvent, now: DateTime<Utc>) -> bool {
    if event.is_all_day {
        return false;
    }
    let Some(start) = parse_instant(&event.started_at) else {
        return false;
    };
    if start > now {
        return true;
    }
    let hide_after = parse_instant(&event.ended_at)
        .unwrap_or(start + chrono::Duration::milliseconds(AFTER_START_GRACE_MS));
    hide_after > now
}

fn participant_names(event: &BriefEvent) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for participant in event.participants.iter().filter(|p| !p.is_current_user) {
        let name = match participant.name.trim() {
            "" => participant.email.trim(),
            name => name,
        };
        if !name.is_empty() && !names.iter().any(|seen| seen == name) {
            names.push(name.to_owned());
        }
    }
    names.truncate(8);
    names
}

/// `pre-meeting-brief.system.md.jinja`.
fn system_prompt(language: &str) -> String {
    format!(
        "You write a 20-second pre-meeting brief from the notes in the user message only.

Write in {language}.

Fill opener with one sentence that names the single most important thing that should or will likely be discussed. Use a real fact from the notes. Fill bullets with exactly three supporting details: an open loop, a named commitment, or something to listen for.

Do not justify why the meeting exists. Everyone already knows they are meeting. Do not write that a sync is crucial, important, or needed for alignment.

Do not copy these instructions into opener or bullets. Do not write labels, headings, or placeholder lines. Do not repeat the opener as a bullet.

Rules:

- Three bullets total. Never a second list.
- Do not write a recap and then a preview. Each topic appears once.
- Do not restate the calendar title, time, location, or attendee list.
- Every name and fact must come from the notes. If it is not there, omit it."
    )
}

/// `pre-meeting-brief.user.md.jinja` with shipping's `getBriefPromptMeetings`.
fn user_prompt(event: &BriefEvent, notes: &[&PastNote]) -> String {
    let title = match event.title.trim() {
        "" => "Untitled",
        title => title,
    };
    let when = [event.started_at.as_str(), event.ended_at.as_str()]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" – ");
    let mut prompt = format!(
        "Write a new brief from these notes only. Cover at most three threads. Do not write a recap list and a separate upcoming list. Use the upcoming meeting only to know who and what this is — do not dump those fields into the brief.

# Upcoming meeting

Title: {title}
When: {}
Location: {}
Participants: {}
Description: {}

# Recent meetings

",
        compact(&when, 160),
        compact(&event.location, 120),
        participant_names(event).join(", "),
        compact(&event.description, 400),
    );
    let mut remaining = MAX_PROMPT_FACTS;
    for note in notes {
        if remaining == 0 {
            break;
        }
        let facts: Vec<String> = facts(note).into_iter().take(remaining).collect();
        remaining -= facts.len();
        if facts.is_empty() {
            continue;
        }
        prompt.push_str(&format!(
            "\n\n## {} ({})\n\nParticipants: {}\n\n{}\n\n",
            note.title,
            note.date_label,
            note.participant_names.join(", "),
            facts.join("\n")
        ));
    }
    prompt
}

/// Shipping's zod `briefSchema`.
fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "opener": {"type": "string"},
            "bullets": {"type": "array", "items": {"type": "string"}, "minItems": 1, "maxItems": MAX_BRIEF_BULLETS},
        },
        "required": ["opener", "bullets"],
        "additionalProperties": false,
    })
}

fn sanitize_opener(text: &str) -> String {
    let body = text.replace('*', "");
    let body = body.trim();
    if body.is_empty() || section_label(body) || instruction_leftover(body) {
        String::new()
    } else {
        body.to_owned()
    }
}

fn sanitize_bullet(text: &str) -> String {
    static MARKER: LazyLock<Regex> = LazyLock::new(|| regex(r"^(?:[-*+]|\d+[.)])\s+"));
    let item = MARKER.replace(text, "").replace('*', "");
    let item = item.trim();
    if item.is_empty() || instruction_leftover(item) {
        String::new()
    } else {
        item.to_owned()
    }
}

fn section_label(text: &str) -> bool {
    static LABEL: LazyLock<Regex> = LazyLock::new(|| {
        regex(
            r"(?i)^(quick\s+)?(recap|summary|overview|agenda|insights?|upcoming|next steps|prepare|brief)\b",
        )
    });
    let plain: String = text
        .chars()
        .filter(|c| !matches!(c, '#' | '*' | '_'))
        .collect();
    let plain = plain.trim();
    plain.is_empty()
        || (plain.encode_utf16().count() < 48 && plain.ends_with(':'))
        || LABEL.is_match(plain)
}

fn instruction_leftover(text: &str) -> bool {
    static LEFTOVER: LazyLock<Regex> = LazyLock::new(|| {
        regex(
            r"(?i)one sentence|why this conversation matters|would not immediately remember|open loop or commitment|thing to listen for or decide|this (meeting|sync|conversation) is (crucial|important|needed|essential)|aligning the team's vision",
        )
    });
    let plain: String = text
        .chars()
        .filter(|c| !matches!(c, '#' | '*' | '_'))
        .collect();
    LEFTOVER.is_match(plain.trim())
}

/// Shipping's `formatPreMeetingBrief` over a (partial) `{opener, bullets}` object.
fn format_brief(brief: &Value) -> String {
    let opener = sanitize_opener(brief["opener"].as_str().unwrap_or_default());
    let bullets: Vec<String> = brief["bullets"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|bullet| sanitize_bullet(bullet.as_str().unwrap_or_default()))
        .filter(|bullet| !bullet.is_empty())
        .take(MAX_BRIEF_BULLETS)
        .map(|bullet| format!("- {bullet}"))
        .collect();
    [
        (!opener.is_empty()).then(|| format!("**{opener}**")),
        (!bullets.is_empty()).then(|| bullets.join("\n")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("\n\n")
}

/// Shipping's `trimPreMeetingBrief`, for a reply that is Markdown instead of the object.
fn trim_brief(text: &str) -> String {
    static HEADING: LazyLock<Regex> = LazyLock::new(|| regex(r"^#{1,6}\s+(.+)$"));
    static BULLET: LazyLock<Regex> = LazyLock::new(|| regex(r"^(?:[-*+]|\d+[.)])\s+(.+)$"));
    let mut opener: Option<String> = None;
    let mut bullets: Vec<String> = Vec::new();
    for line in text
        .split('\n')
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if let Some(bullet) = BULLET.captures(line) {
            let item = sanitize_bullet(&bullet[1]);
            if !item.is_empty() && bullets.len() < MAX_BRIEF_BULLETS {
                bullets.push(format!("- {item}"));
            }
            continue;
        }
        if !bullets.is_empty() || opener.is_some() {
            continue;
        }
        let heading = HEADING.captures(line);
        let body = sanitize_opener(heading.as_ref().map_or(line, |heading| &heading[1]));
        if !body.is_empty() {
            opener = Some(format!("**{body}**"));
        }
    }
    [opener, (!bullets.is_empty()).then(|| bullets.join("\n"))]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// The object so far in a streamed JSON reply, like the AI SDK's partial output: open strings
/// and containers close, and a dangling key or comma is dropped.
fn partial_json(text: &str) -> Option<Value> {
    let mut text = text.trim_start();
    for _ in 0..8 {
        let mut stack = Vec::new();
        let (mut string, mut escape) = (false, false);
        let mut cut = None;
        for (index, c) in text.char_indices() {
            if string {
                match (escape, c) {
                    (true, _) => escape = false,
                    (false, '\\') => escape = true,
                    (false, '"') => string = false,
                    _ => {}
                }
                continue;
            }
            match c {
                '"' => string = true,
                '{' | '[' => {
                    stack.push(c);
                    cut = Some(index + 1);
                }
                '}' | ']' => {
                    stack.pop();
                }
                ',' => cut = Some(index),
                _ => {}
            }
        }
        let mut repaired = text.to_owned();
        if string {
            if escape {
                repaired.pop();
            }
            repaired.push('"');
        }
        for open in stack.iter().rev() {
            repaired.push(if *open == '{' { '}' } else { ']' });
        }
        if let Ok(value) = serde_json::from_str(&repaired) {
            return Some(value);
        }
        text = &text[..cut.filter(|cut| *cut < text.len())?];
    }
    None
}

/// The streamed reply as the memo's Markdown so far.
fn streamed_brief(text: &str) -> String {
    partial_json(text)
        .filter(Value::is_object)
        .map(|brief| format_brief(&brief))
        .unwrap_or_default()
}

/// The finished brief: the object when it matches the schema, else shipping's fallback that
/// keeps a Markdown reply with exactly three bullets.
fn final_brief(text: &str) -> Option<String> {
    let valid = serde_json::from_str::<Value>(text).ok().filter(|brief| {
        brief["opener"].is_string()
            && brief["bullets"].as_array().is_some_and(|bullets| {
                (1..=MAX_BRIEF_BULLETS).contains(&bullets.len())
                    && bullets.iter().all(Value::is_string)
            })
    });
    let brief = match valid {
        Some(brief) => format_brief(&brief),
        None => {
            let brief = trim_brief(text);
            if brief.matches("\n- ").count() + usize::from(brief.starts_with("- "))
                != MAX_BRIEF_BULLETS
            {
                return None;
            }
            brief
        }
    };
    let brief = brief.trim();
    (!brief.is_empty()).then(|| brief.to_owned())
}

/// Shipping's `streamPreMeetingBrief`: the brief's Markdown (`None` when the reply holds no
/// usable brief), with the reply's text passed to `observe` as it streams.
pub(super) async fn generate(
    ai: &AiServices,
    session: SessionId,
    language: String,
    event: &BriefEvent,
    notes: &[&PastNote],
    cancellation: CancellationToken,
    observe: StreamObserver,
) -> Result<Option<String>> {
    if notes.is_empty() {
        return Ok(None);
    }
    let reply = tokio::time::timeout(
        BRIEF_TIMEOUT,
        ai.generate(
            session,
            system_prompt(&language),
            user_prompt(event, notes),
            Some(Structured {
                schema: schema(),
                max_tokens: BRIEF_MAX_OUTPUT_TOKENS,
            }),
            cancellation,
            observe,
        ),
    )
    .await
    .map_err(|_| failure("Pre-meeting brief timed out."))??;
    Ok(final_brief(&reply))
}

fn markdown_body(markdown: &str) -> Option<String> {
    anlg_tiptap::md_to_tiptap_json(markdown)
        .ok()
        .map(|doc| doc.to_string())
}

impl NoteView {
    pub(super) fn load_brief(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.current.as_ref().map(|s| s.summary.id.clone()) else {
            return;
        };
        let runtime = self.runtime.clone();
        cx.spawn(async move |this, cx| {
            let brief = load(&runtime, session.clone()).await;
            let _ = this.update(cx, |this, cx| {
                if this.current.as_ref().map(|s| &s.summary.id) == Some(&session) {
                    this.brief = brief.ok().map(Arc::new);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(super) fn brief_generating(&self) -> bool {
        self.brief_job
            .as_ref()
            .is_some_and(|job| self.current.as_ref().map(|s| &s.summary.id) == Some(&job.session))
    }

    /// Shipping's `useCreatePreMeetingBrief` `available`.
    fn brief_available(&self, cx: &gpui::App) -> bool {
        let pro = crate::product::account::is_pro(cx);
        self.mode == SessionMode::Inactive
            && self.brief_job.is_none()
            && !self.session_id().is_some_and(|id| NoteHolds::held(cx, id))
            && self
                .model
                .is_some_and(|model| model.configured && !(model.hosted && !pro))
            && self
                .brief
                .as_ref()
                .is_some_and(|brief| brief.available(Utc::now()))
    }

    /// Shipping's `CreateBriefSuggestion`, above the memo's template suggestions.
    pub(super) fn brief_suggestion(
        &self,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.brief_available(cx) {
            return None;
        }
        Some(
            div()
                .id("create-brief")
                .a11y_button("Create a brief to prepare this meeting")
                .mb_6()
                .ml(px(-8.))
                .h_8()
                .max_w_full()
                .px_2()
                .flex()
                .items_center()
                .gap_2()
                .text_color(colors.muted_foreground)
                .cursor_pointer()
                .hover(move |style| style.bg(colors.accent).text_color(colors.foreground))
                .on_click(cx.listener(|this, _, _, cx| this.create_brief(cx)))
                .child(icon("editor/SparkleIcon.svg", 16., colors.muted_foreground))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_sm()
                        .line_height(px(20.))
                        .font_weight(crate::ui::theme::MEDIUM)
                        .child(SharedString::from("Create a brief to prepare this meeting")),
                )
                .into_any_element(),
        )
    }

    /// Cancels a streaming brief for any session in `ids` so it stops mutating a deleted or held memo.
    pub(super) fn cancel_brief(&mut self, ids: &[SessionId], cx: &mut Context<Self>) {
        if self
            .brief_job
            .as_ref()
            .is_some_and(|job| ids.contains(&job.session))
            && let Some(job) = self.brief_job.take()
        {
            job.cancellation.cancel();
            brief_editor(&job.editor, false, cx);
            cx.notify();
        }
    }

    /// Points a running brief at the session's memo editor once it is shown again.
    pub(super) fn adopt_brief_editor(
        &mut self,
        editor: &Entity<EditorPane>,
        cx: &mut Context<Self>,
    ) {
        let session = self.current.as_ref().map(|s| s.summary.id.clone());
        if let Some(id) = session.as_ref()
            && NoteHolds::held(cx, id)
        {
            self.cancel_brief(std::slice::from_ref(id), cx);
            return;
        }
        if let Some(job) = &mut self.brief_job
            && Some(&job.session) == session.as_ref()
            && &job.editor != editor
        {
            job.editor = editor.clone();
            brief_editor(editor, true, cx);
        }
    }

    /// Shipping's `createBrief` and `runPreMeetingBriefJob`.
    fn create_brief(&mut self, cx: &mut Context<Self>) {
        if !self.brief_available(cx) {
            return;
        }
        let (Some(session), Some(brief), Some(editor), Some(services)) = (
            self.current.as_ref().map(|s| s.summary.id.clone()),
            self.brief.clone(),
            self.memo_editor(),
            cx.try_global::<AiViewServices>().cloned(),
        ) else {
            return;
        };
        if NoteHolds::held(cx, &session) {
            return;
        }
        let notes: Vec<PastNote> = source_notes(&brief.notes).into_iter().cloned().collect();
        if notes.is_empty() {
            return;
        }
        let event = brief
            .event
            .clone()
            .unwrap_or_else(|| brief.fallback.clone());
        let cancellation = CancellationToken::new();
        let job_cancel = cancellation.clone();
        let job_session = session.clone();
        brief_editor(&editor, true, cx);
        self.brief_job = Some(BriefJob {
            session: session.clone(),
            editor,
            cancellation: cancellation.clone(),
        });
        cx.notify();
        let (send, mut deltas) = futures::channel::mpsc::unbounded::<String>();
        let observe: StreamObserver = Arc::new(move |update| {
            let _ = send.unbounded_send(update.text);
        });
        let runtime = self.runtime.clone();
        let ai = services.ai.clone();
        let reply = services.ai.enqueue(Box::pin(async move {
            let language = crate::meeting::config::preferences(&runtime)
                .await
                .map(|settings| settings.text("ai_language"))
                .unwrap_or_default();
            let language = if language.is_empty() {
                "en".into()
            } else {
                language
            };
            let notes: Vec<&PastNote> = notes.iter().collect();
            generate(
                &ai,
                session,
                language,
                &event,
                &notes,
                cancellation,
                observe,
            )
            .await
        }));
        cx.spawn(async move |this, cx| {
            let mut text = String::new();
            let mut shown = String::new();
            while let Some(delta) = deltas.next().await {
                if job_cancel.is_cancelled() {
                    break;
                }
                text.push_str(&delta);
                let markdown = streamed_brief(&text);
                if markdown.is_empty() || markdown == shown {
                    continue;
                }
                shown = markdown;
                if this
                    .update(cx, |this, cx| {
                        if !job_cancel.is_cancelled() {
                            this.show_brief(&shown, false, cx);
                        }
                    })
                    .is_err()
                {
                    return;
                }
            }
            let result = match reply {
                Ok(reply) => reply.await.map_err(failure).and_then(|result| result),
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                if job_cancel.is_cancelled() {
                    return;
                }
                match result {
                    Ok(Some(markdown)) => this.show_brief(&markdown, true, cx),
                    Err(ServiceError::Cancelled) => {}
                    Ok(None) | Err(_) => toasts::show(
                        toasts::Toast::error(ERROR).id("pre-meeting-brief-error"),
                        cx,
                    ),
                }
                if this
                    .brief_job
                    .as_ref()
                    .is_some_and(|job| job.session == job_session)
                    && let Some(job) = this.brief_job.take()
                {
                    brief_editor(&job.editor, false, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Shipping's `applyBriefToEditor`: the brief replaces the memo; the last one saves now.
    fn show_brief(&mut self, markdown: &str, flush: bool, cx: &mut Context<Self>) {
        let (Some(job), Some(body)) = (&self.brief_job, markdown_body(markdown)) else {
            return;
        };
        if job.cancellation.is_cancelled() || NoteHolds::held(cx, &job.session) {
            return;
        }
        job.editor.update(cx, |editor, cx| {
            editor.replace_content(&body, cx);
            if flush {
                drop(editor.flush());
            }
        });
    }
}

/// While generating, the memo is read-only and says "Creating brief..." (shipping's
/// `readOnly={isGenerating}` and placeholder).
fn brief_editor(editor: &Entity<EditorPane>, generating: bool, cx: &mut gpui::App) {
    editor.update(cx, |editor, cx| {
        editor.set_placeholder(Some(
            if generating {
                "Creating brief..."
            } else {
                "Start writing..."
            }
            .into(),
        ));
        editor.set_read_only(generating, cx);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streamed_json_reads_as_a_growing_brief_and_markdown_replies_need_three_bullets() {
        let reply = r#"{"opener": "Confirm the **pricing** decision", "bullets": ["- Ana owns the SKU list", "Legal review is open", "Listen for budget"]}"#;
        let shown: Vec<String> = (1..=reply.len())
            .filter(|end| reply.is_char_boundary(*end))
            .map(|end| streamed_brief(&reply[..end]))
            .collect();
        assert!(shown.contains(&"**Confirm the pri**".to_owned()));
        assert!(
            shown.contains(
                &"**Confirm the pricing decision**\n\n- Ana owns the SKU list".to_owned()
            )
        );
        let brief = "**Confirm the pricing decision**\n\n- Ana owns the SKU list\n- Legal review is open\n- Listen for budget";
        assert_eq!(shown.last().unwrap(), brief);
        assert_eq!(final_brief(reply).unwrap(), brief);
        // Shipping's fallback keeps a Markdown reply only with exactly three bullets.
        let markdown = "## Recap:\nConfirm the pricing decision\n- Ana owns the SKU list\n- Legal review is open\n- Listen for budget";
        assert_eq!(final_brief(markdown).unwrap(), brief);
        assert_eq!(final_brief("Confirm pricing\n- One\n- Two"), None);
    }

    struct Fake(Arc<std::sync::Mutex<Option<(String, bool)>>>);

    impl crate::meeting::ai::ProviderAdapter for Fake {
        fn stream(
            &self,
            request: crate::meeting::ai::Request,
        ) -> futures::future::BoxFuture<
            'static,
            Result<futures::stream::BoxStream<'static, Result<crate::meeting::ai::ProviderEvent>>>,
        > {
            use crate::meeting::ai::{Part, ProviderEvent, Role};
            let prompt = request
                .messages
                .iter()
                .filter(|message| message.role == Role::User)
                .flat_map(|message| &message.parts)
                .filter_map(|part| match part {
                    Part::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect();
            *self.0.lock().unwrap() = Some((prompt, request.structured.is_some()));
            Box::pin(async move {
                let reply = r#"{"opener": "Confirm who signs off on pricing", "bullets": ["Ana owns the SKU list", "Legal review is open", "Listen for the budget owner"]}"#;
                let chunks: Vec<Result<ProviderEvent>> = reply
                    .as_bytes()
                    .chunks(9)
                    .map(|chunk| {
                        Ok(ProviderEvent::Text(
                            String::from_utf8(chunk.to_vec()).unwrap(),
                        ))
                    })
                    .collect();
                Ok(futures::stream::iter(chunks).boxed())
            })
        }
    }

    #[tokio::test]
    async fn brief_streams_from_related_past_notes_and_their_current_key_facts() {
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(desktop_runtime::Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        ready.receive().await.unwrap();
        let sql = |sql: &'static str, params: Vec<Value>| {
            let runtime = runtime.clone();
            async move {
                runtime
                    .submit(move |services| async move {
                        services
                            .executor
                            .execute(sql.into(), params)
                            .await
                            .map_err(failure)
                    })
                    .unwrap()
                    .receive()
                    .await
                    .unwrap()
            }
        };
        sql(r#"INSERT INTO sessions(id,title,owner_user_id,created_at,event_json) VALUES
            ('next','Pricing sync','me','2026-11-01T09:00:00Z','{"started_at":"2099-01-01T09:00:00Z"}'),
            ('past','Pricing sync','me','2026-03-10T12:00:00Z','{}'),
            ('older','Kickoff','me','2026-02-01T12:00:00Z','{}'),
            ('later','Pricing sync','me','2099-02-01T12:00:00Z','{}')"#, vec![]).await;
        sql(
            "INSERT INTO humans(id,name) VALUES ('ana','Ana Lima'),('me','Me')",
            vec![],
        )
        .await;
        sql("INSERT INTO session_participants(id,session_id,human_id,owner_user_id,source) VALUES
            ('p1','next','ana','me','auto'),('p2','past','ana','me','auto'),('p3','older','ana','me','auto'),
            ('p4','next','me','me','auto'),('p5','later','ana','me','auto')", vec![]).await;
        sql("INSERT INTO session_documents(id,session_id,kind,body,body_format,source_hash) VALUES
            ('d1','past','summary',?,'markdown',''),('d2','older','summary','Kickoff recap: scope agreed','markdown',''),
            ('d3','later','summary','Future plans','markdown',''),
            ('k1','past','key_facts',?,'markdown','fddbda5e'),('k2','older','key_facts','Stale fact','markdown','0')",
            vec![json!("## Decisions\n- Ana owns the **SKU** list\n- Legal review is open"), json!("Ana owns the SKU list\nLegal review closes Friday")]).await;
        let brief = load(&runtime, SessionId("next".into())).await.unwrap();
        assert!(brief.event.is_none() && brief.available(Utc::now()));
        let seen = Arc::new(std::sync::Mutex::new(None));
        let provider: Arc<dyn crate::meeting::ai::ProviderAdapter> = Arc::new(Fake(seen.clone()));
        let ai = AiServices::new(
            runtime.clone(),
            Arc::new(move |_| {
                let provider = provider.clone();
                Box::pin(async move { Ok(provider) })
            }),
            Arc::new(|_, _, _| Box::pin(async { Ok(json!({})) })),
            Arc::new(|_| false),
        )
        .unwrap();
        let streamed = Arc::new(std::sync::Mutex::new(String::new()));
        let observed = streamed.clone();
        let notes = source_notes(&brief.notes);
        let markdown = generate(
            &ai,
            SessionId("next".into()),
            "en".into(),
            &brief.fallback,
            &notes,
            CancellationToken::new(),
            Arc::new(move |update| observed.lock().unwrap().push_str(&update.text)),
        )
        .await
        .unwrap()
        .unwrap();
        let expected = "**Confirm who signs off on pricing**\n\n- Ana owns the SKU list\n- Legal review is open\n- Listen for the budget owner";
        assert_eq!(markdown, expected);
        assert_eq!(streamed_brief(&streamed.lock().unwrap()), expected);
        let (prompt, structured) = seen.lock().unwrap().clone().unwrap();
        assert!(structured);
        // The matching-title note leads with its key facts, whose hash shipping computed; the
        // stale facts fall back to the summary, and the later note is left out.
        assert!(prompt.contains("## Pricing sync (Mar 10, 2026)\n\nParticipants: Ana Lima\n\nAna owns the SKU list\nLegal review closes Friday\n\n"));
        assert!(prompt.contains(
            "## Kickoff (Feb 1, 2026)\n\nParticipants: Ana Lima\n\nKickoff recap: scope agreed"
        ));
        assert!(!prompt.contains("Stale fact") && !prompt.contains("Future plans"));
        assert!(
            prompt.contains("Title: Pricing sync\nWhen: \nLocation: \nParticipants: Ana Lima\n")
        );
        runtime.shutdown().await.unwrap();
    }

    #[gpui::test]
    fn a_held_note_cancels_its_streaming_brief_and_blocks_writes(cx: &mut gpui::TestAppContext) {
        use std::sync::Mutex;

        use futures::executor::block_on;

        use crate::contracts::{EditorInit, LaneContext};
        use crate::meeting::ai::{ProviderAdapter, ProviderEvent, Request};

        struct PausingProvider {
            delta: Mutex<Option<futures::channel::mpsc::UnboundedSender<Result<ProviderEvent>>>>,
        }

        impl ProviderAdapter for PausingProvider {
            fn stream(
                &self,
                _request: Request,
            ) -> futures::future::BoxFuture<
                'static,
                Result<futures::stream::BoxStream<'static, Result<ProviderEvent>>>,
            > {
                let (tx, rx) = futures::channel::mpsc::unbounded();
                let _ = tx.unbounded_send(Ok(ProviderEvent::Text(
                    r#"{"opener": "Confirm pricing"#.to_owned(),
                )));
                *self.delta.lock().unwrap() = Some(tx);
                Box::pin(async move { Ok(rx.boxed()) })
            }
        }

        cx.executor().allow_parking();
        cx.update(crate::ui::theme::UiFonts::init);
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(desktop_runtime::Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        block_on(ready.receive()).unwrap();
        let open = block_on(
            runtime
                .create_note("Pricing sync".into())
                .unwrap()
                .receive(),
        )
        .unwrap();
        let session = open.summary.id.clone();

        let provider = Arc::new(PausingProvider {
            delta: Mutex::new(None),
        });
        let provider_handle = provider.clone();
        let approvals = crate::meeting::tools::Approvals::default();
        let ai_services = AiViewServices {
            ai: Arc::new(
                AiServices::new(
                    runtime.clone(),
                    Arc::new(move |_| {
                        let provider = provider_handle.clone();
                        Box::pin(async move { Ok(provider as Arc<dyn ProviderAdapter>) })
                    }),
                    crate::meeting::tools::executor(runtime.clone(), approvals.clone(), None),
                    Arc::new(|_| false),
                )
                .unwrap(),
            ),
            context: crate::meeting::context::resolver(runtime.clone()),
            approvals,
        };
        cx.update(|cx| cx.set_global(ai_services));

        let (note, cx) =
            cx.add_window_view(|window, cx| NoteView::new(runtime.clone(), window, cx));
        let init = EditorInit {
            session_id: session.clone(),
            document: open.note.clone().unwrap(),
            return_focus: cx.update(|_, cx| cx.focus_handle()),
        };
        let runtime_clone = runtime.clone();
        let memo = cx.update(|window, cx| {
            cx.new(|cx| {
                EditorPane::new(
                    LaneContext {
                        runtime: runtime_clone,
                    },
                    init,
                    window,
                    cx,
                )
            })
        });
        for _ in 0..400 {
            cx.run_until_parked();
            if memo.read_with(cx, |editor, _| editor.is_pristine()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }

        let upcoming = Arc::new(Brief {
            event: None,
            fallback: BriefEvent {
                title: "Pricing sync".into(),
                started_at: String::new(),
                ended_at: String::new(),
                is_all_day: false,
                location: String::new(),
                description: String::new(),
                participants: vec![EventParticipant {
                    name: "Ana Lima".into(),
                    email: String::new(),
                    is_current_user: false,
                }],
            },
            has_participants: true,
            notes: vec![PastNote {
                title: "Pricing sync".into(),
                date_label: "Mar 10, 2026".into(),
                participant_names: vec!["Ana Lima".into()],
                source_summary: "Key fact".into(),
                relationship: Relationship::MatchingTitle,
                summary: Some("Key fact".into()),
                date_ms: 1,
            }],
        });

        note.update(cx, |note, cx| {
            note.show(open, cx);
            note.set_content(&session, memo.clone().into(), cx);
            note.model = Some(crate::workspace::open_note::summary::ModelState {
                configured: true,
                hosted: false,
                local: false,
            });
            note.brief = Some(upcoming);
            note.create_brief(cx);
        });
        assert!(note.read_with(cx, |note, _| note.brief_generating()));

        // Holding the note (as during deletion) must cancel the job and block any brief writes.
        let hold = cx.update(|_, cx| NoteHolds::hold(vec![session.clone()], cx));
        for _ in 0..400 {
            cx.run_until_parked();
            if note.read_with(cx, |note, _| !note.brief_generating()) {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        note.read_with(cx, |note, cx| {
            assert!(!note.brief_generating(), "held note must cancel brief_job");
            assert!(
                !note.brief_available(cx),
                "brief must stay unavailable while held"
            );
        });
        if let Some(tx) = provider.delta.lock().unwrap().take() {
            let _ = tx.unbounded_send(Ok(ProviderEvent::Text(
                r#", "bullets": ["One", "Two", "Three"]}"#.to_owned(),
            )));
            drop(tx);
        }
        note.update(cx, |note, cx| {
            note.show_brief("**Should not apply**\n\n- One\n- Two\n- Three", true, cx);
        });
        cx.run_until_parked();
        assert!(!memo.read_with(cx, |editor, _| editor.is_dirty()));

        cx.update(|_, cx| hold.release(cx));
        drop((memo, note));
        block_on(runtime.shutdown()).unwrap();
    }
}
