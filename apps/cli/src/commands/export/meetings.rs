use std::path::Path;

use sqlx::SqlitePool;

use super::TableFormat;
use super::helpers::csv_escape;
use crate::error::CliResult;
use crate::{db, output};

pub(super) async fn meetings(
    pool: &SqlitePool,
    format: TableFormat,
    out: Option<&Path>,
) -> CliResult<()> {
    let rows = db!(hypr_db_app::list_meetings(pool), "list meetings");

    match format {
        TableFormat::Json => {
            let value: Vec<_> = rows
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "id": m.id,
                        "title": m.title,
                        "created_at": m.created_at,
                    })
                })
                .collect();
            output::write_json(out, &value).await
        }
        TableFormat::Csv => {
            let mut buf = String::from("id,title,created_at\n");
            for m in &rows {
                let title = csv_escape(m.title.as_deref().unwrap_or(""));
                buf.push_str(&format!("{},{},{}\n", m.id, title, m.created_at));
            }
            output::write_text(out, buf).await
        }
        TableFormat::Text => {
            let mut buf = String::new();
            for m in &rows {
                let title = m.title.as_deref().unwrap_or("");
                buf.push_str(&format!("{}\t{}\t{}\n", m.id, title, m.created_at));
            }
            output::write_text(out, buf).await
        }
    }
}
