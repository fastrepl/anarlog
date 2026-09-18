//! `session/move-contents.ts`: `moveSessionContents` — move a finished
//! recording, transcripts, summaries, notes, and action items from one
//! session onto another, with the same result shapes the chat tool returns.

use serde_json::{Value, json};

use super::Store;

fn error(message: &str, source: &str, target: &str) -> Value {
    json!({
        "status": "error",
        "message": message,
        "sourceMeetingId": source,
        "targetMeetingId": target
    })
}

/// `hasNoteContent`
fn has_note_content(markdown: &str) -> bool {
    let trimmed = markdown.trim();
    !trimmed.is_empty() && trimmed != "&nbsp;"
}

impl Store {
    /// `moveSessionContents({ sourceSessionId, targetSessionId })`; `busy`
    /// is `isSessionBusy` for either session, decided by the caller who
    /// knows the capture state.
    pub fn move_session_contents(
        self: &std::sync::Arc<Self>,
        source_id: String,
        target_id: String,
        busy: bool,
    ) -> tokio::task::JoinHandle<Value> {
        let db = self.db.clone();
        let store = self.clone();
        self.runtime.spawn(async move {
            if source_id.is_empty() || target_id.is_empty() {
                let mut value = json!({
                    "status": "error",
                    "message": "Both the source and target meetings are required."
                });
                if !source_id.is_empty() {
                    value["sourceMeetingId"] = source_id.clone().into();
                }
                if !target_id.is_empty() {
                    value["targetMeetingId"] = target_id.clone().into();
                }
                return value;
            }
            if source_id == target_id {
                return error("Choose two different meetings.", &source_id, &target_id);
            }
            if busy {
                return error(
                    "Wait until recording and transcription finish on both meetings, then try again.",
                    &source_id,
                    &target_id,
                );
            }
            let pool = db.pool();
            let source = match super::enhancer::load_snapshot(pool, &source_id).await {
                Ok(Some(snapshot)) => snapshot,
                _ => {
                    return error("The source meeting could not be loaded.", &source_id, &target_id);
                }
            };
            let target = match super::enhancer::load_snapshot(pool, &target_id).await {
                Ok(Some(snapshot)) => snapshot,
                _ => {
                    return error("The target meeting could not be loaded.", &source_id, &target_id);
                }
            };
            let source_dir = store.session_dir(&source_id);
            let target_dir = store.session_dir(&target_id);
            let source_has_audio = anlg_fs_sync_core::audio::exists(&source_dir).unwrap_or(false);
            let target_has_audio = anlg_fs_sync_core::audio::exists(&target_dir).unwrap_or(false);
            let source_action_items: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM action_items WHERE session_id = ? AND deleted_at IS NULL",
            )
            .bind(&source_id)
            .fetch_one(pool)
            .await
            .unwrap_or(0);
            let source_has_notes = has_note_content(&source.raw_markdown);
            let has_anything = source_has_audio
                || !source.transcripts.is_empty()
                || !source.enhanced_notes.is_empty()
                || source_has_notes
                || source_action_items > 0;
            if !has_anything {
                return json!({
                    "status": "nothing_to_move",
                    "message": "The source meeting has no recording, transcript, or notes to move.",
                    "sourceMeetingId": source_id,
                    "targetMeetingId": target_id
                });
            }
            if target_has_audio || !target.transcripts.is_empty() {
                return error(
                    "The target meeting already has a recording or transcript. Move into an empty meeting, or delete the existing recording first.",
                    &source_id,
                    &target_id,
                );
            }

            let mut copied_audio = false;
            let outcome: anyhow::Result<()> = async {
                if source_has_audio {
                    let (source_dir, target_dir) = (source_dir.clone(), target_dir.clone());
                    copied_audio = tokio::task::spawn_blocking(move || {
                        anlg_fs_sync_core::audio::copy(&source_dir, &target_dir)
                    })
                    .await??;
                    if copied_audio {
                        store.catalog_session_audio(target_id.clone()).await??;
                    }
                }
                let now = chrono::Utc::now()
                    .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                    .to_string();
                let source_audio_id = format!("session-audio:{source_id}");
                let target_audio_id = format!("session-audio:{target_id}");
                let rewrite = i64::from(copied_audio);
                let target_has_notes = has_note_content(&target.raw_markdown);
                let next_target_note = if source_has_notes {
                    Some(if target_has_notes {
                        crate::document::md2json(&format!(
                            "{}\n\n{}",
                            target.raw_markdown.trim(),
                            source.raw_markdown.trim()
                        ))
                    } else if source.raw_content_format == "prosemirror_json"
                        && !source.raw_content.is_empty()
                    {
                        source.raw_content.clone()
                    } else {
                        crate::document::md2json(&source.raw_markdown)
                    })
                } else {
                    None
                };
                let mut tx = pool.begin().await?;
                sqlx::query(
                    "UPDATE transcripts
                     SET session_id = ?,
                         audio_attachment_id = CASE
                           WHEN ? = 1 AND audio_attachment_id = ? THEN ?
                           ELSE audio_attachment_id
                         END,
                         updated_at = ?
                     WHERE session_id = ? AND deleted_at IS NULL",
                )
                .bind(&target_id)
                .bind(rewrite)
                .bind(&source_audio_id)
                .bind(&target_audio_id)
                .bind(&now)
                .bind(&source_id)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "UPDATE session_documents
                     SET session_id = ?, updated_at = ?
                     WHERE session_id = ?
                       AND kind IN ('summary', 'template_output')
                       AND deleted_at IS NULL",
                )
                .bind(&target_id)
                .bind(&now)
                .bind(&source_id)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "UPDATE action_items
                     SET session_id = ?, updated_at = ?
                     WHERE session_id = ? AND deleted_at IS NULL",
                )
                .bind(&target_id)
                .bind(&now)
                .bind(&source_id)
                .execute(&mut *tx)
                .await?;
                for table in ["voiceprint_exemplars", "voiceprint_candidates"] {
                    sqlx::query(sqlx::AssertSqlSafe(format!(
                        "UPDATE {table}
                         SET source_session_id = ?,
                             source_attachment_id = CASE
                               WHEN ? = 1 AND source_attachment_id = ? THEN ?
                               ELSE source_attachment_id
                             END,
                             updated_at = ?
                         WHERE source_session_id = ? AND deleted_at IS NULL"
                    )))
                    .bind(&target_id)
                    .bind(rewrite)
                    .bind(&source_audio_id)
                    .bind(&target_audio_id)
                    .bind(&now)
                    .bind(&source_id)
                    .execute(&mut *tx)
                    .await?;
                }
                if let Some(next_target_note) = next_target_note {
                    for (body, session_id) in [
                        (next_target_note, &target_id),
                        (crate::document::md2json(""), &source_id),
                    ] {
                        let affected = sqlx::query(
                            "UPDATE session_documents
                             SET body = ?, body_format = 'prosemirror_json', updated_at = ?
                             WHERE id = ? AND session_id = ? AND kind = 'note' AND deleted_at IS NULL",
                        )
                        .bind(&body)
                        .bind(&now)
                        .bind(session_id)
                        .bind(session_id)
                        .execute(&mut *tx)
                        .await?
                        .rows_affected();
                        anyhow::ensure!(affected == 1, "expected one note row for {session_id}");
                    }
                }
                tx.commit().await?;
                Ok(())
            }
            .await;

