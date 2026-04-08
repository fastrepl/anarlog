use std::path::Path;

use sqlx::SqlitePool;

use super::TableFormat;
use super::helpers::{TableCell, TableColumn, write_table};
use crate::db;
use crate::error::CliResult;

const COLUMNS: &[TableColumn] = &[
    TableColumn {
        key: "id",
        header: "id",
    },
    TableColumn {
        key: "title",
        header: "title",
    },
    TableColumn {
        key: "created_at",
        header: "created_at",
    },
];

pub(super) async fn meetings(
    pool: &SqlitePool,
    format: TableFormat,
    out: Option<&Path>,
) -> CliResult<()> {
    let rows = db!(hypr_db_app::list_meetings(pool), "list meetings");
    let rows: Vec<Vec<TableCell>> = rows
        .iter()
        .map(|meeting| {
            vec![
                TableCell::new(serde_json::json!(meeting.id), meeting.id.clone()),
                TableCell::new(
                    serde_json::json!(meeting.title),
                    meeting.title.as_deref().unwrap_or(""),
                ),
                TableCell::new(
                    serde_json::json!(meeting.created_at),
                    meeting.created_at.to_string(),
                ),
            ]
        })
        .collect();

    write_table(format, out, COLUMNS, &rows).await
}
