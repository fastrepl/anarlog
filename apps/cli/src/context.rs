use serde::Serialize;
use serde_json::Value;

use crate::{Error, Result};

pub const DEFAULT_LIST_LIMIT: u32 = 20;
pub const MAX_LIST_LIMIT: u32 = 200;
pub const DEFAULT_TRANSCRIPT_LIMIT: u32 = 200;
pub const MAX_TRANSCRIPT_LIMIT: u32 = 500;

#[derive(Debug, Serialize)]
pub struct Pagination {
    pub offset: u32,
    pub limit: u32,
    pub returned: usize,
    pub total: Option<usize>,
    pub next_offset: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct MeetingPage {
    pub meetings: Vec<hypr_db_app::SessionListItem>,
    pub pagination: Pagination,
}

#[derive(Debug, Serialize)]
pub struct TranscriptPage {
    #[serde(flatten)]
    pub content: TranscriptPageContent,
    pub pagination: Pagination,
}

#[derive(Debug, Serialize)]
pub struct TranscriptPageContent {
    pub meeting_id: String,
    pub text: String,
    pub words: Vec<Value>,
}

#[derive(Debug, Serialize)]
pub struct Meeting {
    #[serde(flatten)]
    pub session: hypr_db_app::SessionRow,
    pub note: Option<Document>,
    pub summaries: Vec<Document>,
    pub participants: Vec<hypr_db_app::SessionParticipantRow>,
    pub action_items: Vec<hypr_db_app::SessionActionItemRow>,
}

#[derive(Debug, Serialize)]
pub struct MeetingExport {
    #[serde(flatten)]
    pub meeting: Meeting,
    pub transcripts: Vec<Transcript>,
}

#[derive(Debug, Serialize)]
pub struct Document {
    pub id: String,
    pub kind: String,
    pub template_id: String,
    pub title: String,
    pub body_format: String,
    pub body: String,
    pub markdown: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct Transcript {
    pub id: String,
    pub source: String,
    pub provider: String,
    pub model: String,
    pub language: String,
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,
    pub memo: String,
    pub text: String,
    pub words: Vec<Value>,
    pub speaker_hints: Vec<Value>,
}

impl Meeting {
    pub async fn load(db: &hypr_db_core::Db, id: &str) -> Result<Self> {
        let pool = db.pool();
        let (session, note, documents, participants, action_items) = tokio::try_join!(
            hypr_db_app::get_session(pool, id),
            hypr_db_app::get_session_note(pool, id),
            hypr_db_app::list_session_documents(pool, id),
            hypr_db_app::list_session_participants(pool, id),
            hypr_db_app::list_session_action_items(pool, id),
        )
        .map_err(|error| Error::operation("load meeting", error.to_string()))?;

        let session = session.ok_or_else(|| Error::NotFound(format!("meeting '{id}'")))?;
        let summaries = documents
            .into_iter()
            .filter(|document| matches!(document.kind.as_str(), "summary" | "template_output"))
            .map(Document::from)
            .collect();

        Ok(Self {
            session,
            note: note.map(Document::from),
            summaries,
            participants,
            action_items,
        })
    }

    pub fn to_markdown(&self) -> String {
        let title = if self.session.title.trim().is_empty() {
            "Untitled meeting"
        } else {
            self.session.title.trim()
        };
        let mut sections = vec![format!("# {title}"), self.metadata_markdown()];

        if let Some(note) = &self.note {
            push_section(&mut sections, "Notes", &note.markdown);
        }
        for summary in &self.summaries {
            let heading = if summary.title.trim().is_empty() {
                "Summary"
            } else {
                summary.title.trim()
            };
            push_section(&mut sections, heading, &summary.markdown);
        }
        if !self.action_items.is_empty() {
            let body = self
                .action_items
                .iter()
                .map(|item| {
                    let checked = matches!(item.status.as_str(), "done" | "completed");
                    format!("- [{}] {}", if checked { "x" } else { " " }, item.text)
                })
                .collect::<Vec<_>>()
                .join("\n");
            push_section(&mut sections, "Action items", &body);
        }

        sections.join("\n\n").trim().to_string()
    }