            if let Err(error) = outcome {
                tracing::error!(%error, "Failed to move meeting contents");
                if copied_audio {
                    // `rollbackCopiedAudio`
                    if let Ok(Err(error)) = store.delete_session_audio(target_id.clone()).await {
                        tracing::error!(%error, "failed to roll back copied recording");
                    }
                }
                return json!({
                    "status": "error",
                    "message": "The move could not be completed. Nothing was changed.",
                    "sourceMeetingId": source_id,
                    "targetMeetingId": target_id
                });
            }
            if copied_audio
                && let Ok(Err(error)) = store.delete_session_audio(source_id.clone()).await
            {
                tracing::error!(%error, "moved recording but failed to remove the source file");
            }
            json!({
                "status": "moved",
                "sourceMeetingId": source_id,
                "targetMeetingId": target_id,
                "sourceTitle": source.title,
                "targetTitle": target.title,
                "moved": {
                    "recording": copied_audio,
                    "transcripts": source.transcripts.len(),
                    "summaries": source.enhanced_notes.len(),
                    "notes": source_has_notes,
                    "actionItems": source_action_items
                }
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_content_and_empty_bodies_follow_the_frontend() {
        assert!(!has_note_content("  &nbsp; "));
        assert!(has_note_content("Hi"));
        assert_eq!(
            crate::document::md2json(""),
            r#"{"type":"doc","content":[{"type":"paragraph"}]}"#
        );
        assert!(crate::document::md2json("Hello").contains(r#""text":"Hello""#));
    }
}
