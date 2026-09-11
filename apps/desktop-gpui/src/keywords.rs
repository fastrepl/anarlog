//! `stt/useKeywords.ts`: the transcription hints sent as `keywords` when a
//! live or batch transcription starts — mapped participants, attached event
//! attendees, dictionary terms, and `#hashtags` from the note, title, and
//! event metadata.
//!
//! The Tauri app additionally runs `retext-keywords` (a Brill part-of-speech
//! tagger over a ~100k-word LGPL lexicon) to add frequent nouns and noun
//! phrases. That lexicon is not bundled here, so the shell sends the
//! deterministic subset only: fewer hints, never different ones.

use crate::workspace::dictionary::normalize_keyword_list;

const MAX_TRANSCRIPTION_HINTS: usize = 50;

/// The `getSessionKeywords` SQL row.
#[derive(Debug, Clone, Default, sqlx::FromRow)]
pub struct Snapshot {
    pub raw_md: String,
    pub title: String,
    pub event_json: Option<String>,
    pub participant_names_json: String,
    pub event_participants_json: String,
}

pub const SNAPSHOT_SQL: &str = r#"
SELECT
  COALESCE(note.body, '') AS raw_md,
  COALESCE(session.title, '') AS title,
  session.event_json,
  COALESCE((
    SELECT json_group_array(name)
    FROM (
      SELECT COALESCE(NULLIF(human.name, ''), participant.display_name) AS name
      FROM session_participants AS participant
      LEFT JOIN humans AS human
        ON human.id = participant.human_id
        AND human.deleted_at IS NULL
      WHERE participant.session_id = session.id
        AND participant.source <> 'excluded'
        AND participant.deleted_at IS NULL
        AND COALESCE(NULLIF(human.name, ''), participant.display_name) <> ''
      ORDER BY name, participant.id
    )
  ), '[]') AS participant_names_json,
  COALESCE((
    SELECT event.participants_json
    FROM events AS event
    WHERE event.deleted_at IS NULL
      AND (
        event.id = session.event_id
        OR (
          event.tracking_id_event = CASE
            WHEN json_valid(session.event_json)
            THEN json_extract(session.event_json, '$.tracking_id')
            ELSE ''
          END
          AND event.calendar_id = CASE
            WHEN json_valid(session.event_json)
            THEN json_extract(session.event_json, '$.calendar_id')
            ELSE ''
          END
        )
      )
    ORDER BY event.started_at, event.id
    LIMIT 1
  ), '[]') AS event_participants_json
FROM sessions AS session
LEFT JOIN session_documents AS note
  ON note.id = session.id
  AND note.kind = 'note'
  AND note.deleted_at IS NULL
WHERE session.id = ? AND session.deleted_at IS NULL
LIMIT 1
"#;

/// `getSessionKeywords`: `buildKeywords` over the snapshot row.
pub fn session_keywords(snapshot: Option<&Snapshot>, dictionary_terms: &[String]) -> Vec<String> {
    let Some(snapshot) = snapshot else {
        return build_keywords("", "", None, &[], &[], dictionary_terms);
    };
    build_keywords(
        &snapshot.raw_md,
        &snapshot.title,
        snapshot.event_json.as_deref(),
        &parse_string_list(&snapshot.participant_names_json),
        &parse_event_participant_names(&snapshot.event_participants_json),
        dictionary_terms,
    )
}

/// `buildKeywords`: participants, attendees, dictionary, then extracted
/// terms, normalised and capped at `MAX_TRANSCRIPTION_HINTS`.
pub fn build_keywords(
    raw_md: &str,
    title: &str,
    event_json: Option<&str>,
    session_participant_terms: &[String],
    event_participant_terms: &[String],
    dictionary_terms: &[String],
) -> Vec<String> {
    let source_text = build_keyword_source_text(raw_md, title, event_json);
    let extracted = if source_text.is_empty() {
        Vec::new()
    } else {
        extract_keywords_from_markdown(&source_text)
    };
    let mut keywords = normalize_keyword_list(
        session_participant_terms
            .iter()
            .chain(event_participant_terms)
            .chain(dictionary_terms)
            .chain(&extracted)
            .map(String::as_str),
    );
    keywords.truncate(MAX_TRANSCRIPTION_HINTS);
    keywords
}

