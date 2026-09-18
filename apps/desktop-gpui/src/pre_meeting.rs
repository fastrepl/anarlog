//! `session/insights/pre-meeting.ts` and `past-notes.ts`: the pre-meeting
//! brief the empty memo offers before an upcoming meeting, built from the
//! summaries of earlier meetings with the same people.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use regex::Regex;
use serde::Deserialize;
use serde_json::{Value, json};

pub const MAX_BRIEF_MEETINGS: usize = 5;
const AFTER_START_GRACE_MS: i64 = 5 * 60 * 1000;
const MAX_FACTS: usize = 3;
const MAX_PROMPT_FACTS: usize = 4;
const MAX_BRIEF_BULLETS: usize = 3;
const MAX_SUMMARY_LENGTH: usize = 320;
pub const BRIEF_MAX_OUTPUT_TOKENS: u32 = 512;
pub const BRIEF_GENERATION_TIMEOUT_SECS: u64 = 45;
const MAX_PAST_NOTES: usize = 8;
const MAX_SOURCE_LENGTH: usize = 6000;

const SYSTEM_TEMPLATE: &str =
    include_str!("../../desktop/src/session/insights/pre-meeting-brief.system.md.jinja");
const USER_TEMPLATE: &str =
    include_str!("../../desktop/src/session/insights/pre-meeting-brief.user.md.jinja");

/// `briefSchema`: `Output.object`'s JSON Schema.
pub fn brief_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "opener": { "type": "string" },
            "bullets": {
                "type": "array",
                "items": { "type": "string" },
                "minItems": 1,
                "maxItems": MAX_BRIEF_BULLETS
            }
        },
        "required": ["opener", "bullets"],
        "additionalProperties": false
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BriefEvent {
    pub title: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub is_all_day: bool,
    pub location: Option<String>,
    pub description: Option<String>,
    pub participants: Vec<BriefParticipant>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BriefParticipant {
    pub name: Option<String>,
    pub email: Option<String>,
    pub is_current_user: bool,
    pub is_organizer: bool,
}

/// What `useCreatePreMeetingBrief` reads for one session.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BriefInputs {
    pub event: Option<BriefEvent>,
    pub notes: Vec<PastSessionNote>,
    /// `hasParticipants`: people other than the owner are attached.
    pub has_participants: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Relationship {
    SharedParticipants = 0,
    MatchingTitle = 1,
    SameSeries = 2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PastSessionNote {
    pub session_id: String,
    pub title: String,
    pub date_label: String,
    pub participant_names: Vec<String>,
    pub source_summary: String,
    pub relationship: Relationship,
    /// Saved `key_facts` for the same source, when one exists.
    pub summary: Option<String>,
}

/// `shouldShowPreMeetingBrief`: an upcoming timed event, or one still
/// running (until its end; five minutes past the start without one, #7478).
pub fn should_show_pre_meeting_brief(event: Option<&BriefEvent>, now_ms: i64) -> bool {
    let Some(event) = event else {
        return false;
    };
    if event.is_all_day {
        return false;
    }
    let Some(start_ms) = event
        .started_at
        .as_deref()
        .and_then(crate::scheduled_auto_start::parse_event_instant)
        .map(|instant| instant.timestamp_millis())
    else {
        return false;
    };
    if start_ms > now_ms {
        return true;
    }
    let end_ms = event
        .ended_at
        .as_deref()
        .and_then(crate::scheduled_auto_start::parse_event_instant)
        .map(|instant| instant.timestamp_millis());
    let hide_after_ms = end_ms.unwrap_or(start_ms + AFTER_START_GRACE_MS);
    hide_after_ms > now_ms
}

/// `selectBriefSourceNotes`
pub fn select_brief_source_notes(notes: &[PastSessionNote]) -> Vec<&PastSessionNote> {
    notes
        .iter()
        .filter(|note| !pre_meeting_brief_facts(note).is_empty())
        .take(MAX_BRIEF_MEETINGS)
        .collect()
}

/// `canCreatePreMeetingBrief`: with an event, its window decides; without
/// one, added participants do (#7478 — an ended meeting stays ineligible).
pub fn can_create_pre_meeting_brief(
    event: Option<&BriefEvent>,
    now_ms: i64,
    notes: &[PastSessionNote],
    has_participants: bool,
) -> bool {
    let eligible = match event {
        Some(event) => should_show_pre_meeting_brief(Some(event), now_ms),
        None => has_participants,
    };
    eligible && !select_brief_source_notes(notes).is_empty()
}

/// `getBriefEventParticipantNames`
pub fn brief_event_participant_names(event: Option<&BriefEvent>) -> Vec<String> {
    let mut seen = HashSet::new();
    event
        .map(|event| event.participants.as_slice())
        .unwrap_or(&[])
        .iter()
        .filter(|participant| !participant.is_current_user)
        .filter_map(|participant| {
            let name = participant.name.as_deref().map(str::trim).unwrap_or("");
            let email = participant.email.as_deref().map(str::trim).unwrap_or("");
            let label = if !name.is_empty() { name } else { email };
            (!label.is_empty() && seen.insert(label.to_string())).then(|| label.to_string())
        })
        .take(8)
        .collect()
}

/// `getPreMeetingBriefFacts`: the saved key facts, else the compacted source
/// summary.
pub fn pre_meeting_brief_facts(note: &PastSessionNote) -> Vec<String> {
    if let Some(summary) = &note.summary {
        let mut seen = HashSet::new();
        let facts: Vec<String> = summary
            .split('\n')
            .map(|line| compact_brief_text(line, MAX_SUMMARY_LENGTH))
            .filter(|fact| !fact.is_empty())
            .take(MAX_FACTS)
            .filter(|fact| seen.insert(fact.clone()))
            .collect();
        if !facts.is_empty() {
            return facts;
        }
    }
    let source = compact_brief_text(&note.source_summary, MAX_SUMMARY_LENGTH);
    if source.is_empty() {
        Vec::new()
    } else {
        vec![source]
    }
}

/// `compactBriefText`
pub fn compact_brief_text(value: &str, max_length: usize) -> String {
    static TAGS: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"<[^>]*>").unwrap());
    static IMAGES: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"!\[[^\]]*\]\([^)]+\)").unwrap());
    static LINKS: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\([^)]+\)").unwrap());
    static URLS: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"(?i)https?://\S+").unwrap());
    static LIST_MARKERS: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"(^|\s)([-*+]|\d+[.)])\s+").unwrap());
    static MARKS: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"[`_~>#]").unwrap());
    static SPACES: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"\s+").unwrap());

    let text = crate::contact_summary::extract_plain_text(value);
    let text = TAGS.replace_all(&text, " ");
    let text = IMAGES.replace_all(&text, " ");
    let text = LINKS.replace_all(&text, "$1");
    let text = URLS.replace_all(&text, " ");
    let text = LIST_MARKERS.replace_all(&text, " ");
    let text = MARKS.replace_all(&text, "");
    let text = SPACES.replace_all(&text, " ").trim().to_string();
    truncate_at_word(&text, max_length, "…")
}

