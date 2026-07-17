use crate::cli::{DocumentKind, ExportFormat, MeetingCommand};
use crate::{Result, output};
use hypr_agent_access::{
    Document, GetMeetingInput, GetMeetingTranscriptInput, GetRecurringMeetingHistoryInput,
    ListMeetingsInput, MeetingListItem, get_meeting, get_meeting_export, get_meeting_transcript,
    get_recurring_meeting_history, list_meetings,
};

pub async fn run(db: &hypr_db_core::Db, command: MeetingCommand, json: bool) -> Result<()> {
    match command {
        MeetingCommand::List {
            query,
            series_id,
            limit,
            offset,
        } => {
            let page = list_meetings(
                db.pool(),
                ListMeetingsInput {
                    query,
                    series_id,
                    limit: Some(limit),
                    offset: Some(offset),
                },
            )
            .await?;
            let rendered = if json {
                output::json("meetings.list", &page.meetings, Some(&page.pagination))?
            } else {
                render_list(&page.meetings)
            };
            output::emit(&rendered);
            Ok(())
        }
        MeetingCommand::Get { id } => {
            let meeting = get_meeting(db.pool(), GetMeetingInput { meeting_id: id }).await?;
            let rendered = if json {
                output::json("meetings.get", &meeting, None)?
            } else {
                meeting.to_markdown()
            };
            output::emit(&rendered);
            Ok(())
        }
        MeetingCommand::Note { id, kind } => {
            let meeting = get_meeting(
                db.pool(),
                GetMeetingInput {
                    meeting_id: id.clone(),
                },
            )
            .await?;
            if json {
                match kind {
                    DocumentKind::Note => {
                        output::emit(&output::json("meetings.note", &meeting.note, None)?)
                    }
                    DocumentKind::Summary => {
                        output::emit(&output::json("meetings.note", &meeting.summaries, None)?)
                    }
                    DocumentKind::All => output::emit(&output::json(
                        "meetings.note",
                        &serde_json::json!({
                            "note": meeting.note,
                            "summaries": meeting.summaries,
                        }),
                        None,
                    )?),
                }
                return Ok(());
            }

            let text = match kind {
                DocumentKind::Note => meeting
                    .note
                    .map(|note| note.markdown)
                    .ok_or_else(|| crate::Error::NotFound(format!("note for meeting '{id}'")))?,
                DocumentKind::Summary => render_documents(&meeting.summaries),
                DocumentKind::All => {
                    let mut documents = meeting.note.into_iter().collect::<Vec<_>>();
                    documents.extend(meeting.summaries);
                    render_documents(&documents)
                }
            };
            output::emit(&text);
            Ok(())
        }
        MeetingCommand::Transcript { id, limit, offset } => {
            let page = get_meeting_transcript(
                db.pool(),
                GetMeetingTranscriptInput {
                    meeting_id: id,
                    offset: Some(offset),
                    limit: Some(limit),
                },
            )
            .await?;
            let rendered = if json {
                let content = serde_json::json!({
                    "meeting_id": &page.meeting_id,
                    "text": &page.text,
                    "words": &page.words,
                });
                output::json("meetings.transcript", &content, Some(&page.pagination))?
            } else {
                page.text
            };
            output::emit(&rendered);
            Ok(())
        }
        MeetingCommand::History { id, limit, offset } => {
            let page = get_recurring_meeting_history(
                db.pool(),
                GetRecurringMeetingHistoryInput {
                    meeting_id: id,
                    limit: Some(limit),
                    offset: Some(offset),
                },
            )
            .await?;
            let rendered = if json {
                output::json("meetings.history", &page.meetings, Some(&page.pagination))?
            } else {
                render_list(&page.meetings)
            };
            output::emit(&rendered);
            Ok(())
        }
        MeetingCommand::Export {
            id,
            format,
            output: path,
            force,
        } => {
            let meeting = get_meeting_export(db.pool(), id).await?;
            let content = match (format, json) {
                (ExportFormat::Markdown, false) => meeting.to_markdown(),
                (ExportFormat::Json, false) => output::raw_json(&meeting)?,
                (ExportFormat::Markdown, true) => output::json(
                    "meetings.export",
                    &serde_json::json!({
                        "format": "markdown",
                        "content": meeting.to_markdown(),
                    }),
                    None,
                )?,
                (ExportFormat::Json, true) => output::json("meetings.export", &meeting, None)?,
            };
            output::write_or_emit(&content, path.as_deref(), force)
        }
    }
}

fn render_list(meetings: &[MeetingListItem]) -> String {
    if meetings.is_empty() {
        return "No meetings found.".to_string();
    }

    let title_width = meetings
        .iter()
        .map(|meeting| meeting.title.chars().count())
        .max()
        .unwrap_or(0)
        .clamp(5, 48);
    let mut lines = vec![format!("{:<24}  {:<title_width$}  ID", "DATE", "TITLE")];
    for meeting in meetings {
        let occurred_at = if meeting.started_at.is_empty() {
            &meeting.created_at
        } else {
            &meeting.started_at
        };
        lines.push(format!(
            "{:<24}  {:<title_width$}  {}",
            truncate(occurred_at, 24),
            truncate(
                if meeting.title.is_empty() {
                    "Untitled"
                } else {
                    &meeting.title
                },
                title_width,
            ),
            meeting.id,
        ));
    }
    lines.join("\n")
}

fn render_documents(documents: &[Document]) -> String {
    documents
        .iter()
        .filter(|document| !document.markdown.trim().is_empty())
        .map(|document| {
            let title = if document.title.trim().is_empty() {
                "Summary"
            } else {
                document.title.trim()
            };
            format!("## {title}\n\n{}", document.markdown.trim())
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_string();
    }
    let mut text = value
        .chars()
        .take(width.saturating_sub(1))
        .collect::<String>();
    text.push('…');
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_render_is_bounded_and_contains_ids() {
        let rendered = render_list(&[MeetingListItem {
            id: "meeting-1".to_string(),
            title: "A very long planning meeting title that should not own the terminal"
                .to_string(),
            kind: "meeting".to_string(),
            status: "active".to_string(),
            created_at: "2026-07-13T09:00:00Z".to_string(),
            updated_at: "2026-07-13T09:00:00Z".to_string(),
            started_at: String::new(),
            ended_at: String::new(),
            series_id: String::new(),
        }]);
        assert!(rendered.contains("meeting-1"));
        assert!(rendered.contains('…'));
    }
}
