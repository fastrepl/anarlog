use std::path::Path;

use sqlx::SqlitePool;

use super::DocFormat;
use super::helpers::{build_segments, capitalize};
use crate::error::{CliError, CliResult};
use crate::{db, output};

pub(super) async fn meeting(
    pool: &SqlitePool,
    id: &str,
    format: DocFormat,
    out: Option<&Path>,
) -> CliResult<()> {
    let meeting = db!(hypr_db_app::get_meeting(pool, id), "get meeting")
        .ok_or_else(|| CliError::not_found(format!("meeting '{id}'"), None))?;

    let participants = db!(
        hypr_db_app::list_meeting_participants(pool, id),
        "list participants"
    );
    let notes = db!(hypr_db_app::list_notes_by_meeting(pool, id), "list notes");
    let words = db!(hypr_db_app::load_words(pool, id), "load words");
    let hints = db!(hypr_db_app::load_hints(pool, id), "load hints");
    let chat_messages = db!(
        hypr_db_app::load_chat_messages(pool, id),
        "load chat messages"
    );

    let human_ids: Vec<&str> = participants.iter().map(|p| p.human_id.as_str()).collect();
    let mut participant_names = Vec::new();
    for hid in &human_ids {
        let name = hypr_db_app::get_human(pool, hid)
            .await
            .ok()
            .flatten()
            .map(|h| h.name)
            .unwrap_or_else(|| hid.to_string());
        participant_names.push(name);
    }

    let segments = build_segments(&words, &hints);

    match format {
        DocFormat::Json => {
            let value = serde_json::json!({
                "id": meeting.id,
                "title": meeting.title,
                "created_at": meeting.created_at,
                "participants": participant_names,
                "notes": notes.iter().map(|n| serde_json::json!({
                    "kind": n.kind,
                    "title": n.title,
                    "content": n.content,
                })).collect::<Vec<_>>(),
                "transcript": segments.iter().map(|s| serde_json::json!({
                    "speaker": s.speaker,
                    "start_ms": s.start_ms,
                    "end_ms": s.end_ms,
                    "text": s.text,
                })).collect::<Vec<_>>(),
                "chat": chat_messages.iter().map(|m| serde_json::json!({
                    "role": m.role,
                    "content": m.content,
                })).collect::<Vec<_>>(),
            });
            output::write_json(out, &value).await
        }
        DocFormat::Markdown => {
            let mut buf = String::new();
            let title = meeting.title.as_deref().unwrap_or("Untitled");
            buf.push_str(&format!("# {title}\n\n"));
            buf.push_str(&format!("**Date:** {}\n", meeting.created_at));
            if !participant_names.is_empty() {
                buf.push_str(&format!(
                    "**Participants:** {}\n",
                    participant_names.join(", ")
                ));
            }

            if !notes.is_empty() {
                buf.push_str("\n## Notes\n\n");
                for note in &notes {
                    if !note.title.is_empty() {
                        buf.push_str(&format!("### {}\n\n", note.title));
                    }
                    buf.push_str(&note.content);
                    buf.push_str("\n\n");
                }
            }

            if !segments.is_empty() {
                buf.push_str("## Transcript\n\n");
                for seg in &segments {
                    let ts = output::format_timestamp_ms(seg.start_ms);
                    buf.push_str(&format!("**{}** ({ts}): {}\n\n", seg.speaker, seg.text));
                }
            }

            if !chat_messages.is_empty() {
                buf.push_str("## Chat\n\n");
                for msg in &chat_messages {
                    let role = capitalize(&msg.role);
                    buf.push_str(&format!("**{role}:** {}\n\n", msg.content));
                }
            }

            output::write_text(out, buf).await
        }
        DocFormat::Text => {
            let mut buf = String::new();
            let title = meeting.title.as_deref().unwrap_or("Untitled");
            buf.push_str(&format!("{title}\n{}\n\n", meeting.created_at));

            for note in &notes {
                if !note.title.is_empty() {
                    buf.push_str(&format!("[{}]\n", note.title));
                }
                buf.push_str(&note.content);
                buf.push_str("\n\n");
            }

            for seg in &segments {
                let ts = output::format_timestamp_ms(seg.start_ms);
                buf.push_str(&format!("{} ({ts}): {}\n", seg.speaker, seg.text));
            }

            if !chat_messages.is_empty() {
                buf.push('\n');
                for msg in &chat_messages {
                    buf.push_str(&format!("{}: {}\n", msg.role, msg.content));
                }
            }

            output::write_text(out, buf).await
        }
    }
}