/// JavaScript's `slice` counts UTF-16 units; the texts here are plain prose,
/// so characters are close enough and never split a code point.
fn truncate_at_word(text: &str, max_length: usize, ellipsis: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_length {
        return text.to_string();
    }
    let slice: String = chars[..=max_length].iter().collect();
    let last_space = slice.rfind(' ').map(|byte| slice[..byte].chars().count());
    let end = match last_space {
        Some(index) if (index as f64) > max_length as f64 * 0.6 => index,
        _ => max_length,
    };
    let head: String = chars[..end].iter().collect();
    format!("{}{ellipsis}", head.trim())
}

/// `formatPreMeetingBrief`: `**opener**`, a blank line, then `- ` bullets.
pub fn format_pre_meeting_brief(opener: Option<&str>, bullets: &[String]) -> String {
    let opener = sanitize_brief_opener(opener);
    let bullets: Vec<String> = bullets
        .iter()
        .map(|bullet| sanitize_brief_bullet(bullet))
        .filter(|bullet| !bullet.is_empty())
        .take(MAX_BRIEF_BULLETS)
        .map(|bullet| format!("- {bullet}"))
        .collect();
    let mut parts = Vec::new();
    if !opener.is_empty() {
        parts.push(format!("**{opener}**"));
    }
    if !bullets.is_empty() {
        parts.push(bullets.join("\n"));
    }
    parts.join("\n\n")
}

/// `trimPreMeetingBrief`: salvage an opener and up to three bullets from a
/// reply that did not parse as the object.
pub fn trim_pre_meeting_brief(text: &str) -> String {
    static HEADING: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"^#{1,6}\s+(.+)$").unwrap());
    static BULLET: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"^(?:[-*+]|\d+[.)])\s+(.+)$").unwrap());
    let mut opener: Option<String> = None;
    let mut bullets: Vec<String> = Vec::new();
    for raw_line in text.split('\n') {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(bullet) = BULLET.captures(line) {
            let item = sanitize_brief_bullet(&bullet[1]);
            if !item.is_empty() && bullets.len() < MAX_BRIEF_BULLETS {
                bullets.push(format!("- {item}"));
            }
            continue;
        }
        if !bullets.is_empty() || opener.is_some() {
            continue;
        }
        let body = HEADING
            .captures(line)
            .map(|heading| heading[1].to_string())
            .unwrap_or_else(|| line.to_string());
        let body = sanitize_brief_opener(Some(&body));
        if !body.is_empty() {
            opener = Some(format!("**{body}**"));
        }
    }
    let mut parts = Vec::new();
    if let Some(opener) = opener {
        parts.push(opener);
    }
    if !bullets.is_empty() {
        parts.push(bullets.join("\n"));
    }
    parts.join("\n\n")
}

fn sanitize_brief_opener(text: Option<&str>) -> String {
    let body = text.unwrap_or("").replace('*', "").trim().to_string();
    if body.is_empty() || is_brief_section_label(&body) || is_brief_instruction_leftover(&body) {
        return String::new();
    }
    body
}

fn sanitize_brief_bullet(text: &str) -> String {
    static MARKER: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"^(?:[-*+]|\d+[.)])\s+").unwrap());
    let item = MARKER.replace(text, "").replace('*', "").trim().to_string();
    if item.is_empty() || is_brief_instruction_leftover(&item) {
        return String::new();
    }
    item
}

fn strip_marks(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '#' | '*' | '_'))
        .collect::<String>()
        .trim()
        .to_string()
}

