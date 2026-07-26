use std::path::PathBuf;

use serde::Serialize;

use crate::{Args, Result, db, output};

#[derive(Debug, Serialize)]
struct DoctorReport {
    cli_version: &'static str,
    ready: bool,
    database: DatabaseReport,
}

#[derive(Debug, Serialize)]
struct DatabaseReport {
    path: PathBuf,
    exists: bool,
    opened_read_only: bool,
    schema_ready: bool,
    error: Option<String>,
}

pub async fn run(args: &Args, json: bool) -> Result<bool> {
    let report = inspect(args).await?;
    let rendered = if json {
        output::json("doctor", &report, None)?
    } else {
        render(&report)
    };
    output::emit(&rendered);
    Ok(report.ready)
}

async fn inspect(args: &Args) -> Result<DoctorReport> {
    let path = db::resolve_path(args)?;
    let exists = path.exists();
    let mut database = DatabaseReport {
        path: path.clone(),
        exists,
        opened_read_only: false,
        schema_ready: false,
        error: None,
    };

    if !exists {
        database.error = Some("database file does not exist".to_string());
    } else if !path.is_file() {
        database.error = Some("database path is not a file".to_string());
    } else {
        match hypr_db_core::Db::connect_local_read_only(&path).await {
            Ok(connection) => {
                database.opened_read_only = true;
                match schema_check(&connection).await {
                    Ok(()) => database.schema_ready = true,
                    Err(error) => database.error = Some(error),
                }
            }
            Err(error) => database.error = Some(format!("open database failed: {error}")),
        }
    }

    Ok(DoctorReport {
        cli_version: env!("CARGO_PKG_VERSION"),
        ready: database.opened_read_only && database.schema_ready,
        database,
    })
}

async fn schema_check(db: &hypr_db_core::Db) -> std::result::Result<(), String> {
    tokio::try_join!(
        hypr_db_app::get_session(db.pool(), "__anarlog_doctor__"),
        hypr_db_app::list_session_documents(db.pool(), "__anarlog_doctor__"),
        hypr_db_app::list_session_transcripts(db.pool(), "__anarlog_doctor__"),
        hypr_db_app::list_session_participants(db.pool(), "__anarlog_doctor__"),
        hypr_db_app::list_session_action_items(db.pool(), "__anarlog_doctor__"),
    )
    .map(|_| ())
    .map_err(|error| format!("schema check failed: {error}"))
}

fn render(report: &DoctorReport) -> String {
    let status = |value| if value { "yes" } else { "no" };
    let mut lines = vec![
        format!("Anarlog CLI {}", report.cli_version),
        format!("Ready: {}", status(report.ready)),
        format!("Database: {}", report.database.path.display()),
        format!("Exists: {}", status(report.database.exists)),
        format!(
            "Opened read-only: {}",
            status(report.database.opened_read_only)
        ),
        format!("Schema ready: {}", status(report.database.schema_ready)),
    ];
    if let Some(error) = &report.database.error {
        lines.push(format!("Issue: {error}"));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use crate::cli::Command;

    use super::*;

    fn args(path: PathBuf) -> Args {
        Args {
            base: None,
            db_path: Some(path),
            json: true,
            command: Command::Doctor,
        }
    }

    #[tokio::test]
    async fn reports_missing_database_as_not_ready() {
        let dir = tempfile::tempdir().unwrap();
        let report = inspect(&args(dir.path().join("missing.db"))).await.unwrap();

        assert!(!report.ready);
        assert!(!report.database.exists);
        assert_eq!(
            report.database.error.as_deref(),
            Some("database file does not exist")
        );
    }

    #[tokio::test]
    async fn reports_current_schema_as_ready_over_read_only_connection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.db");
        let db = hypr_db_core::Db::connect_local_plain(&path).await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        db.pool().close().await;

        let report = inspect(&args(path)).await.unwrap();

        assert!(report.ready);
        assert!(report.database.opened_read_only);
        assert!(report.database.schema_ready);
        assert!(report.database.error.is_none());
    }
}
