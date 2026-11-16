use std::{path::PathBuf, sync::Arc};

use desktop_runtime::{CancellationToken, Reply, Result, RuntimeHandle, SessionId};
use serde_json::json;

use super::mutations::{NOW, failure, statement, transaction};

#[derive(Clone)]
pub enum NoteCommand {
    Delete(Arc<[SessionId]>),
    Move {
        ids: Arc<[SessionId]>,
        folder: Arc<str>,
    },
}

pub fn directory(runtime: &RuntimeHandle, session: SessionId) -> Result<Reply<PathBuf>> {
    runtime.read(CancellationToken::new(), move |services| async move {
        let rows = services
            .executor
            .execute(
                "SELECT id FROM sessions WHERE id=? AND deleted_at IS NULL AND locked=0".into(),
                vec![json!(session)],
            )
            .await
            .map_err(failure)?;
        if rows.is_empty() {
            return Err(failure("This note is unavailable or locked."));
        }
        let fs = super::folders::filesystem(&services.executor).await?;
        tokio::task::spawn_blocking(move || {
            let path = fs.resolve_session_dir(&session.0).map_err(failure)?;
            let metadata = path
                .metadata()
                .map_err(|error| failure(format!("Note folder is unavailable: {error}")))?;
            if !metadata.is_dir() {
                return Err(failure("The note folder path is not a directory."));
            }
            Ok(path)
        })
        .await
        .map_err(failure)?
    })
}

pub fn select_template(
    runtime: &RuntimeHandle,
    session: SessionId,
    template: Arc<str>,
) -> Result<Reply<String>> {
    runtime.submit(move |services| async move {
        transaction(&services.executor,vec![statement(format!("UPDATE session_documents SET template_id=?1,updated_at={NOW} WHERE id=(SELECT id FROM session_documents WHERE session_id=?2 AND kind='note' AND deleted_at IS NULL ORDER BY CASE WHEN id=?2 THEN 0 ELSE 1 END,created_at,id LIMIT 1) AND EXISTS(SELECT 1 FROM templates WHERE id=?1) AND EXISTS(SELECT 1 FROM sessions WHERE id=?2 AND deleted_at IS NULL AND locked=0)"),vec![json!(template),json!(session.0)],Some(1))]).await?;
        Ok("Template selected for this note.".into())
    })
}

pub fn dispatch(runtime: &RuntimeHandle, command: NoteCommand) -> Result<Reply<()>> {
    runtime.submit(move |services| async move {
        let (ids,destination) = match command {
            NoteCommand::Delete(ids) => (ids,None),
            NoteCommand::Move { ids,folder } => (ids,Some(folder)),
        };
        if ids.is_empty() || ids.len()>100 { return Err(failure("Select between 1 and 100 notes")); }
        let values = ids.iter().map(|id| json!(id.0)).collect::<Vec<_>>();
        if let Some(destination) = destination { return super::folders::move_notes(&services.executor,&values,&destination).await; }
        let mut statements = Vec::new();
        for id in ids.iter() {
            statements.push(statement(format!("UPDATE sessions SET deleted_at={NOW},updated_at={NOW} WHERE id=? AND deleted_at IS NULL AND locked=0"),vec![json!(id.0)],Some(1)));
            for table in ["session_documents","transcripts","session_participants","session_tags","action_items","session_attachments"] {
                statements.push(statement(format!("UPDATE {table} SET deleted_at={NOW},updated_at={NOW} WHERE session_id=? AND deleted_at IS NULL"),vec![json!(id.0)],None));
            }
            statements.push(statement(format!("UPDATE entity_mentions SET deleted_at={NOW},updated_at={NOW} WHERE ((source_type='session' AND source_id=?1) OR (target_type='session' AND target_id=?1)) AND deleted_at IS NULL"),vec![json!(id.0)],None));
        }
        transaction(&services.executor,statements).await
    })
}
