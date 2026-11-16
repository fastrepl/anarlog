use std::sync::Arc;

use anlg_db_execute::TransactionStatement;
use anlg_listener_core::LiveTranscriptDelta;
use anlg_transcript::{RenderTranscriptHuman, RenderTranscriptRequest, RenderedTranscriptSegment};
use desktop_runtime::{
    CancellationToken, Reply, Result, RuntimeHandle, ServiceError, Services, SessionId,
};
use serde_json::{Value, json};

use super::model::{
    Interval, MAX_SNAPSHOT_BYTES, Transcript, Word, failure, recovered_additions, validate_delta,
};

#[derive(Clone)]
pub struct TranscriptStore(pub RuntimeHandle);

pub struct SessionTranscript {
    pub segments: Vec<Arc<RenderedTranscriptSegment>>,
    pub transcripts: Vec<Transcript>,
}

#[derive(Clone)]
pub struct SpeakerAssignment {
    pub transcript_id: Arc<str>,
    pub anchor: String,
    pub human_id: String,
    pub scope: SpeakerScope,
}

#[derive(Clone)]
pub enum SpeakerScope {
    All { channel: i32, speaker_index: i32 },
    Segment { word_ids: Vec<String> },
}

impl TranscriptStore {
    pub fn load(
        &self,
        session: SessionId,
        cancel: CancellationToken,
    ) -> Result<Reply<SessionTranscript>> {
        self.0.read(cancel, move |services| async move {
            let rows = services.executor.execute(
                "SELECT id FROM transcripts WHERE session_id = ? AND deleted_at IS NULL AND EXISTS (SELECT 1 FROM sessions WHERE id = transcripts.session_id AND deleted_at IS NULL AND locked = 0) ORDER BY started_at_ms, created_at, id".into(),
                vec![json!(session)],
            ).await.map_err(failure)?;
            let mut transcripts = Vec::new();
            for row in rows {
                let id = string(&row, "id")?;
                transcripts.push(load(&services, id.into()).await?);
            }
            let humans = services.executor.execute(
                "SELECT DISTINCT h.id AS human_id, h.name FROM humans h WHERE h.deleted_at IS NULL AND (EXISTS (SELECT 1 FROM session_participants p WHERE p.session_id = ? AND p.human_id = h.id AND p.deleted_at IS NULL) OR EXISTS (SELECT 1 FROM transcripts t, json_each(t.speaker_hints_json) j WHERE t.session_id = ? AND t.deleted_at IS NULL AND json_valid(j.value) AND json_valid(json_extract(j.value, '$.value')) AND json_extract(json_extract(j.value, '$.value'), '$.human_id') = h.id))".into(),
                vec![json!(session), json!(session)],
            ).await.map_err(failure)?;
            let humans = humans.into_iter().map(|row| serde_json::from_value::<RenderTranscriptHuman>(row).map_err(failure)).collect::<Result<Vec<_>>>()?;
            let inputs = transcripts.iter().map(Transcript::render_input).collect::<Result<Vec<_>>>()?;
            let segments = anlg_transcript::render_transcript_segments(RenderTranscriptRequest {
                speaker_context: None,
                preview: None,
                transcripts: inputs,
                participant_human_ids: Vec::new(),
                self_human_id: None,
                humans,
            }).into_iter().map(Arc::new).collect();
            Ok(SessionTranscript { segments, transcripts })
        })
    }

    pub fn snapshot(&self, id: Arc<str>) -> Result<Reply<Transcript>> {
        self.0
            .submit(move |services| async move { load(&services, id).await })
    }

    pub fn journal(
        &self,
        id: Arc<str>,
        operation: Arc<str>,
        mut delta: LiveTranscriptDelta,
    ) -> Result<Reply<()>> {
        validate_delta(&delta)?;
        delta.partials.clear();
        self.0.submit(
            move |services| async move { journal(&services, &id, &operation, &delta).await },
        )
    }

    pub fn fold(&self, id: Arc<str>) -> Result<Reply<()>> {
        self.mutate(id, |_| Ok(()))
    }

