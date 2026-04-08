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
        key: "created_at",
        header: "created_at",
    },
];

pub(super) async fn orgs(
    pool: &SqlitePool,
    format: TableFormat,
    out: Option<&Path>,
) -> CliResult<()> {
    let rows = db!(hypr_db_app::list_organizations(pool), "list organizations");
    let rows: Vec<Vec<TableCell>> = rows
        .iter()
        .map(|org| {
            vec![
                TableCell::new(serde_json::json!(org.id), org.id.clone()),
                TableCell::new(serde_json::json!(org.name), org.name.clone()),
                TableCell::new(
                    serde_json::json!(org.created_at),
                    org.created_at.to_string(),
                ),
            ]
        })
        .collect();

    write_table(format, out, COLUMNS, &rows).await
}
