use serde_json::{Map, Value};

use crate::types::{MeetingSummary, RecordingMeeting, TranscriptSegment};

pub fn meeting_has_content(
    summary: Option<&MeetingSummary>,
    transcript: &[TranscriptSegment],
) -> bool {
    !transcript.is_empty() || summary.is_some_and(summary_has_content)
}

pub fn meeting_json(
    recording: &RecordingMeeting,
    summary: Option<&MeetingSummary>,
    transcript: Vec<TranscriptSegment>,
) -> Option<Value> {
    let id = recording.external_id()?;
    let title = recording
        .topic
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or("Zoom meeting");
    let mut record = Map::new();
    record.insert("id".into(), Value::String(id));
    if let Some(meeting_id) = recording.meeting_id() {
        record.insert("meeting_id".into(), Value::String(meeting_id));
    }
    record.insert("title".into(), Value::String(title.to_string()));
    if let Some(start_time) = recording.start_time.clone() {
        record.insert("start_time".into(), Value::String(start_time));
    }
    if let Some(url) = recording.share_url.clone() {
        record.insert("url".into(), Value::String(url));
    }
    if let Some(summary) = summary {
        if let Some(text) = summary_markdown(summary) {
            record.insert("summary".into(), Value::String(text));
        }
        let action_items = summary
            .next_steps
            .iter()
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .map(|item| Value::String(item.to_string()))
            .collect::<Vec<_>>();
        if !action_items.is_empty() {
            record.insert("action_items".into(), Value::Array(action_items));
        }
    }
    if !transcript.is_empty() {
        record.insert(
            "transcript".into(),
            serde_json::to_value(transcript).unwrap_or(Value::Array(Vec::new())),
        );
    }
    Some(Value::Object(record))
}

fn summary_has_content(summary: &MeetingSummary) -> bool {
    [
        summary.edited_summary.as_deref(),
        summary.summary_overview.as_deref(),
        summary.summary_title.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|value| !value.trim().is_empty())
        || summary.summary_details.iter().any(|detail| {
            detail
                .summary
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
        })
        || summary
            .next_steps
            .iter()
            .any(|item| !item.trim().is_empty())
}

fn summary_markdown(summary: &MeetingSummary) -> Option<String> {
    let mut sections = Vec::new();
    if let Some(title) = nonempty(summary.summary_title.as_deref()) {
        sections.push(title);
    }
    if let Some(overview) = nonempty(summary.edited_summary.as_deref())
        .or_else(|| nonempty(summary.summary_overview.as_deref()))
    {
        sections.push(overview);
    }
    for detail in &summary.summary_details {
        let Some(text) = nonempty(detail.summary.as_deref()) else {
            continue;
        };
        if let Some(label) = nonempty(detail.label.as_deref()) {
            sections.push(format!("**{label}**\n{text}"));
        } else {
            sections.push(text);
        }
    }
    if sections.is_empty() {
        None
    } else {
        Some(sections.join("\n\n"))
    }
}

fn nonempty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{RecordingFile, SummaryDetail};

    #[test]
    fn builds_importable_meeting_json() {
        let recording = RecordingMeeting {
            uuid: Some("meeting/one".into()),
            id: Some(serde_json::json!(123)),
            topic: Some("Weekly planning".into()),
            start_time: Some("2026-08-01T10:00:00Z".into()),
            share_url: Some("https://zoom.us/rec/share/abc".into()),
            recording_files: vec![RecordingFile {
                file_type: Some("TRANSCRIPT".into()),
                download_url: Some("https://zoom.us/rec/download/abc".into()),
                ..RecordingFile::default()
            }],
            ..RecordingMeeting::default()
        };
        let summary = MeetingSummary {
            summary_overview: Some("We agreed to ship.".into()),
            next_steps: vec!["Prepare the release".into()],
            summary_details: vec![SummaryDetail {
                label: Some("Decision".into()),
                summary: Some("Ship this week.".into()),
            }],
            ..MeetingSummary::default()
        };

        let json = meeting_json(
            &recording,
            Some(&summary),
            vec![TranscriptSegment {
                speaker: "Ada".into(),
                text: "Let's ship it.".into(),
                start_ms: 1_000,
                end_ms: 2_000,
            }],
        )
        .unwrap();

        assert_eq!(json["id"], "meeting/one");
        assert_eq!(json["title"], "Weekly planning");
        assert!(
            json["summary"]
                .as_str()
                .unwrap()
                .contains("We agreed to ship.")
        );
        assert_eq!(json["action_items"][0], "Prepare the release");
        assert_eq!(json["transcript"][0]["speaker"], "Ada");
    }
}
