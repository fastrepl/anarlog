use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use crate::transaction_utils::{js_iso8601_timestamp, rollback_row_count_mismatch};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct MoveSessionContentsRequest {
    pub source_session_id: String,
    pub target_session_id: String,
    pub rewrite_audio_ids: bool,
    pub next_target_note: Option<String>,
    pub empty_source_note: Option<String>,
}

pub async fn move_session_contents(
    pool: &SqlitePool,
    request: MoveSessionContentsRequest,
) -> Result<(), String> {
    let now = js_iso8601_timestamp();
    let source_audio_id = format!("session-audio:{}", request.source_session_id);
    let target_audio_id = format!("session-audio:{}", request.target_session_id);
    let move_request = anlg_db_app::SessionAudioMove {
        source_session_id: &request.source_session_id,
        target_session_id: &request.target_session_id,
        rewrite_audio_ids: request.rewrite_audio_ids,
        source_audio_id: &source_audio_id,
        target_audio_id: &target_audio_id,
        now: &now,
    };
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    anlg_db_app::move_session_transcripts(&mut transaction, &move_request)
        .await
        .map_err(|error| error.to_string())?;
    anlg_db_app::move_session_summary_documents(
        &mut transaction,
        &request.source_session_id,
        &request.target_session_id,
        &now,
    )
    .await
    .map_err(|error| error.to_string())?;
    anlg_db_app::move_session_action_items(
        &mut transaction,
        &request.source_session_id,
        &request.target_session_id,
        &now,
    )
    .await
    .map_err(|error| error.to_string())?;
    anlg_db_app::move_session_voiceprint_exemplars(&mut transaction, &move_request)
        .await
        .map_err(|error| error.to_string())?;
    anlg_db_app::move_session_voiceprint_candidates(&mut transaction, &move_request)
        .await
        .map_err(|error| error.to_string())?;

    if let Some(next_target_note) = &request.next_target_note {
        let updated = anlg_db_app::rewrite_session_note_body(
            &mut transaction,
            &request.target_session_id,
            &request.target_session_id,
            next_target_note,
            &now,
        )
        .await
        .map_err(|error| error.to_string())?;
        if updated != 1 {
            return Err(rollback_row_count_mismatch(transaction, 5, updated, 1).await);
        }
        let updated = anlg_db_app::rewrite_session_note_body(
            &mut transaction,
            &request.source_session_id,
            &request.source_session_id,
            request.empty_source_note.as_deref().unwrap_or_default(),
            &now,
        )
        .await
        .map_err(|error| error.to_string())?;
        if updated != 1 {
            return Err(rollback_row_count_mismatch(transaction, 6, updated, 1).await);
        }
    }

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}
