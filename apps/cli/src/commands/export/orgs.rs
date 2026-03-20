use std::path::Path;

use sqlx::SqlitePool;

use super::TableFormat;
use super::helpers::csv_escape;
use crate::error::CliResult;
use crate::{db, output};

pub(super) async fn orgs(
    pool: &SqlitePool,
    format: TableFormat,
    out: Option<&Path>,
) -> CliResult<()> {
    let rows = db!(hypr_db_app::list_organizations(pool), "list organizations");

    match format {
        TableFormat::Json => {
            let value: Vec<_> = rows
                .iter()
                .map(|o| {
                    serde_json::json!({
                        "id": o.id,
                        "name": o.name,
                        "created_at": o.created_at,
                    })
                })
                .collect();
            output::write_json(out, &value).await
        }
        TableFormat::Csv => {
            let mut buf = String::from("id,name,created_at\n");
            for o in &rows {
                buf.push_str(&format!(
                    "{},{},{}\n",
                    o.id,
                    csv_escape(&o.name),
                    o.created_at,
                ));
            }
            output::write_text(out, buf).await
        }
        TableFormat::Text => {
            let mut buf = String::new();
            for o in &rows {
                buf.push_str(&format!("{}\t{}\n", o.id, o.name));
            }
            output::write_text(out, buf).await
        }
    }
}
