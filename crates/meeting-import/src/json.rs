use serde::Serialize;
use serde_json::{Map, Value};

pub use anlg_zoom::TranscriptSegment;

#[derive(Debug, Clone, Default, Serialize)]
pub struct ImportedMeeting {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub transcript: Vec<TranscriptSegment>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub action_items: Vec<String>,
}

pub struct ImportFile {
    pub path: String,
    pub name: String,
    pub content: String,
}

pub fn meeting_has_content(meeting: &ImportedMeeting) -> bool {
    !meeting.transcript.is_empty()
        || nonempty(meeting.summary.as_deref()).is_some()
        || nonempty(meeting.notes.as_deref()).is_some()
        || meeting
            .action_items
            .iter()
            .any(|item| !item.trim().is_empty())
}

pub fn meeting_file(provider: &str, meeting: &ImportedMeeting) -> Result<ImportFile, String> {
    meeting_file_with_scheme("oauth", provider, meeting)
}

pub fn meeting_file_with_scheme(
    scheme: &str,
    provider: &str,
    meeting: &ImportedMeeting,
) -> Result<ImportFile, String> {
    let safe_id = safe_file_component(&meeting.id);
    let mut record = Map::new();
    record.insert("id".into(), Value::String(meeting.id.clone()));
    record.insert("title".into(), Value::String(meeting.title.clone()));
    if let Some(start_time) = nonempty(meeting.start_time.as_deref()) {
        record.insert("start_time".into(), Value::String(start_time));
    }
    if let Some(url) = nonempty(meeting.url.as_deref()) {
        record.insert("url".into(), Value::String(url));
    }
    if let Some(summary) = nonempty(meeting.summary.as_deref()) {
        record.insert("summary".into(), Value::String(summary));
    }
    if let Some(notes) = nonempty(meeting.notes.as_deref()) {
        record.insert("notes".into(), Value::String(notes));
    }
    if !meeting.transcript.is_empty() {
        record.insert(
            "transcript".into(),
            serde_json::to_value(&meeting.transcript).unwrap_or(Value::Array(Vec::new())),
        );
    }
    let action_items = meeting
        .action_items
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(|item| Value::String(item.to_string()))
        .collect::<Vec<_>>();
    if !action_items.is_empty() {
        record.insert("action_items".into(), Value::Array(action_items));
    }

    Ok(ImportFile {
        path: format!("{scheme}://{provider}/{safe_id}.json"),
        name: format!("{safe_id}.json"),
        content: serde_json::to_string(&Value::Object(record))
            .map_err(|error| format!("could not save a meeting: {error}"))?,
    })
}

pub fn nonempty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub fn safe_file_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "meeting".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_importable_meeting_json() {
        let meeting = ImportedMeeting {
            id: "meeting/one".into(),
            title: "Weekly planning".into(),
            start_time: Some("2026-08-01T10:00:00Z".into()),
            url: Some("https://example.com/rec".into()),
            summary: Some("We agreed to ship.".into()),
            notes: None,
            transcript: vec![TranscriptSegment {
                speaker: "Ada".into(),
                text: "Let's ship it.".into(),
                start_ms: 1_000,
                end_ms: 2_000,
            }],
            action_items: vec!["Prepare the release".into()],
        };
        let file = meeting_file("fathom", &meeting).unwrap();
        assert_eq!(file.path, "oauth://fathom/meeting-one.json");
        let cli = meeting_file_with_scheme("cli", "plaud", &meeting).unwrap();
        assert_eq!(cli.path, "cli://plaud/meeting-one.json");
        let json: Value = serde_json::from_str(&file.content).unwrap();
        assert_eq!(json["id"], "meeting/one");
        assert_eq!(json["title"], "Weekly planning");
        assert_eq!(json["action_items"][0], "Prepare the release");
        assert_eq!(json["transcript"][0]["speaker"], "Ada");
    }
}
