use std::path::Path;

use sqlx::SqlitePool;

use super::DocFormat;
use super::helpers::capitalize;
use crate::error::CliResult;
use crate::{db, output};

pub(super) async fn chat(
    pool: &SqlitePool,
    meeting_id: &str,
    format: DocFormat,
    out: Option<&Path>,
) -> CliResult<()> {
    let rows = db!(
        hypr_db_app::load_chat_messages(pool, meeting_id),
        "load chat messages"
    );

    match format {
        DocFormat::Json => {
            let value: Vec<_> = rows
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "role": m.role,
                        "content": m.content,
                        "created_at": m.created_at,
                    })
                })
                .collect();
            output::write_json(out, &value).await
        }
        DocFormat::Markdown => {
            let mut buf = String::new();
            for msg in &rows {
                let role = capitalize(&msg.role);
                buf.push_str(&format!("**{role}:** {}\n\n", msg.content));
            }
            output::write_text(out, buf).await
        }
        DocFormat::Text => {
            let mut buf = String::new();
            for msg in &rows {
                buf.push_str(&format!("{}: {}\n", msg.role, msg.content));
            }
            output::write_text(out, buf).await
        }
    }
}