fn is_brief_section_label(text: &str) -> bool {
    static SECTION_LABEL: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
        Regex::new(r"(?i)^(quick\s+)?(recap|summary|overview|agenda|insights?|upcoming|next steps|prepare|brief)\b").unwrap()
    });
    let plain = strip_marks(text);
    if plain.is_empty() {
        return true;
    }
    if plain.chars().count() < 48 && plain.ends_with(':') {
        return true;
    }
    SECTION_LABEL.is_match(&plain)
}

fn is_brief_instruction_leftover(text: &str) -> bool {
    static LEFTOVER: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
        Regex::new(r"(?i)one sentence|why this conversation matters|would not immediately remember|open loop or commitment|thing to listen for or decide|this (meeting|sync|conversation) is (crucial|important|needed|essential)|aligning the team's vision").unwrap()
    });
    LEFTOVER.is_match(&strip_marks(text))
}

/// `getBriefPromptMeetings`: at most four facts across the source notes.
fn brief_prompt_meetings(notes: &[&PastSessionNote]) -> Vec<Value> {
    let mut remaining = MAX_PROMPT_FACTS;
    let mut meetings = Vec::new();
    for note in notes {
        if remaining == 0 {
            break;
        }
        let facts: Vec<String> = pre_meeting_brief_facts(note)
            .into_iter()
            .take(remaining)
            .collect();
        remaining -= facts.len();
        if facts.is_empty() {
            continue;
        }
        meetings.push(json!({
            "title": note.title,
            "date": note.date_label,
            "participants": note.participant_names,
            "notes": facts.join("\n"),
        }));
    }
    meetings
}

/// `mergeBriefMarkdown`
pub fn merge_brief_markdown(brief: &str, existing: &str) -> String {
    let brief = brief.trim();
    let existing = existing.trim();
    if brief.is_empty() {
        return existing.to_string();
    }
    if existing.is_empty() {
        return brief.to_string();
    }
    format!("{brief}\n\n{existing}")
}

/// The system and user prompts of `streamPreMeetingBrief`.
pub fn brief_prompts(
    language: &str,
    event: &BriefEvent,
    notes: &[PastSessionNote],
) -> Result<(String, String), String> {
    let source_notes = select_brief_source_notes(notes);
    let title = event.title.as_deref().map(str::trim).unwrap_or("");
    let when = [event.started_at.as_deref(), event.ended_at.as_deref()]
        .into_iter()
        .flatten()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" – ");
    let system = render_jinja(SYSTEM_TEMPLATE, &json!({ "language": language }))?;
    let user = render_jinja(
        USER_TEMPLATE,
        &json!({
            "meeting": {
                "title": if title.is_empty() { "Untitled" } else { title },
                "when": compact_brief_text(&when, 160),
                "location": compact_brief_text(event.location.as_deref().unwrap_or(""), 120),
                "participants": brief_event_participant_names(Some(event)),
                "description": compact_brief_text(event.description.as_deref().unwrap_or(""), 400),
            },
            "past_meetings": brief_prompt_meetings(&source_notes),
        }),
    )?;
    Ok((system, user))
}

/// `templateCommands.renderCustom`: the plugin trims the rendered text.
fn render_jinja(template: &str, ctx: &Value) -> Result<String, String> {
    let Value::Object(ctx) = ctx else {
        return Err("template context must be an object".to_string());
    };
    anlg_template_app_legacy::render_custom(template, ctx)
        .map(|rendered| rendered.trim().to_string())
        .map_err(|error| error.to_string())
}

/// The brief a (possibly partial) model reply describes: `parsePartialJson`
/// closes what the stream has produced so far, then only complete strings
/// count.
pub fn brief_from_partial_json(text: &str) -> Option<(Option<String>, Vec<String>)> {
    let value: Value = serde_json::from_str(text.trim())
        .or_else(|_| serde_json::from_str(&complete_partial_json(text.trim())))
        .ok()?;
    let object = value.as_object()?;
    let opener = object
        .get("opener")
        .and_then(Value::as_str)
        .map(str::to_string);
    let bullets = object
        .get("bullets")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    Some((opener, bullets))
}

