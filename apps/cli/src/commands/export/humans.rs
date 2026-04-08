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
        key: "name",
        header: "name",
    },
    TableColumn {
        key: "email",
        header: "email",
    },
    TableColumn {
        key: "job_title",
        header: "job_title",
    },
    TableColumn {
        key: "org_id",
        header: "org_id",
    },
];

pub(super) async fn humans(
    pool: &SqlitePool,
    format: TableFormat,
    out: Option<&Path>,
) -> CliResult<()> {
    let rows = db!(hypr_db_app::list_humans(pool), "list humans");
    let rows: Vec<Vec<TableCell>> = rows
        .iter()
        .map(|human| {
            vec![
                TableCell::new(serde_json::json!(human.id), human.id.clone()),
                TableCell::new(serde_json::json!(human.name), human.name.clone()),
                TableCell::new(serde_json::json!(human.email), human.email.clone()),
                TableCell::new(serde_json::json!(human.job_title), human.job_title.clone()),
                TableCell::new(serde_json::json!(human.org_id), human.org_id.clone()),
            ]
        })
        .collect();

    write_table(format, out, COLUMNS, &rows).await
}
