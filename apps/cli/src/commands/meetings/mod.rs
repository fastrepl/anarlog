use clap::Subcommand;
use sqlx::SqlitePool;

use crate::error::{CliError, CliResult};

#[derive(Subcommand)]
pub enum Commands {
    /// List meetings
    List,
    /// View a specific meeting
    View {
        #[arg(long)]
        id: String,
    },
    /// List participants in a meeting
    Participants {
        #[arg(long)]
        id: String,
    },
    /// Add a participant to a meeting
    AddParticipant {
        #[arg(long)]
        meeting: String,
        #[arg(long)]
        human: String,
    },
    /// Remove a participant from a meeting
    RmParticipant {
        #[arg(long)]
        meeting: String,
        #[arg(long)]
        human: String,
    },
}

pub async fn run(pool: &SqlitePool, command: Commands) -> CliResult<()> {
    match command {
        Commands::List => list(pool).await,
        Commands::View { id } => view(pool, &id).await,
        Commands::Participants { id } => participants(pool, &id).await,
        Commands::AddParticipant { meeting, human } => {
            add_participant(pool, &meeting, &human).await
        }
        Commands::RmParticipant { meeting, human } => {
            remove_participant(pool, &meeting, &human).await
        }
    }
}

async fn list(pool: &SqlitePool) -> CliResult<()> {
    let meetings = hypr_db_app::list_meetings(pool)
        .await
        .map_err(|e| CliError::operation_failed("list meetings", e.to_string()))?;

    let rows: Vec<serde_json::Value> = meetings
        .into_iter()
        .map(|m| {
            serde_json::json!({
                "id": m.id,
                "title": m.title,
                "created_at": m.created_at,
            })
        })
        .collect();

    crate::output::write_json(None, &rows).await
}

async fn view(pool: &SqlitePool, meeting_id: &str) -> CliResult<()> {
    let meeting = hypr_db_app::get_meeting(pool, meeting_id)
        .await
        .map_err(|e| CliError::operation_failed("get meeting", e.to_string()))?
        .ok_or_else(|| CliError::not_found(format!("meeting '{meeting_id}'"), None))?;

    let words = hypr_db_app::load_words(pool, meeting_id)
        .await
        .unwrap_or_default();

    let notes = hypr_db_app::list_notes_by_meeting(pool, meeting_id)
        .await
        .unwrap_or_default();

    let participants = hypr_db_app::list_meeting_participants(pool, meeting_id)
        .await
        .unwrap_or_default();

    let result = serde_json::json!({
        "id": meeting.id,
        "title": meeting.title,
        "created_at": meeting.created_at,
        "words": words.len(),
        "notes": notes.iter().map(|n| serde_json::json!({
            "id": n.id,
            "kind": n.kind,
            "title": n.title,
            "content": n.content,
        })).collect::<Vec<_>>(),
        "participants": participants.iter().map(|p| serde_json::json!({
            "human_id": p.human_id,
            "source": p.source,
        })).collect::<Vec<_>>(),
    });

    crate::output::write_json(None, &result).await
}

async fn participants(pool: &SqlitePool, meeting_id: &str) -> CliResult<()> {
    let rows = hypr_db_app::list_meeting_participants(pool, meeting_id)
        .await
        .map_err(|e| CliError::operation_failed("query", e.to_string()))?;

    let result: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "human_id": r.human_id,
                "source": r.source,
            })
        })
        .collect();

    crate::output::write_json(None, &result).await
}

async fn add_participant(pool: &SqlitePool, meeting_id: &str, human_id: &str) -> CliResult<()> {
    hypr_db_app::add_meeting_participant(pool, meeting_id, human_id, "manual")
        .await
        .map_err(|e| CliError::operation_failed("add participant", e.to_string()))?;
    let result = serde_json::json!({ "added": human_id, "meeting": meeting_id });
    crate::output::write_json(None, &result).await
}

async fn remove_participant(pool: &SqlitePool, meeting_id: &str, human_id: &str) -> CliResult<()> {
    hypr_db_app::remove_meeting_participant(pool, meeting_id, human_id)
        .await
        .map_err(|e| CliError::operation_failed("remove participant", e.to_string()))?;
    let result = serde_json::json!({ "removed": human_id, "meeting": meeting_id });
    crate::output::write_json(None, &result).await
}