    fn metadata_markdown(&self) -> String {
        let occurred_at = if self.session.started_at.is_empty() {
            &self.session.created_at
        } else {
            &self.session.started_at
        };
        let mut lines = vec![
            format!("- ID: `{}`", self.session.id),
            format!("- Date: {occurred_at}"),
        ];
        if !self.session.series_id.is_empty() {
            lines.push(format!("- Series: `{}`", self.session.series_id));
        }
        let people = self
            .participants
            .iter()
            .filter_map(|participant| {
                let name = participant.display_name.trim();
                (!name.is_empty()).then_some(name)
            })
            .collect::<Vec<_>>();
        if !people.is_empty() {
            lines.push(format!("- Participants: {}", people.join(", ")));
        }
        lines.join("\n")
    }
}

impl MeetingExport {
    pub async fn load(db: &hypr_db_core::Db, id: &str) -> Result<Self> {
        let (meeting, transcripts) =
            tokio::try_join!(Meeting::load(db, id), load_transcripts(db, id),)?;
        Ok(Self {
            meeting,
            transcripts,
        })
    }

    pub fn to_markdown(&self) -> String {
        let mut markdown = self.meeting.to_markdown();
        let transcript = render_transcripts(&self.transcripts);
        if !transcript.is_empty() {
            markdown.push_str("\n\n## Transcript\n\n");
            markdown.push_str(&transcript);
        }
        markdown
    }
}

impl From<hypr_db_app::SessionDocumentRow> for Document {
    fn from(value: hypr_db_app::SessionDocumentRow) -> Self {
        let markdown = body_to_markdown(&value.body, &value.body_format);
        Self {
            id: value.id,
            kind: value.kind,
            template_id: value.template_id,
            title: value.title,
            body_format: value.body_format,
            body: value.body,
            markdown,
            sort_order: value.sort_order,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<hypr_db_app::SessionTranscriptRow> for Transcript {
    fn from(value: hypr_db_app::SessionTranscriptRow) -> Self {
        let words = json_array(&value.words_json);
        let text = transcript_text(&words);
        Self {
            id: value.id,
            source: value.source,
            provider: value.provider,
            model: value.model,
            language: value.language,
            started_at_ms: value.started_at_ms,
            ended_at_ms: value.ended_at_ms,
            memo: value.memo,
            text,
            words,
            speaker_hints: json_array(&value.speaker_hints_json),
        }
    }
}

pub async fn load_transcripts(db: &hypr_db_core::Db, id: &str) -> Result<Vec<Transcript>> {
    hypr_db_app::list_session_transcripts(db.pool(), id)
        .await
        .map(|rows| rows.into_iter().map(Transcript::from).collect())
        .map_err(|error| Error::operation("load transcript", error.to_string()))
}

pub async fn list_meetings_page(
    db: &hypr_db_core::Db,
    query: Option<&str>,
    series_id: Option<&str>,
    limit: u32,
    offset: u32,
) -> Result<MeetingPage> {
    let limit = limit.clamp(1, MAX_LIST_LIMIT);
    let mut meetings = hypr_db_app::list_sessions(
        db.pool(),
        hypr_db_app::ListSessions {
            query,
            series_id,
            limit: limit + 1,
            offset,
        },
    )
    .await
    .map_err(|error| Error::operation("list meetings", error.to_string()))?;
    let has_more = meetings.len() > limit as usize;
    meetings.truncate(limit as usize);
    let pagination = pagination(offset, limit, meetings.len(), None, has_more);

    Ok(MeetingPage {
        meetings,
        pagination,
    })
}

pub async fn recurring_meetings_page(
    db: &hypr_db_core::Db,
    meeting_id: &str,
    limit: u32,
    offset: u32,
) -> Result<MeetingPage> {
    let meeting = hypr_db_app::get_session(db.pool(), meeting_id)
        .await
        .map_err(|error| Error::operation("load meeting", error.to_string()))?
        .ok_or_else(|| Error::NotFound(format!("meeting '{meeting_id}'")))?;
    let series_id = meeting.series_id.trim();
    if series_id.is_empty() {
        let limit = limit.clamp(1, MAX_LIST_LIMIT);
        return Ok(MeetingPage {
            meetings: Vec::new(),
            pagination: pagination(offset, limit, 0, Some(0), false),
        });
    }

    list_meetings_page(db, None, Some(series_id), limit, offset).await
}

pub fn transcript_page(
    meeting_id: &str,
    transcripts: &[Transcript],
    offset: u32,
    limit: u32,
) -> TranscriptPage {
    let mut words = Vec::new();
    for transcript in transcripts {
        for word in &transcript.words {
            let mut word = word.clone();
            if let Some(object) = word.as_object_mut() {
                object.insert(
                    "transcript_id".to_string(),
                    Value::String(transcript.id.clone()),
                );
            }
            words.push(word);
        }
    }

    let total_words = words.len();
    let offset_usize = offset as usize;
    let limit = limit.clamp(1, MAX_TRANSCRIPT_LIMIT);
    let words = words
        .into_iter()
        .skip(offset_usize)
        .take(limit as usize)
        .collect::<Vec<_>>();
    let text = words
        .iter()
        .filter_map(|word| word.get("text").and_then(Value::as_str))
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let has_more = offset_usize.saturating_add(words.len()) < total_words;
    let pagination = pagination(offset, limit, words.len(), Some(total_words), has_more);

    TranscriptPage {
        content: TranscriptPageContent {
            meeting_id: meeting_id.to_string(),
            text,
            words,
        },
        pagination,
    }
}

pub fn render_transcripts(transcripts: &[Transcript]) -> String {
    transcripts
        .iter()
        .filter(|transcript| !transcript.text.trim().is_empty())
        .map(|transcript| transcript.text.trim())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn body_to_markdown(body: &str, format: &str) -> String {
    if format != "prosemirror_json" {
        return body.to_string();
    }
    serde_json::from_str(body)
        .ok()
        .and_then(|value| hypr_tiptap::tiptap_json_to_md(&value).ok())
        .map(|markdown| markdown.trim_end().to_string())
        .unwrap_or_else(|| body.to_string())
}

fn json_array(value: &str) -> Vec<Value> {
    serde_json::from_str(value).unwrap_or_default()
}

fn transcript_text(words: &[Value]) -> String {
    words
        .iter()
        .filter_map(|word| word.get("text").and_then(Value::as_str))
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn push_section(sections: &mut Vec<String>, title: &str, body: &str) {
    if !body.trim().is_empty() {
        sections.push(format!("## {title}\n\n{}", body.trim()));
    }
}

fn pagination(
    offset: u32,
    limit: u32,
    returned: usize,
    total: Option<usize>,
    has_more: bool,
) -> Pagination {
    Pagination {
        offset,
        limit,
        returned,
        total,
        next_offset: has_more.then(|| offset.saturating_add(returned as u32)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_prosemirror_and_tolerates_invalid_json() {
        let body = serde_json::json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": "Hello"}]
            }]
        })
        .to_string();
        assert_eq!(body_to_markdown(&body, "prosemirror_json"), "Hello");
        assert_eq!(body_to_markdown("{broken", "prosemirror_json"), "{broken");
    }

    #[test]
    fn transcript_text_uses_word_text() {
        let words = serde_json::json!([
            {"text": " Hello "},
            {"text": "world."},
            {"other": "ignored"}
        ]);
        assert_eq!(transcript_text(words.as_array().unwrap()), "Hello world.");
    }

    #[test]
    fn transcript_does_not_treat_meeting_memo_as_spoken_text() {
        let transcript = Transcript::from(hypr_db_app::SessionTranscriptRow {
            id: "transcript-1".to_string(),
            workspace_id: String::new(),
            owner_user_id: String::new(),
            session_id: "meeting-1".to_string(),
            source: String::new(),
            provider: String::new(),
            model: String::new(),
            language: String::new(),
            started_at_ms: 0,
            ended_at_ms: None,
            audio_attachment_id: String::new(),
            memo: "private meeting note".to_string(),
            words_json: r#"[{"text":"spoken words"}]"#.to_string(),
            speaker_hints_json: "[]".to_string(),
            metadata_json: "{}".to_string(),
            created_at: String::new(),
            updated_at: String::new(),
        });

        assert_eq!(transcript.text, "spoken words");
        assert_eq!(transcript.memo, "private meeting note");
    }

    #[test]
    fn transcript_page_is_bounded_and_has_pagination() {
        let transcript = Transcript {
            id: "transcript-1".to_string(),
            source: String::new(),
            provider: String::new(),
            model: String::new(),
            language: "en".to_string(),
            started_at_ms: 0,
            ended_at_ms: None,
            memo: String::new(),
            text: String::new(),
            words: vec![
                serde_json::json!({"text": "one"}),
                serde_json::json!({"text": "two"}),
                serde_json::json!({"text": "three"}),
            ],
            speaker_hints: Vec::new(),
        };

        let page = transcript_page("meeting-1", &[transcript], 1, 1);

        assert_eq!(page.content.text, "two");
        assert_eq!(page.pagination.total, Some(3));
        assert_eq!(page.pagination.next_offset, Some(2));
        assert_eq!(page.content.words[0]["transcript_id"], "transcript-1");
    }
}
