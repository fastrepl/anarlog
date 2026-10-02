use std::collections::HashSet;

use anlg_db_app::{
    load_render_humans, load_session_render_participant_ids, load_session_render_transcripts,
};
use anlg_transcript::{
    RenderTranscriptHuman, RenderTranscriptRequest, RenderedTranscriptSegment,
    StoredLiveTranscriptDelta, StoredSpeakerHint, materialize_live_transcript,
    parse_stored_speaker_hints, parse_stored_transcript_words, render_stored_transcript_segments,
};
use serde_json::Value;
use sqlx::SqlitePool;

#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
pub struct RenderSessionTranscriptRequest {
    pub session_id: String,
    pub self_human_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct RenderedSessionTranscript {
    pub segments: Vec<RenderedTranscriptSegment>,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
}

pub async fn render_session_transcript(
    pool: &SqlitePool,
    request: RenderSessionTranscriptRequest,
) -> Result<Option<RenderedSessionTranscript>, String> {
    let mut transaction = pool.begin().await.map_err(|error| error.to_string())?;
    let rows = load_session_render_transcripts(&mut transaction, &request.session_id)
        .await
        .map_err(|error| error.to_string())?;
    if rows.is_empty() {
        transaction
            .commit()
            .await
            .map_err(|error| error.to_string())?;
        return Ok(None);
    }

    let started_at = rows.iter().map(|transcript| transcript.started_at_ms).min();
    let ended_at = rows
        .iter()
        .filter_map(|transcript| transcript.ended_at_ms)
        .max();
    let participant_human_ids =
        load_session_render_participant_ids(&mut transaction, &request.session_id)
            .await
            .map_err(|error| error.to_string())?;
    let self_human_id = request.self_human_id.filter(|id| !id.is_empty());
    let mut human_ids = Vec::new();
    let mut seen_human_ids = HashSet::new();
    if let Some(self_id) = &self_human_id {
        insert_human_id(&mut human_ids, &mut seen_human_ids, self_id);
    }
    for participant_id in &participant_human_ids {
        insert_human_id(&mut human_ids, &mut seen_human_ids, participant_id);
    }

    let mut transcripts = Vec::new();
    for row in rows {
        let words = parse_stored_transcript_words(&row.words_json);
        let hints = parse_stored_speaker_hints(&row.speaker_hints_json);
        let deltas = row
            .pending_delta_jsons
            .iter()
            .filter_map(|json| serde_json::from_str::<StoredLiveTranscriptDelta>(json).ok())
            .collect::<Vec<_>>();
        let (words, hints) = materialize_live_transcript(words, hints, &deltas);

        for hint in &hints {
            if let Some(human_id) = assigned_human_id(hint) {
                insert_human_id(&mut human_ids, &mut seen_human_ids, &human_id);
            }
        }
        transcripts.push((Some(row.started_at_ms), words, hints));
    }

    let humans = load_render_humans(&mut transaction, &human_ids)
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|(human_id, name)| RenderTranscriptHuman { human_id, name })
        .collect();
    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())?;

    let Some(segments) = render_stored_transcript_segments(
        transcripts,
        RenderTranscriptRequest {
            transcripts: vec![],
            participant_human_ids,
            self_human_id,
            humans,
            speaker_context: None,
            preview: None,
        },
    ) else {
        return Ok(None);
    };
    Ok(Some(RenderedSessionTranscript {
        segments,
        started_at,
        ended_at,
    }))
}

fn assigned_human_id(hint: &StoredSpeakerHint) -> Option<String> {
    if hint.hint_type != "automatic_speaker_assignment"
        && hint.hint_type != "user_speaker_assignment"
    {
        return None;
    }
    let value = match &hint.value {
        Value::String(json) => serde_json::from_str::<Value>(json).ok()?,
        value => value.clone(),
    };
    let human_id = value.get("human_id")?.as_str()?;
    (!human_id.is_empty()).then(|| human_id.to_string())
}

fn insert_human_id(
    human_ids: &mut Vec<String>,
    seen_human_ids: &mut HashSet<String>,
    human_id: &str,
) {
    if !human_id.is_empty() && seen_human_ids.insert(human_id.to_string()) {
        human_ids.push(human_id.to_string());
    }
}

#[cfg(test)]
mod tests {
    use anlg_db_app::prepare_schema;
    use anlg_db_core::Db;

    use super::*;

    async fn test_db() -> Db {
        let db = Db::connect_memory_plain().await.unwrap();
        prepare_schema(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id)
             VALUES ('session-1', 'workspace-1', 'owner-human')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO humans (id, name, email)
             VALUES
                ('owner-human', 'Owner', 'owner@example.com'),
                ('speaker-human', 'Assigned Speaker', 'speaker@example.com')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO session_participants
                (id, session_id, human_id, display_name, email, source, created_at)
             VALUES ('participant', 'session-1', 'speaker-human', '', '', 'manual', '2026-01-01')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let words = serde_json::json!([{
            "id": "earlier-word",
            "text": "earlier",
            "start_ms": 0,
            "end_ms": 400,
            "channel": 0
        }]);
        sqlx::query(
            "INSERT INTO transcripts
                (id, session_id, started_at_ms, ended_at_ms, words_json, speaker_hints_json)
             VALUES ('transcript-1', 'session-1', 1000, 2000, ?, '[]')",
        )
        .bind(words.to_string())
        .execute(db.pool())
        .await
        .unwrap();

        let hints = serde_json::json!([
            {
                "id": "pending-word:provider_speaker_index",
                "word_id": "pending-word",
                "type": "provider_speaker_index",
                "value": r#"{"channel":1,"speaker_index":7}"#
            },
            {
                "word_id": "pending-word",
                "type": "user_speaker_assignment",
                "value": r#"{"human_id":"speaker-human","scope":"speaker","channel":1,"speaker_index":7}"#
            },
        ]);
        sqlx::query(
            "INSERT INTO transcripts
                (id, session_id, started_at_ms, ended_at_ms, words_json, speaker_hints_json)
             VALUES ('transcript-2', 'session-1', 500, 4000, '[]', ?)",
        )
        .bind(hints.to_string())
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO transcript_live_state (transcript_id, next_sequence)
             VALUES ('transcript-2', 1)",
        )
        .execute(db.pool())
        .await
        .unwrap();
        let delta = serde_json::json!({
            "new_words": [{
                "id": "pending-word",
                "text": "pending word",
                "start_ms": 500,
                "end_ms": 900,
                "channel": 1,
                "speaker_index": 7,
                "state": "final"
            }],
            "replaced_ids": [],
            "partials": []
        });
        sqlx::query(
            "INSERT INTO transcript_live_deltas (id, transcript_id, sequence, delta_json)
             VALUES ('delta-1', 'transcript-2', 0, ?)",
        )
        .bind(delta.to_string())
        .execute(db.pool())
        .await
        .unwrap();
        db
    }

    #[tokio::test]
    async fn render_session_transcript_materializes_pending_words_with_assigned_human_and_transcript_dates()
     {
        let db = test_db().await;
        let rendered = render_session_transcript(
            db.pool(),
            RenderSessionTranscriptRequest {
                session_id: "session-1".to_string(),
                self_human_id: Some("owner-human".to_string()),
            },
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(rendered.started_at, Some(500));
        assert_eq!(rendered.ended_at, Some(4000));
        assert!(rendered.segments.iter().any(|segment| {
            segment.speaker_label == "Assigned Speaker" && segment.text.contains("pending word")
        }));
    }
}