    pub fn edit_word(
        &self,
        id: Arc<str>,
        expected: Word,
        text: Option<String>,
    ) -> Result<Reply<()>> {
        self.mutate(id, move |snapshot| {
            let index = snapshot
                .words
                .iter()
                .position(|word| word.id == expected.id)
                .ok_or(ServiceError::Conflict)?;
            if snapshot.words[index] != expected {
                return Err(ServiceError::Conflict);
            }
            match &text {
                Some(text) => snapshot.words[index].text = text.clone(),
                None => {
                    snapshot.words.remove(index);
                }
            }
            Ok(())
        })
    }

    pub fn repair(
        &self,
        id: Arc<str>,
        before: Vec<Word>,
        recovered: Vec<Word>,
        hints: Vec<Value>,
        gaps: Vec<Interval>,
    ) -> Result<Reply<()>> {
        self.mutate(id, move |snapshot| {
            let additions = recovered_additions(recovered.clone(), &before, &snapshot.words, &gaps);
            let ids: std::collections::HashSet<_> =
                additions.iter().map(|word| word.id.as_str()).collect();
            snapshot.hints.extend(
                hints
                    .iter()
                    .filter(|hint| hint["word_id"].as_str().is_some_and(|id| ids.contains(id)))
                    .cloned(),
            );
            snapshot.words.extend(additions);
            snapshot.words.sort_by_key(|word| word.start_ms);
            Ok(())
        })
    }

    pub fn assign(&self, assignment: SpeakerAssignment) -> Result<Reply<()>> {
        self.mutate(assignment.transcript_id.clone(), move |snapshot| {
            apply_assignment(snapshot, &assignment)
        })
    }

    pub fn assign_participant(
        &self,
        session: SessionId,
        assignment: SpeakerAssignment,
        previous_human: Option<String>,
        new_name: Option<String>,
    ) -> Result<Reply<()>> {
        self.0.submit(move |services| async move {
            let rows = services.executor.execute(
                "SELECT id FROM transcripts WHERE session_id = ? AND deleted_at IS NULL".into(),
                vec![json!(session)],
            ).await.map_err(failure)?;
            if !rows.iter().any(|row| row["id"].as_str() == Some(&assignment.transcript_id)) {
                return Err(ServiceError::Conflict);
            }
            let mut statements = Vec::new();
            if let Some(name) = new_name {
                if name.trim().is_empty() || name.len() > 512 { return Err(failure("Enter a name up to 512 bytes.")); }
                statements.push(statement(
                    "INSERT INTO humans (id, workspace_id, owner_user_id, name) SELECT ?, workspace_id, owner_user_id, ? FROM sessions WHERE id = ? AND deleted_at IS NULL AND locked = 0",
                    vec![json!(assignment.human_id), json!(name.trim()), json!(session)], Some(1)));
            }
            statements.push(statement(
                "INSERT INTO session_participants (id, workspace_id, owner_user_id, session_id, human_id, display_name) SELECT ?, s.workspace_id, s.owner_user_id, s.id, h.id, h.name FROM sessions s JOIN humans h ON h.id = ? AND h.deleted_at IS NULL WHERE s.id = ? AND s.deleted_at IS NULL AND s.locked = 0 AND NOT EXISTS (SELECT 1 FROM session_participants p WHERE p.session_id = s.id AND p.human_id = h.id AND p.deleted_at IS NULL)",
                vec![json!(uuid::Uuid::new_v4().to_string()), json!(assignment.human_id), json!(session)], None));
            for row in rows {
                let id: Arc<str> = string(&row, "id")?.into();
                let mut snapshot = load(&services, id.clone()).await?;
                if id == assignment.transcript_id {
                    apply_assignment(&mut snapshot, &assignment)?;
                } else if let Some(previous) = &previous_human {
                    let rendered = anlg_transcript::render_transcript_segments(RenderTranscriptRequest {
                        transcripts: vec![snapshot.render_input()?], speaker_context: None, preview: None,
                        participant_human_ids: Vec::new(), self_human_id: None, humans: Vec::new(),
                    });
                    let ids: Vec<String> = rendered.into_iter()
                        .filter(|segment| segment.key.speaker_human_id.as_ref() == Some(previous))
                        .flat_map(|segment| segment.words.into_iter().filter_map(|word| word.id)).collect();
                    let Some(anchor) = ids.first().cloned() else { continue };
                    apply_assignment(&mut snapshot, &SpeakerAssignment {
                        transcript_id: id, anchor, human_id: assignment.human_id.clone(),
                        scope: SpeakerScope::Segment { word_ids: ids },
                    })?;
                } else { continue; }
                statements.extend(save_statements(&snapshot)?);
            }
            services.executor.execute_transaction(statements).await.map_err(|error| match error {
                anlg_db_execute::Error::UnexpectedRowsAffected { .. } => ServiceError::Conflict,
                error => failure(error),
            })?;
            Ok(())
        })
    }

