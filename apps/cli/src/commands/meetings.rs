use crate::cli::{DocumentKind, ExportFormat, MeetingCommand};
use crate::context::{Meeting, MeetingExport, load_transcripts, render_transcripts};
use crate::{Error, Result, output};

pub async fn run(db: &hypr_db_core::Db, command: MeetingCommand, json: bool) -> Result<()> {
    match command {
        MeetingCommand::List {
            query,
            series_id,
            limit,
            offset,
        } => {
            let meetings = hypr_db_app::list_sessions(
                db.pool(),
                hypr_db_app::ListSessions {
                    query: query.as_deref(),
                    series_id: series_id.as_deref(),
                    limit,
                    offset,
                },
            )
            .await
            .map_err(|error| Error::operation("list meetings", error.to_string()))?;
            let rendered = if json {
                output::json(&meetings)?
            } else {
                render_list(&meetings)
            };
            output::emit(&rendered);
            Ok(())
        }
        MeetingCommand::Get { id } => {
            let meeting = Meeting::load(db, &id).await?;
            let rendered = if json {
                output::json(&meeting)?
            } else {
                meeting.to_markdown()
            };
            output::emit(&rendered);
            Ok(())
        }
        MeetingCommand::Note { id, kind } => {
            let meeting = Meeting::load(db, &id).await?;
            if json {
                match kind {
                    DocumentKind::Note => output::emit(&output::json(&meeting.note)?),
                    DocumentKind::Summary => output::emit(&output::json(&meeting.summaries)?),
                    DocumentKind::All => output::emit(&output::json(&serde_json::json!({
                        "note": meeting.note,
                        "summaries": meeting.summaries,
                    }))?),
                }
                return Ok(());
            }

            let text = match kind {
                DocumentKind::Note => meeting
                    .note
                    .map(|note| note.markdown)
                    .ok_or_else(|| Error::NotFound(format!("note for meeting '{id}'")))?,
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
        MeetingCommand::Transcript { id } => {
            ensure_session_exists(db, &id).await?;
            let transcripts = load_transcripts(db, &id).await?;
            let rendered = if json {
                output::json(&transcripts)?
            } else {
                render_transcripts(&transcripts)
            };
            output::emit(&rendered);
            Ok(())
        }
        MeetingCommand::History { id, limit } => {
            ensure_session_exists(db, &id).await?;
            let meetings = hypr_db_app::list_recurring_sessions(db.pool(), &id, limit)
                .await
                .map_err(|error| Error::operation("load recurring history", error.to_string()))?;
            let rendered = if json {
                output::json(&meetings)?
            } else {
                render_list(&meetings)
            };
            output::emit(&rendered);
            Ok(())
        }
        MeetingCommand::Export {
            id,
            format,
            output: path,
        } => {
            let meeting = MeetingExport::load(db, &id).await?;
            let content = match format {
                ExportFormat::Markdown => meeting.to_markdown(),
                ExportFormat::Json => output::json(&meeting)?,
            };
            output::write_or_emit(&content, path.as_deref())
        }
    }
}

async fn ensure_session_exists(db: &hypr_db_core::Db, id: &str) -> Result<()> {
    let exists = hypr_db_app::get_session(db.pool(), id)
        .await
        .map_err(|error| Error::operation("load meeting", error.to_string()))?
        .is_some();
    if exists {
        Ok(())
    } else {
        Err(Error::NotFound(format!("meeting '{id}'")))
    }
}

fn render_list(meetings: &[hypr_db_app::SessionListItem]) -> String {
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

fn render_documents(documents: &[crate::context::Document]) -> String {
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
        let rendered = render_list(&[hypr_db_app::SessionListItem {
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