/// Close the open strings, arrays, and objects of a JSON prefix (the shape
/// `parsePartialJson` repairs): a value string still being written is kept
/// and closed, an unfinished key or dangling `key:` / comma is dropped, then
/// every open container is closed.
pub fn complete_partial_json(text: &str) -> String {
    #[derive(Clone, Copy, PartialEq)]
    enum Ctx {
        /// Right after `{` or `,`: a key (or `}`) comes next.
        ObjectKey,
        /// After a key: `:` comes next.
        ObjectColon,
        /// After `:`: a value comes next.
        ObjectValue,
        /// After a value: `,` or `}` comes next.
        ObjectAfter,
        /// Right after `[` or `,`: a value (or `]`) comes next.
        ArrayValue,
        ArrayAfter,
    }
    let mut stack: Vec<Ctx> = Vec::new();
    let mut in_string = false;
    let mut string_is_key = false;
    let mut escaped = false;
    let mut literal_start: Option<usize> = None;
    // The end of the longest prefix that only needs closers appended.
    let mut last_good = 0;

    let finish_value = |stack: &mut Vec<Ctx>| {
        if let Some(top) = stack.last_mut() {
            *top = match *top {
                Ctx::ObjectValue => Ctx::ObjectAfter,
                Ctx::ArrayValue => Ctx::ArrayAfter,
                other => other,
            };
        }
    };
    let literal_complete = |literal: &str| {
        matches!(literal, "true" | "false" | "null")
            || literal.chars().last().is_some_and(|c| c.is_ascii_digit())
    };

    for (index, ch) in text.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
                if string_is_key {
                    if let Some(top) = stack.last_mut() {
                        *top = Ctx::ObjectColon;
                    }
                } else {
                    finish_value(&mut stack);
                    last_good = index + ch.len_utf8();
                }
            }
            continue;
        }
        if let Some(start) = literal_start
            && !ch.is_ascii_alphanumeric()
            && !matches!(ch, '.' | '-' | '+')
        {
            if literal_complete(&text[start..index]) {
                finish_value(&mut stack);
                last_good = index;
            }
            literal_start = None;
        }
        match ch {
            c if c.is_whitespace() => {}
            '"' => {
                in_string = true;
                string_is_key = stack.last() == Some(&Ctx::ObjectKey);
            }
            '{' | '[' => {
                stack.push(if ch == '{' {
                    Ctx::ObjectKey
                } else {
                    Ctx::ArrayValue
                });
                last_good = index + 1;
            }
            '}' | ']' => {
                stack.pop();
                finish_value(&mut stack);
                last_good = index + 1;
            }
            ',' => {
                if let Some(top) = stack.last_mut() {
                    *top = match *top {
                        Ctx::ObjectAfter | Ctx::ObjectKey => Ctx::ObjectKey,
                        _ => Ctx::ArrayValue,
                    };
                }
            }
            ':' => {
                if let Some(top) = stack.last_mut() {
                    *top = Ctx::ObjectValue;
                }
            }
            _ => {
                if literal_start.is_none() {
                    literal_start = Some(index);
                }
            }
        }
    }

    let mut out = if in_string && !string_is_key {
        finish_value(&mut stack);
        format!("{text}\"")
    } else {
        if let Some(start) = literal_start
            && literal_complete(&text[start..])
        {
            finish_value(&mut stack);
            last_good = text.len();
        }
        text[..last_good].to_string()
    };
    // The context the kept prefix ends in must not expect a value: a cut
    // right after `{` or `[` (or a comma, which the cut removed) is fine.
    let trimmed_len = out.trim_end().trim_end_matches(',').len();
    out.truncate(trimmed_len);
    while let Some(ctx) = stack.pop() {
        out.push(match ctx {
            Ctx::ObjectKey | Ctx::ObjectColon | Ctx::ObjectValue | Ctx::ObjectAfter => '}',
            Ctx::ArrayValue | Ctx::ArrayAfter => ']',
        });
    }
    out
}