    fn mutate<F>(&self, id: Arc<str>, mutation: F) -> Result<Reply<()>>
    where
        F: Fn(&mut Transcript) -> Result<()> + Send + 'static,
    {
        self.0.submit(move |services| async move {
            for _ in 0..5 {
                let mut snapshot = load(&services, id.clone()).await?;
                mutation(&mut snapshot)?;
                match save(&services, snapshot).await {
                    Err(ServiceError::Conflict) => continue,
                    result => return result,
                }
            }
            Err(ServiceError::Conflict)
        })
    }
}

fn apply_assignment(snapshot: &mut Transcript, assignment: &SpeakerAssignment) -> Result<()> {
    if !snapshot
        .words
        .iter()
        .any(|word| word.id == assignment.anchor)
    {
        return Err(ServiceError::Conflict);
    }
    let value = match &assignment.scope {
        SpeakerScope::All {
            channel,
            speaker_index,
        } => json!({
            "human_id": assignment.human_id, "scope": "speaker", "channel": channel, "speaker_index": speaker_index,
        }),
        SpeakerScope::Segment { word_ids } => {
            if !word_ids.contains(&assignment.anchor)
                || word_ids
                    .iter()
                    .any(|id| !snapshot.words.iter().any(|word| &word.id == id))
            {
                return Err(ServiceError::Conflict);
            }
            json!({"human_id": assignment.human_id, "scope": "segment", "word_ids": word_ids, "extend_to_adjacent": false})
        }
    };
    let suffix = if matches!(assignment.scope, SpeakerScope::Segment { .. }) {
        ":segment"
    } else {
        ""
    };
    let id = format!("{}:user_speaker_assignment{suffix}", assignment.anchor);
    snapshot.hints.retain(|hint| hint["id"] != id);
    snapshot.hints.push(json!({"id": id, "word_id": assignment.anchor, "type": "user_speaker_assignment", "value": value.to_string()}));
    Ok(())
}

pub(super) async fn journal(
    services: &Services,
    id: &str,
    operation: &str,
    delta: &LiveTranscriptDelta,
) -> Result<()> {
    let encoded = serde_json::to_string(delta).map_err(failure)?;
    let previous = services
        .executor
        .execute(
            "SELECT delta_json FROM transcript_live_deltas WHERE id = ? AND transcript_id = ?"
                .into(),
            vec![json!(operation), json!(id)],
        )
        .await
        .map_err(failure)?;
    if let Some(row) = previous.first() {
        return if string(row, "delta_json")? == encoded {
            Ok(())
        } else {
            Err(ServiceError::Conflict)
        };
    }
    services.executor.execute_transaction(vec![
        statement("INSERT OR IGNORE INTO transcript_live_state (transcript_id) SELECT t.id FROM transcripts t JOIN sessions s ON s.id = t.session_id WHERE t.id = ? AND t.deleted_at IS NULL AND s.deleted_at IS NULL AND s.locked = 0", vec![json!(id)], None),
        statement("INSERT INTO transcript_live_deltas (id, transcript_id, sequence, delta_json) SELECT ?, l.transcript_id, l.next_sequence, ? FROM transcript_live_state l JOIN transcripts t ON t.id = l.transcript_id JOIN sessions s ON s.id = t.session_id WHERE l.transcript_id = ? AND t.deleted_at IS NULL AND s.deleted_at IS NULL AND s.locked = 0", vec![json!(operation), json!(encoded), json!(id)], Some(1)),
        statement("UPDATE transcript_live_state SET next_sequence = next_sequence + 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE transcript_id = ?", vec![json!(id)], Some(1)),
    ]).await.map_err(failure)?;
    Ok(())
}

