#![forbid(unsafe_code)]

mod cli;
mod commands;
mod context;
mod db;
mod error;
mod mcp;
mod output;

pub use cli::Args;
pub use error::{Error, Result};

pub async fn run(args: Args) -> Result<()> {
    let db = std::sync::Arc::new(db::open(&args).await?);

    match args.command {
        cli::Command::Meetings { command } => {
            commands::meetings::run(db.as_ref(), command, args.json).await
        }
        cli::Command::Mcp => mcp::serve(db).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn export_command_reads_existing_database_without_migrating_it() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");
        let output_path = dir.path().join("meeting.md");
        let db = hypr_db_core::Db::connect_local_plain(&db_path).await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, title, started_at) VALUES ('meeting-1', 'Planning', '2026-07-13')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO session_documents (id, session_id, kind, body_format, body)
             VALUES ('meeting-1', 'meeting-1', 'note', 'markdown', 'Decide the launch date.')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db.pool().close().await;

        run(Args {
            base: None,
            db_path: Some(db_path),
            json: false,
            command: cli::Command::Meetings {
                command: cli::MeetingCommand::Export {
                    id: "meeting-1".to_string(),
                    format: cli::ExportFormat::Markdown,
                    output: Some(output_path.clone()),
                },
            },
        })
        .await
        .unwrap();

        let exported = std::fs::read_to_string(output_path).unwrap();
        assert!(exported.contains("# Planning"));
        assert!(exported.contains("Decide the launch date."));
    }
}
