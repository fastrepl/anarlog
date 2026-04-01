use std::path::Path;

use sqlx::SqlitePool;

use super::DocFormat;
use crate::error::CliResult;
use crate::{db, output};

pub(super) async fn notes(
    pool: &SqlitePool,
    meeting_id: &str,
    format: DocFormat,
    out: Option<&Path>,
) -> CliResult<()> {
    let rows = db!(
        hypr_db_app::list_notes_by_meeting(pool, meeting_id),
        "list notes"
    );

    match format {
        DocFormat::Json => {
            let value: Vec<_> = rows
                .iter()
                .map(|n| {
                    serde_json::json!({
                        "kind": n.kind,
                        "title": n.title,
                        "content": n.content,
                    })
                })
                .collect();
            output::write_json(out, &value).await
        }
        DocFormat::Markdown => {
            let mut buf = String::new();
            for note in &rows {
                if !note.title.is_empty() {
                    buf.push_str(&format!("## {}\n\n", note.title));
                }
                buf.push_str(&note.content);
                buf.push_str("\n\n");
            }
            output::write_text(out, buf).await
        }
        DocFormat::Text => {
            let mut buf = String::new();
            for note in &rows {
                if !note.title.is_empty() {
                    buf.push_str(&format!("[{}]\n", note.title));
                }
                buf.push_str(&note.content);
                buf.push_str("\n\n");
            }
            output::write_text(out, buf).await
        }
    }
}