pub(super) async fn load(services: &Services, id: Arc<str>) -> Result<Transcript> {
    let rows = services.executor.execute(
        "SELECT t.started_at_ms, t.content_revision, t.words_json, t.speaker_hints_json, COALESCE(l.next_sequence, 0) AS next_sequence FROM transcripts t LEFT JOIN transcript_live_state l ON l.transcript_id = t.id JOIN sessions s ON s.id = t.session_id WHERE t.id = ? AND t.deleted_at IS NULL AND s.deleted_at IS NULL AND s.locked = 0".into(),
        vec![json!(id)],
    ).await.map_err(failure)?;
    let row = rows.first().ok_or(ServiceError::Conflict)?;
    let words = string(row, "words_json")?;
    let hints = string(row, "speaker_hints_json")?;
    if words.len() + hints.len() > MAX_SNAPSHOT_BYTES {
        return Err(ServiceError::Unsupported(
            "Transcript exceeds the bounded snapshot limit; original preserved.".into(),
        ));
    }
    let mut transcript = Transcript {
        id: id.clone(),
        started_at: integer(row, "started_at_ms")?,
        revision: integer(row, "content_revision")?,
        sequence: integer(row, "next_sequence")?,
        words: serde_json::from_str(words).map_err(failure)?,
        hints: serde_json::from_str(hints).map_err(failure)?,
    };
    for word in &transcript.words {
        word.validate()?;
    }
    let mut sequence = -1;
    loop {
        let rows = services.executor.execute(
            "SELECT sequence, delta_json FROM transcript_live_deltas WHERE transcript_id = ? AND sequence > ? AND sequence < ? ORDER BY sequence LIMIT 64".into(),
            vec![json!(id), json!(sequence), json!(transcript.sequence)],
        ).await.map_err(failure)?;
        if rows.is_empty() {
            break;
        }
        for row in rows {
            let raw = string(&row, "delta_json")?;
            if raw.len() > MAX_SNAPSHOT_BYTES {
                return Err(failure("Oversized live journal entry."));
            }
            transcript.apply(&serde_json::from_str(raw).map_err(failure)?)?;
            sequence = integer(&row, "sequence")?;
        }
    }
    Ok(transcript)
}

pub(super) async fn save(services: &Services, snapshot: Transcript) -> Result<()> {
    services
        .executor
        .execute_transaction(save_statements(&snapshot)?)
        .await
        .map_err(|error| match error {
            anlg_db_execute::Error::UnexpectedRowsAffected { .. } => ServiceError::Conflict,
            error => failure(error),
        })?;
    Ok(())
}

fn save_statements(snapshot: &Transcript) -> Result<Vec<TransactionStatement>> {
    let words = serde_json::to_string(&snapshot.words).map_err(failure)?;
    let hints = serde_json::to_string(&snapshot.hints).map_err(failure)?;
    if words.len() + hints.len() > MAX_SNAPSHOT_BYTES {
        return Err(failure(
            "Transcript snapshot limit exceeded; journal preserved.",
        ));
    }
    Ok(vec![
        statement(
            "UPDATE transcripts SET words_json = ?, speaker_hints_json = ?, content_revision = content_revision + 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ? AND content_revision = ? AND deleted_at IS NULL AND COALESCE((SELECT next_sequence FROM transcript_live_state WHERE transcript_id = transcripts.id), 0) = ? AND EXISTS (SELECT 1 FROM sessions WHERE id = transcripts.session_id AND deleted_at IS NULL AND locked = 0)",
            vec![
                json!(words),
                json!(hints),
                json!(snapshot.id),
                json!(snapshot.revision),
                json!(snapshot.sequence),
            ],
            Some(1),
        ),
        statement(
            "DELETE FROM transcript_live_state WHERE transcript_id = ?",
            vec![json!(snapshot.id)],
            None,
        ),
    ])
}

pub fn statement(
    sql: &str,
    params: Vec<Value>,
    expected_rows_affected: Option<u64>,
) -> TransactionStatement {
    TransactionStatement {
        sql: sql.into(),
        params,
        expected_rows_affected,
    }
}

pub fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .ok_or_else(|| failure(format!("Invalid {key}; original record preserved.")))
}

pub fn integer(value: &Value, key: &str) -> Result<i64> {
    value[key]
        .as_i64()
        .ok_or_else(|| failure(format!("Invalid {key}; original record preserved.")))
}
