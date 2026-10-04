#![forbid(unsafe_code)]

mod cli;
mod cloud;
mod commands;
mod db;
mod error;
mod mcp;
mod output;

pub use cli::Args;
pub use error::{Error, Result};
pub use output::JSON_SCHEMA_VERSION;

pub const VERSION: &str = match option_env!("APP_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

pub async fn run(mut args: Args) -> Result<u8> {
    if let cli::Command::Auth { command } = &args.command {
        commands::auth::run(command, args.json).await?;
        return Ok(0);
    }

    if matches!(&args.command, cli::Command::Doctor) {
        let ready = commands::doctor::run(&args, args.json).await?;
        return Ok(if ready { 0 } else { 1 });
    }

    let json = args.json;
    let command = std::mem::replace(&mut args.command, cli::Command::Doctor);
    match command {
        cli::Command::Auth { .. } => unreachable!("auth returns before opening the database"),
        cli::Command::Doctor => unreachable!("doctor returns before opening the database"),
        cli::Command::Meetings { source, command } => {
            if matches!(&command, cli::MeetingCommand::PilotUsage { .. })
                && !matches!(source, cli::MeetingSource::Local)
            {
                return Err(Error::operation(
                    "read pilot capture usage",
                    "explicit local source required",
                ));
            }
            if let cli::MeetingCommand::Delete {
                id,
                if_export_sha256,
                require_transcription_complete,
            } = &command
            {
                if !matches!(source, cli::MeetingSource::Local) || !require_transcription_complete {
                    return Err(Error::operation(
                        "delete meeting",
                        "explicit local source and completion check required",
                    ));
                }
                commands::meeting_delete::run(&args, id, if_export_sha256, json).await?;
                return Ok(0);
            }
            if matches!(&command, cli::MeetingCommand::InitializeManaged) {
                if !matches!(source, cli::MeetingSource::Local) {
                    return Err(Error::operation(
                        "initialize managed recorder",
                        "explicit local source required",
                    ));
                }
                commands::meeting_delete::initialize_managed(&args, json).await?;
                return Ok(0);
            }
            if matches!(&command, cli::MeetingCommand::Capabilities) {
                output::emit(&output::json(
                    "meetings.capabilities",
                    &serde_json::json!({"recorder_version":VERSION,"managed_initialization":1,"managed_settings":1,"intelligence_disabled":true,"conditional_delete":1,"attachment_export":1,"calendar_identity_export":1,"hold_inventory_export":1,"hold_orphan_inventory":1,"hold_filesystem_inventory":1,"hold_resumable_filesystem_inventory":1,"capture_completion":1,"pilot_usage":1}),
                    None,
                )?);
                return Ok(0);
            }
            let source = commands::meetings::DataSource::open(&args, source).await?;
            commands::meetings::run(&source, command, json).await?
        }
        cli::Command::Proposals { command } => {
            let db = db::open_write(&args).await?;
            commands::proposals::run(&db, command, json).await?
        }
        cli::Command::Mcp => {
            let db = std::sync::Arc::new(db::open_write(&args).await?);
            mcp::serve(db).await?
        }
    }

    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn doctor_returns_nonzero_status_when_database_is_not_ready() {
        let dir = tempfile::tempdir().unwrap();
        let status = run(Args {
            base: None,
            db_path: Some(dir.path().join("missing.db")),
            json: true,
            command: cli::Command::Doctor,
        })
        .await
        .unwrap();

        assert_eq!(status, 1);
    }

    #[tokio::test]
    async fn export_command_reads_existing_database_without_migrating_it() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");
        let output_path = dir.path().join("meeting.md");
        let db = anlg_db_core::Db::connect_local_plain(&db_path)
            .await
            .unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
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
                source: cli::MeetingSource::Local,
                command: cli::MeetingCommand::Export {
                    id: "meeting-1".to_string(),
                    format: cli::ExportFormat::Markdown,
                    output: Some(output_path.clone()),
                    force: false,
                },
            },
        })
        .await
        .unwrap();

        let exported = std::fs::read_to_string(output_path).unwrap();
        assert!(exported.contains("# Planning"));
        assert!(exported.contains("Decide the launch date."));
    }

    #[tokio::test]
    async fn proposal_create_lists_and_declines_without_changing_the_note() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");
        let db = anlg_db_core::Db::connect_local_plain(&db_path)
            .await
            .unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, title, started_at) VALUES ('meeting-1', 'Planning', '2026-07-13')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO session_documents (id, session_id, kind, body_format, body)
             VALUES
             ('meeting-1', 'meeting-1', 'note', 'markdown', 'Original memo'),
             ('summary-1', 'meeting-1', 'summary', 'markdown', 'Original summary')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db.pool().close().await;

        run(Args {
            base: None,
            db_path: Some(db_path.clone()),
            json: true,
            command: cli::Command::Proposals {
                command: cli::ProposalCommand::Create {
                    meeting_id: "meeting-1".to_string(),
                    kind: cli::ProposalKind::Summary,
                    target_id: None,
                    content: Some("Revised summary".to_string()),
                    content_file: None,
                },
            },
        })
        .await
        .unwrap();

        let read = anlg_db_core::Db::connect_local_read_only(&db_path)
            .await
            .unwrap();
        let pending: Vec<(String, String)> =
            sqlx::query_as("SELECT status, proposed_markdown FROM session_proposals")
                .fetch_all(read.pool())
                .await
                .unwrap();
        let note: String =
            sqlx::query_scalar("SELECT body FROM session_documents WHERE id = 'summary-1'")
                .fetch_one(read.pool())
                .await
                .unwrap();
        read.pool().close().await;

        assert_eq!(
            pending,
            vec![("pending".to_string(), "Revised summary".to_string())]
        );
        assert_eq!(note, "Original summary");
    }
}