/// One row of `sessions` for `buildPastSessionNotes`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PastSessionRow {
    pub id: String,
    pub user_id: String,
    pub title: String,
    pub created_at: String,
    pub event_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PastParticipantRow {
    pub session_id: String,
    pub human_id: String,
    pub user_id: String,
    pub source: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PastEnhancedNoteRow {
    pub session_id: String,
    pub content: String,
    pub position: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PastKeyFactsRow {
    pub session_id: String,
    pub content: String,
    pub source_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PastSessionNotesData {
    pub sessions: Vec<PastSessionRow>,
    pub participants: Vec<PastParticipantRow>,
    pub enhanced_notes: Vec<PastEnhancedNoteRow>,
    pub key_facts: Vec<PastKeyFactsRow>,
}

#[derive(Debug, Default, Deserialize)]
struct EventJson {
    #[serde(default)]
    started_at: Option<String>,
    #[serde(default)]
    recurrence_series_id: Option<String>,
}

fn session_event(session: &PastSessionRow) -> Option<EventJson> {
    if session.event_json.trim().is_empty() {
        return None;
    }
    serde_json::from_str(&session.event_json).ok()
}

/// `buildPastSessionNotes`: earlier sessions in the same series, with the
/// same title and people, or with shared participants — newest first within
/// each relationship rank, at most eight.
pub fn build_past_session_notes(
    data: &PastSessionNotesData,
    session_id: &str,
    user_id: Option<&str>,
) -> Vec<PastSessionNote> {
    let Some(current) = data
        .sessions
        .iter()
        .find(|session| session.id == session_id)
    else {
        return Vec::new();
    };
    let mut participants_by_session: HashMap<&str, Vec<&PastParticipantRow>> = HashMap::new();
    for participant in &data.participants {
        participants_by_session
            .entry(participant.session_id.as_str())
            .or_default()
            .push(participant);
    }
    let mut notes_by_session: HashMap<&str, Vec<&PastEnhancedNoteRow>> = HashMap::new();
    for note in &data.enhanced_notes {
        notes_by_session
            .entry(note.session_id.as_str())
            .or_default()
            .push(note);
    }
    let mut names_by_human: HashMap<&str, &str> = HashMap::new();
    for participant in &data.participants {
        let name = participant.name.trim();
        if !name.is_empty() {
            names_by_human.insert(participant.human_id.as_str(), name);
        }
    }
    let participant_ids_for = |id: &str| -> HashSet<String> {
        session_participant_ids(
            participants_by_session
                .get(id)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            user_id,
        )
    };

    let current_participant_ids = participant_ids_for(session_id);
    let current_event = session_event(current);
    let current_series_id = recurrence_series_id(current_event.as_ref());
    let current_title_key = session_title_key(current);
    if current_series_id.is_none()
        && current_title_key.is_empty()
        && current_participant_ids.is_empty()
    {
        return Vec::new();
    }
    let current_timestamp = session_timestamp(current);

    struct Item {
        note: PastSessionNote,
        date_ms: i64,
    }
    let mut items: Vec<Item> = Vec::new();
    for candidate in &data.sessions {
        if candidate.id == session_id {
            continue;
        }
        let candidate_timestamp = session_timestamp(candidate);
        if current_timestamp > 0
            && candidate_timestamp > 0
            && candidate_timestamp >= current_timestamp
        {
            continue;
        }
        let candidate_event = session_event(candidate);
        let candidate_participant_ids = participant_ids_for(&candidate.id);
        let Some(relationship) = past_session_relationship(
            &current_participant_ids,
            current_series_id.as_deref(),
            &current_title_key,
            &candidate_participant_ids,
            recurrence_series_id(candidate_event.as_ref()).as_deref(),
            &session_title_key(candidate),
        ) else {
            continue;
        };
        let Some(source) = session_key_facts_source(
            notes_by_session
                .get(candidate.id.as_str())
                .map(Vec::as_slice)
                .unwrap_or(&[]),
        ) else {
            continue;
        };
        let title = session_title(candidate);
        let date_label = format_session_date(candidate);
        let source_hash =
            create_source_hash(&[title.as_str(), date_label.as_str(), source.as_str()].join("\n"));
        let saved = data
            .key_facts
            .iter()
            .rev()
            .find(|row| row.session_id == candidate.id)
            .filter(|row| row.source_hash == source_hash && !row.content.trim().is_empty())
            .map(|row| row.content.trim().to_string());
        let mut all_ids: Vec<&str> = current_participant_ids
            .iter()
            .chain(candidate_participant_ids.iter())
            .map(String::as_str)
            .collect();
        all_ids.sort();
        all_ids.dedup();
        let participant_names = session_participant_names(&names_by_human, &all_ids);
        items.push(Item {
            note: PastSessionNote {
                session_id: candidate.id.clone(),
                title,
                date_label,
                participant_names,
                source_summary: source,
                relationship,
                summary: saved,
            },
            date_ms: candidate_timestamp,
        });
    }
    items.sort_by(|a, b| {
        b.note
            .relationship
            .cmp(&a.note.relationship)
            .then(b.date_ms.cmp(&a.date_ms))
    });
    items
        .into_iter()
        .take(MAX_PAST_NOTES)
        .map(|item| item.note)
        .collect()
}

fn session_participant_ids(
    participants: &[&PastParticipantRow],
    user_id: Option<&str>,
) -> HashSet<String> {
    let mut ids = HashSet::new();
    for mapping in participants {
        if mapping.source == "excluded" || mapping.human_id.is_empty() {
            continue;
        }
        let owner_user_id =
            (!mapping.user_id.trim().is_empty()).then_some(mapping.user_id.as_str());
        let is_current_user = match user_id {
            Some(user_id) => mapping.human_id == user_id,
            None => owner_user_id.is_some_and(|owner| mapping.human_id == owner),
        };
        if !is_current_user {
            ids.insert(mapping.human_id.clone());
        }
    }
    ids
}

fn session_participant_names(names_by_human: &HashMap<&str, &str>, ids: &[&str]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut names: Vec<String> = ids
        .iter()
        .map(|id| names_by_human.get(id).copied().unwrap_or(id).to_string())
        .filter(|name| seen.insert(name.to_lowercase()))
        .collect();
    names.sort_by_key(|name| name.to_lowercase());
    names
}

fn past_session_relationship(
    current_participant_ids: &HashSet<String>,
    current_series_id: Option<&str>,
    current_title_key: &str,
    candidate_participant_ids: &HashSet<String>,
    candidate_series_id: Option<&str>,
    candidate_title_key: &str,
) -> Option<Relationship> {
    if let Some(series) = current_series_id
        && candidate_series_id == Some(series)
    {
        return Some(Relationship::SameSeries);
    }
    let shares_participants = !current_participant_ids.is_empty()
        && candidate_participant_ids
            .iter()
            .any(|id| current_participant_ids.contains(id));
    if !current_title_key.is_empty()
        && current_title_key == candidate_title_key
        && shares_participants
    {
        return Some(Relationship::MatchingTitle);
    }
    if shares_participants {
        return Some(Relationship::SharedParticipants);
    }
    None
}

fn session_title(session: &PastSessionRow) -> String {
    let title = session.title.trim();
    if title.is_empty() {
        "Untitled".to_string()
    } else {
        title.to_string()
    }
}

fn session_title_key(session: &PastSessionRow) -> String {
    static SPACES: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"\s+").unwrap());
    let key = SPACES
        .replace_all(&session_title(session).to_lowercase(), " ")
        .to_string();
    if matches!(key.as_str(), "new note" | "untitled") {
        String::new()
    } else {
        key
    }
}

fn recurrence_series_id(event: Option<&EventJson>) -> Option<String> {
    event
        .and_then(|event| event.recurrence_series_id.as_deref())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

fn session_instant(session: &PastSessionRow) -> Option<DateTime<Utc>> {
    let event = session_event(session);
    let value = event
        .as_ref()
        .and_then(|event| event.started_at.as_deref())
        .filter(|value| !value.is_empty())
        .unwrap_or(session.created_at.as_str());
    if value.is_empty() {
        return None;
    }
    crate::scheduled_auto_start::parse_event_instant(value)
}

fn session_timestamp(session: &PastSessionRow) -> i64 {
    session_instant(session)
        .map(|instant| instant.timestamp_millis())
        .unwrap_or(0)
}

/// `format(parsed, "MMM d, yyyy")` in the local time zone.
fn format_session_date(session: &PastSessionRow) -> String {
    session_instant(session)
        .map(|instant| {
            instant
                .with_timezone(&chrono::Local)
                .format("%b %-d, %Y")
                .to_string()
        })
        .unwrap_or_default()
}

fn session_key_facts_source(notes: &[&PastEnhancedNoteRow]) -> Option<String> {
    let mut summaries: Vec<&&PastEnhancedNoteRow> = notes
        .iter()
        .filter(|note| !note.content.trim().is_empty())
        .collect();
    summaries.sort_by_key(|note| note.position);
    let text = summaries
        .iter()
        .map(|note| crate::contact_summary::extract_plain_text(&note.content))
        .collect::<Vec<_>>()
        .join("\n\n");
    let text = clean_source_text(&text);
    (!text.is_empty()).then(|| truncate_at_word(&text, MAX_SOURCE_LENGTH, "..."))
}

fn clean_source_text(text: &str) -> String {
    static IMAGES: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"!\[[^\]]*\]\([^)]+\)").unwrap());
    static LINKS: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\([^)]+\)").unwrap());
    static MARKS: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"[`*_~>#]").unwrap());
    static LIST_MARKERS: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"(^|\s)([-+]|[0-9]+[.)])\s+").unwrap());
    static SPACES: std::sync::LazyLock<Regex> =
        std::sync::LazyLock::new(|| Regex::new(r"\s+").unwrap());
    let text = IMAGES.replace_all(text, "");
    let text = LINKS.replace_all(&text, "$1");
    let text = MARKS.replace_all(&text, "");
    let text = LIST_MARKERS.replace_all(&text, " ");
    SPACES.replace_all(&text, " ").trim().to_string()
}

