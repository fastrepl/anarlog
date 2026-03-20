use std::path::Path;

use sqlx::SqlitePool;

use super::TableFormat;
use super::helpers::csv_escape;
use crate::error::CliResult;
use crate::{db, output};

pub(super) async fn humans(
    pool: &SqlitePool,
    format: TableFormat,
    out: Option<&Path>,
) -> CliResult<()> {
    let rows = db!(hypr_db_app::list_humans(pool), "list humans");

    match format {
        TableFormat::Json => {
            let value: Vec<_> = rows
                .iter()
                .map(|h| {
                    serde_json::json!({
                        "id": h.id,
                        "name": h.name,
                        "email": h.email,
                        "job_title": h.job_title,
                        "org_id": h.org_id,
                    })
                })
                .collect();
            output::write_json(out, &value).await
        }
        TableFormat::Csv => {
            let mut buf = String::from("id,name,email,job_title,org_id\n");
            for h in &rows {
                buf.push_str(&format!(
                    "{},{},{},{},{}\n",
                    h.id,
                    csv_escape(&h.name),
                    csv_escape(&h.email),
                    csv_escape(&h.job_title),
                    h.org_id,
                ));
            }
            output::write_text(out, buf).await
        }
        TableFormat::Text => {
            let mut buf = String::new();
            for h in &rows {
                buf.push_str(&format!("{}\t{}\t{}\n", h.id, h.name, h.email));
            }
            output::write_text(out, buf).await
        }
    }
}