/// `buildKeywordSourceText`: note, title, and the event's title,
/// description, and location, one per line.
pub fn build_keyword_source_text(raw_md: &str, title: &str, event_json: Option<&str>) -> String {
    let mut fields = vec![raw_md.trim().to_string(), title.trim().to_string()];
    fields.extend(event_keyword_fields(event_json));
    fields
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// `extractKeywordsFromMarkdown`, minus the part-of-speech pass: the
/// `#hashtags` outside code, at least two characters long.
pub fn extract_keywords_from_markdown(markdown: &str) -> Vec<String> {
    let text = remove_code_blocks(markdown);
    extract_hashtags(&text)
        .into_iter()
        .filter(|keyword| keyword.chars().count() >= 2)
        .collect()
}

fn event_keyword_fields(event_json: Option<&str>) -> Vec<String> {
    let Some(event_json) = event_json.filter(|json| !json.is_empty()) else {
        return Vec::new();
    };
    let Ok(event) = serde_json::from_str::<serde_json::Value>(event_json) else {
        return Vec::new();
    };
    ["title", "description", "location"]
        .iter()
        .filter_map(|key| event.get(key).and_then(|value| value.as_str()))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_string_list(value: &str) -> Vec<String> {
    match serde_json::from_str::<serde_json::Value>(value) {
        Ok(serde_json::Value::Array(items)) => items
            .into_iter()
            .filter_map(|item| match item {
                serde_json::Value::String(text) => Some(text),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// `parseEventParticipantNames`: attendee names except the current user.
fn parse_event_participant_names(participants_json: &str) -> Vec<String> {
    match serde_json::from_str::<serde_json::Value>(participants_json) {
        Ok(serde_json::Value::Array(participants)) => participants
            .iter()
            .filter(|participant| {
                participant
                    .get("is_current_user")
                    .and_then(|value| value.as_bool())
                    != Some(true)
            })
            .filter_map(|participant| participant.get("name").and_then(|name| name.as_str()))
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

/// `removeCodeBlocks`: fenced blocks first, then inline code.
fn remove_code_blocks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("```") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 3..];
        match after.find("```") {
            Some(end) => rest = &after[end + 3..],
            None => {
                // An unterminated fence is not a block: `/```[\s\S]*?```/`
                // leaves it in place.
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    remove_inline_code(&out)
}

fn remove_inline_code(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('`') {
        let after = &rest[start + 1..];
        // `/`[^`]+`/`: at least one non-backtick character between the ticks.
        match after.find('`') {
            Some(end) if end > 0 => {
                out.push_str(&rest[..start]);
                rest = &after[end + 1..];
            }
            _ => {
                out.push_str(&rest[..=start]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// `/#([\p{L}\p{N}_]+)/gu`
fn extract_hashtags(text: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((_, ch)) = chars.next() {
        if ch != '#' {
            continue;
        }
        let mut tag = String::new();
        while let Some(&(_, next)) = chars.peek() {
            if next.is_alphanumeric() || next == '_' {
                tag.push(next);
                chars.next();
            } else {
                break;
            }
        }
        if !tag.is_empty() {
            tags.push(tag);
        }
    }
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn extracts_hashtags_from_markdown() {
        assert_eq!(
            extract_keywords_from_markdown("This is #awesome and #cool stuff"),
            strings(&["awesome", "cool"])
        );
    }

    #[test]
    fn handles_unicode_hashtags() {
        assert_eq!(
            extract_keywords_from_markdown("#日本語 #한글 #Español"),
            strings(&["日本語", "한글", "Español"])
        );
    }

    #[test]
    fn excludes_code_blocks_and_inline_code() {
        let keywords = extract_keywords_from_markdown(
            "Use the `#useState` hook\n```js\nconst value = 1; // #todo\n```\nKeep #reading for keywords",
        );
        assert_eq!(keywords, strings(&["reading"]));
    }

    #[test]
    fn leaves_unterminated_fences_and_empty_ticks_alone() {
        assert_eq!(remove_code_blocks("a ``` b #c"), "a ``` b #c");
        // `/`[^`]+`/g` skips the empty pair and pairs the next two ticks.
        assert_eq!(remove_code_blocks("a `` b `x` c"), "a `x` c");
    }

    #[test]
    fn filters_single_character_hashtags() {
        assert!(extract_keywords_from_markdown("#a #b #cd").eq(&strings(&["cd"])));
    }

    #[test]
    fn source_text_includes_note_title_and_event_metadata() {
        let event = serde_json::json!({
            "title": "OpenWorld review",
            "description": "Airborne Brothers follow-up",
            "location": "Zoom",
        })
        .to_string();
        assert_eq!(
            build_keyword_source_text("Discuss product launch", "Erebor sync", Some(&event)),
            [
                "Discuss product launch",
                "Erebor sync",
                "OpenWorld review",
                "Airborne Brothers follow-up",
                "Zoom",
            ]
            .join("\n")
        );
        assert_eq!(build_keyword_source_text("", "", Some("not json")), "");
    }

    #[test]
    fn builds_keywords_from_the_session_snapshot() {
        let snapshot = Snapshot {
            raw_md: "Discuss #Launch and production systems".into(),
            title: "Erebor sync".into(),
            event_json: Some(
                serde_json::json!({
                    "title": "OpenWorld review",
                    "description": "Airborne Brothers follow-up",
                    "location": "Zoom",
                })
                .to_string(),
            ),
            participant_names_json: "[]".into(),
            event_participants_json: "[]".into(),
        };
        let keywords = session_keywords(Some(&snapshot), &strings(&["Anarlog"]));
        assert_eq!(keywords, strings(&["Anarlog", "Launch"]));
    }

    #[test]
    fn prioritizes_mapped_participants_and_attached_event_attendees() {
        let snapshot = Snapshot {
            raw_md: "Discuss #Launch and production systems".into(),
            title: "Erebor sync".into(),
            event_json: None,
            participant_names_json: serde_json::json!(["Alice Kim"]).to_string(),
            event_participants_json: serde_json::json!([
                { "name": "Alice Kim", "email": "alice@example.com" },
                { "name": "Mina Park", "email": "mina@example.com" },
                { "name": "John Jeong", "email": "john@example.com", "is_current_user": true },
            ])
            .to_string(),
        };
        let keywords = session_keywords(Some(&snapshot), &strings(&["Anarlog"]));
        assert_eq!(
            keywords,
            strings(&["Alice Kim", "Mina Park", "Anarlog", "Launch"])
        );
    }

    #[test]
    fn caps_hints_and_dedupes_case_insensitively() {
        let dictionary: Vec<String> = (0..60).map(|i| format!("term{i}")).collect();
        let keywords = build_keywords("#term0 #TERM1 #fresh", "", None, &[], &[], &dictionary);
        assert_eq!(keywords.len(), MAX_TRANSCRIPTION_HINTS);
        assert_eq!(keywords[0], "term0");
        assert!(!keywords.contains(&"TERM1".to_string()));
        assert!(!keywords.contains(&"fresh".to_string()));
    }

    #[test]
    fn missing_snapshot_still_sends_dictionary_terms() {
        assert_eq!(
            session_keywords(None, &strings(&["Anarlog", "x"])),
            strings(&["Anarlog"])
        );
    }
}