/// `createSourceHash`: FNV-1a over UTF-16 code units, lowercase hex.
fn create_source_hash(text: &str) -> String {
    let mut hash: u32 = 0x811c_9dc5;
    for unit in text.encode_utf16() {
        hash ^= u32::from(unit);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    format!("{hash:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(session_id: &str) -> PastSessionNote {
        PastSessionNote {
            session_id: session_id.to_string(),
            title: "Weekly sync".to_string(),
            date_label: "Aug 14, 2026".to_string(),
            participant_names: Vec::new(),
            source_summary: "The source summary remains available.".to_string(),
            relationship: Relationship::SameSeries,
            summary: Some("- Confirm launch timing.\n- Ada owns the prototype.".to_string()),
        }
    }

    fn timed(started_at: &str, ended_at: &str, all_day: bool) -> BriefEvent {
        BriefEvent {
            started_at: Some(started_at.to_string()),
            ended_at: Some(ended_at.to_string()),
            is_all_day: all_day,
            ..BriefEvent::default()
        }
    }

    #[test]
    fn keeps_link_labels_without_urls() {
        assert_eq!(
            compact_brief_text(
                "Review [launch plan](https://example.com/launch) at https://meet.example.com.",
                200
            ),
            "Review launch plan at"
        );
    }

    #[test]
    fn prefers_generated_facts_then_the_source_summary() {
        let n = note("previous");
        assert_eq!(
            pre_meeting_brief_facts(&n),
            vec!["Confirm launch timing.", "Ada owns the prototype."]
        );
        let without = PastSessionNote { summary: None, ..n };
        assert_eq!(
            pre_meeting_brief_facts(&without),
            vec!["The source summary remains available."]
        );
    }

    #[test]
    fn visibility_follows_the_event_window() {
        let now_ms = chrono::DateTime::parse_from_rfc3339("2026-08-21T08:00:00.000Z")
            .unwrap()
            .timestamp_millis();
        assert!(!should_show_pre_meeting_brief(
            Some(&timed(
                "2026-08-21T07:00:00.000Z",
                "2026-08-21T07:30:00.000Z",
                false
            )),
            now_ms
        ));
        assert!(should_show_pre_meeting_brief(
            Some(&timed("2026-08-21T07:58:00.000Z", "", false)),
            now_ms
        ));
        assert!(!should_show_pre_meeting_brief(
            Some(&timed(
                "2026-08-22T07:00:00.000Z",
                "2026-08-22T07:30:00.000Z",
                true
            )),
            now_ms
        ));
        assert!(should_show_pre_meeting_brief(
            Some(&timed(
                "2026-08-21T08:02:00.0000000",
                "2026-08-21T09:00:00.0000000",
                false
            )),
            now_ms
        ));
        // #7478: a short meeting hides at or after its end time, grace or not.
        for ended_at in ["2026-08-21T07:59:00.000Z", "2026-08-21T08:00:00.000Z"] {
            assert!(!should_show_pre_meeting_brief(
                Some(&timed("2026-08-21T07:58:00.000Z", ended_at, false)),
                now_ms
            ));
        }
    }

    #[test]
    fn requires_an_upcoming_event_or_participants_plus_notes() {
        let now_ms = chrono::DateTime::parse_from_rfc3339("2026-08-21T08:00:00.000Z")
            .unwrap()
            .timestamp_millis();
        let event = timed(
            "2026-08-21T09:00:00.000Z",
            "2026-08-21T10:00:00.000Z",
            false,
        );
        let notes = vec![note("previous")];
        assert!(can_create_pre_meeting_brief(
            Some(&event),
            now_ms,
            &notes,
            false
        ));
        assert!(!can_create_pre_meeting_brief(
            Some(&event),
            now_ms,
            &[],
            false
        ));
        assert!(!can_create_pre_meeting_brief(None, now_ms, &notes, false));
        assert!(can_create_pre_meeting_brief(None, now_ms, &notes, true));
        assert!(!can_create_pre_meeting_brief(None, now_ms, &[], true));
        // #7478: participants do not make an ended meeting eligible.
        let ended = timed(
            "2026-08-21T07:00:00.000Z",
            "2026-08-21T07:30:00.000Z",
            false,
        );
        assert!(!can_create_pre_meeting_brief(
            Some(&ended),
            now_ms,
            &notes,
            true
        ));
    }

    #[test]
    fn keeps_the_five_most_recent_usable_notes() {
        let notes: Vec<PastSessionNote> = (0..6)
            .map(|index| PastSessionNote {
                title: format!("Meeting {index}"),
                source_summary: if index == 2 {
                    String::new()
                } else {
                    format!("Notes from meeting {index}")
                },
                summary: (index != 2).then(|| format!("Fact {index}")),
                ..note(&format!("meeting-{index}"))
            })
            .collect();
        let ids: Vec<&str> = select_brief_source_notes(&notes)
            .iter()
            .map(|note| note.session_id.as_str())
            .collect();
        assert_eq!(
            ids,
            [
                "meeting-0",
                "meeting-1",
                "meeting-3",
                "meeting-4",
                "meeting-5"
            ]
        );
    }

    #[test]
    fn formats_an_opener_and_three_bullets() {
        let bullets = [
            "John still owns the scratchpad rewrite.",
            "CI cost gating is unresolved.",
            "Artem left the Korea workshop dates open.",
            "Extra fact should not appear.",
        ]
        .map(String::from);
        assert_eq!(
            format_pre_meeting_brief(Some("Ada slipped the prototype date."), &bullets),
            "**Ada slipped the prototype date.**\n\n- John still owns the scratchpad rewrite.\n- CI cost gating is unresolved.\n- Artem left the Korea workshop dates open."
        );
        assert_eq!(
            format_pre_meeting_brief(
                Some("One sentence: why this conversation matters."),
                &[
                    "John's proposal to show a Linear chip above the chat box.".to_string(),
                    "Yujong's commitment to discuss CI spending.".to_string()
                ]
            ),
            "- John's proposal to show a Linear chip above the chat box.\n- Yujong's commitment to discuss CI spending."
        );
    }

    #[test]
    fn trims_a_reply_to_one_liner_and_three_bullets() {
        assert_eq!(
            trim_pre_meeting_brief(
                "**Ada slipped the prototype date.**\n\n- John still owns the scratchpad rewrite.\n- CI cost gating is unresolved.\n- Artem left the Korea workshop dates open.\n- Extra fact should not appear.\n"
            ),
            "**Ada slipped the prototype date.**\n\n- John still owns the scratchpad rewrite.\n- CI cost gating is unresolved.\n- Artem left the Korea workshop dates open."
        );
        assert_eq!(
            trim_pre_meeting_brief(
                "**Design Sync is crucial for aligning the team's vision.**\n\n- Artem and John will discuss the single-surface scratchpad.\n- John will report on suggestions UI.\n- John will propose a Linear ticket chip.\n"
            ),
            "- Artem and John will discuss the single-surface scratchpad.\n- John will report on suggestions UI.\n- John will propose a Linear ticket chip."
        );
        assert_eq!(
            trim_pre_meeting_brief(
                "Quick Recap for Founders Sync Meeting:\n- John proposed a single-surface scratchpad.\n- Sungbin has been focusing on Linear tickets.\n- John raised CI cost gating.\n- Artem mentioned October workshop dates.\n- John asked Granola about the waitlist.\n\nUpcoming Meeting Insight:\n- Expect John to discuss the scratchpad.\n- Sungbin might present Linear progress.\n"
            ),
            "- John proposed a single-surface scratchpad.\n- Sungbin has been focusing on Linear tickets.\n- John raised CI cost gating."
        );
    }

    #[test]
    fn merges_the_brief_above_existing_notes() {
        assert_eq!(merge_brief_markdown("## Brief", ""), "## Brief");
        assert_eq!(
            merge_brief_markdown("## Brief", "Existing notes"),
            "## Brief\n\nExisting notes"
        );
    }

    #[test]
    fn prompt_sends_at_most_four_facts() {
        let notes: Vec<PastSessionNote> = (0..3)
            .map(|index| PastSessionNote {
                summary: Some(format!("- Fact {index}a.\n- Fact {index}b.")),
                ..note(&format!("meeting-{index}"))
            })
            .collect();
        let sources = select_brief_source_notes(&notes);
        let meetings = brief_prompt_meetings(&sources);
        let facts: Vec<String> = meetings
            .iter()
            .flat_map(|meeting| {
                meeting["notes"]
                    .as_str()
                    .unwrap()
                    .split('\n')
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(facts, ["Fact 0a.", "Fact 0b.", "Fact 1a.", "Fact 1b."]);
        let (system, user) = brief_prompts(
            "en",
            &BriefEvent {
                title: Some("Weekly Product Sync".to_string()),
                ..BriefEvent::default()
            },
            &notes,
        )
        .unwrap();
        assert!(system.contains("Write in en."));
        assert!(user.contains("Title: Weekly Product Sync"));
        assert!(user.contains("## Weekly sync (Aug 14, 2026)"));
    }

    #[test]
    fn partial_json_yields_the_complete_strings_so_far() {
        assert_eq!(
            brief_from_partial_json(r#"{"opener": "Follow up with Ada."#),
            Some((Some("Follow up with Ada.".to_string()), vec![]))
        );
        assert_eq!(
            brief_from_partial_json(
                r#"{"opener": "Follow up.", "bullets": ["Ada owns the prototype.", "CI is"#
            ),
            Some((
                Some("Follow up.".to_string()),
                vec!["Ada owns the prototype.".to_string(), "CI is".to_string()]
            ))
        );
        assert_eq!(
            brief_from_partial_json(r#"{"opener": "Follow up.", "bul"#),
            Some((Some("Follow up.".to_string()), vec![]))
        );
        assert_eq!(
            brief_from_partial_json(r#"{"opener": "Follow up.", "bullets":"#),
            Some((Some("Follow up.".to_string()), vec![]))
        );
        assert_eq!(
            brief_from_partial_json(r#"{"opener": "Follow up.", "bullets": ["A.","#),
            Some((Some("Follow up.".to_string()), vec!["A.".to_string()]))
        );
        assert_eq!(
            brief_from_partial_json(r#"{"opener": "Follow up.", "bullets": ["A.", "B."]}"#),
            Some((
                Some("Follow up.".to_string()),
                vec!["A.".to_string(), "B.".to_string()]
            ))
        );
        assert_eq!(complete_partial_json("{"), "{}");
    }

    #[test]
    fn builds_related_notes_newest_first_by_relationship() {
        let data = PastSessionNotesData {
            sessions: vec![
                PastSessionRow {
                    id: "current".into(),
                    user_id: "me".into(),
                    title: "Weekly sync".into(),
                    created_at: "2026-08-21T09:00:00Z".into(),
                    event_json:
                        r#"{"started_at":"2026-08-21T09:00:00Z","recurrence_series_id":"series-1"}"#
                            .into(),
                },
                PastSessionRow {
                    id: "series".into(),
                    user_id: "me".into(),
                    title: "Weekly sync".into(),
                    created_at: "2026-08-07T09:00:00Z".into(),
                    event_json:
                        r#"{"started_at":"2026-08-07T09:00:00Z","recurrence_series_id":"series-1"}"#
                            .into(),
                },
                PastSessionRow {
                    id: "shared".into(),
                    user_id: "me".into(),
                    title: "Design review".into(),
                    created_at: "2026-08-14T09:00:00Z".into(),
                    event_json: String::new(),
                },
                PastSessionRow {
                    id: "later".into(),
                    user_id: "me".into(),
                    title: "Weekly sync".into(),
                    created_at: "2026-08-28T09:00:00Z".into(),
                    event_json: String::new(),
                },
                PastSessionRow {
                    id: "unrelated".into(),
                    user_id: "me".into(),
                    title: "Solo".into(),
                    created_at: "2026-08-01T09:00:00Z".into(),
                    event_json: String::new(),
                },
            ],
            participants: vec![
                PastParticipantRow {
                    session_id: "current".into(),
                    human_id: "ada".into(),
                    user_id: "me".into(),
                    source: String::new(),
                    name: "Ada".into(),
                },
                PastParticipantRow {
                    session_id: "current".into(),
                    human_id: "me".into(),
                    user_id: "me".into(),
                    source: String::new(),
                    name: "Me".into(),
                },
                PastParticipantRow {
                    session_id: "shared".into(),
                    human_id: "ada".into(),
                    user_id: "me".into(),
                    source: String::new(),
                    name: "Ada".into(),
                },
                PastParticipantRow {
                    session_id: "later".into(),
                    human_id: "ada".into(),
                    user_id: "me".into(),
                    source: String::new(),
                    name: "Ada".into(),
                },
            ],
            enhanced_notes: vec![
                PastEnhancedNoteRow {
                    session_id: "series".into(),
                    content: "# Notes\n\n- Ship the **prototype** by Friday.".into(),
                    position: 0,
                },
                PastEnhancedNoteRow {
                    session_id: "shared".into(),
                    content: "Reviewed the mocks.".into(),
                    position: 0,
                },
                PastEnhancedNoteRow {
                    session_id: "later".into(),
                    content: "Future.".into(),
                    position: 0,
                },
                PastEnhancedNoteRow {
                    session_id: "unrelated".into(),
                    content: "Alone.".into(),
                    position: 0,
                },
            ],
            key_facts: Vec::new(),
        };
        let notes = build_past_session_notes(&data, "current", Some("me"));
        let ids: Vec<&str> = notes.iter().map(|note| note.session_id.as_str()).collect();
        assert_eq!(ids, ["series", "shared"]);
        assert_eq!(notes[0].relationship, Relationship::SameSeries);
        assert_eq!(
            notes[0].source_summary,
            "Notes Ship the prototype by Friday."
        );
        assert_eq!(notes[0].participant_names, vec!["Ada".to_string()]);
        assert_eq!(notes[1].relationship, Relationship::SharedParticipants);
    }

    #[test]
    fn source_hash_matches_the_javascript_fnv() {
        // (0x811c9dc5 ^ 'a') * 0x01000193 mod 2^32, as `createSourceHash("a")`.
        assert_eq!(create_source_hash("a"), "e40c292c");
    }
}
