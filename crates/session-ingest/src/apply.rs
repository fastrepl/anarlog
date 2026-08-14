use std::{collections::HashSet, fmt::Write};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::{SessionIngestEnvelope, validate};

const SOURCE_TYPE: &str = "meeting_bot";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyOutcome {
    Applied,
    AlreadyApplied,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid session ingest envelope: {0}")]
    InvalidEnvelope(String),
    #[error("session ingest schema {actual} is unsupported; supported version is {supported}")]
    UnsupportedSchema { actual: u32, supported: u32 },
    #[error("session ingest envelope is {actual} bytes; maximum is {maximum}")]
    EnvelopeTooLarge { actual: usize, maximum: usize },
    #[error("workspace mismatch: expected '{expected}', received '{actual}'")]
    WorkspaceMismatch { expected: String, actual: String },
    #[error("{entity} '{id}' belongs to a different session, workspace, or owner")]
    OwnershipConflict { entity: &'static str, id: String },
    #[error("session '{session_id}' is not owned by capture source '{source_id}'")]
    SourceConflict {
        session_id: String,
        source_id: String,
    },
    #[error("{entity} '{id}' is not owned by capture source '{source_id}'")]
    ChildSourceConflict {
        entity: &'static str,
        id: String,
        source_id: String,
    },
    #[error("session '{session_id}' already finalized at revision {revision}")]
    Finalized { session_id: String, revision: u64 },
    #[error("session '{session_id}' was deleted")]
    DeletedSession { session_id: String },
    #[error("revision {actual} is older than applied revision {applied}")]
    StaleRevision { actual: u64, applied: u64 },
    #[error("revision {revision} was already applied with different content")]
    RevisionConflict { revision: u64 },
    #[error("existing metadata for {entity} '{id}' is invalid")]
    InvalidExistingMetadata { entity: &'static str, id: String },
    #[error("could not serialize session ingest data: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("could not apply session ingest envelope: {0}")]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IngestMarker {
    source_id: String,
    revision: u64,
    content_hash: String,
    finalized: bool,
}

struct ExistingSession {
    workspace_id: String,
    owner_user_id: String,
    metadata_json: String,
    deleted_at: Option<String>,
}

pub async fn apply_session_envelope(
    pool: &SqlitePool,
    expected_workspace_id: &str,
    envelope: &SessionIngestEnvelope,
) -> Result<ApplyOutcome, Error> {
    let serialized = serde_json::to_vec(envelope)?;
    validate::validate(expected_workspace_id, envelope, &serialized)?;
    let marker = IngestMarker {
        source_id: envelope.source_id.clone(),
        revision: envelope.revision,
        content_hash: content_hash(envelope)?,
        finalized: envelope.finalized,
    };

    let mut tx = pool.begin().await?;
    let existing = load_session(&mut tx, &envelope.session.id).await?;
    let attached_existing = match existing.as_ref() {
        Some(existing) => {
            ensure_session_scope(existing, envelope)?;
            if existing.deleted_at.is_some() {
                return Err(Error::DeletedSession {
                    session_id: envelope.session.id.clone(),
                });
            }
            match read_marker("session", &envelope.session.id, &existing.metadata_json)? {
                Some(applied) => {
                    if applied.source_id != envelope.source_id {
                        return Err(Error::SourceConflict {
                            session_id: envelope.session.id.clone(),
                            source_id: envelope.source_id.clone(),
                        });
                    }
                    if envelope.revision < applied.revision {
                        return Err(Error::StaleRevision {
                            actual: envelope.revision,
                            applied: applied.revision,
                        });
                    }
                    if envelope.revision == applied.revision {
                        if marker.content_hash == applied.content_hash {
                            return Ok(ApplyOutcome::AlreadyApplied);
                        }
                        return Err(Error::RevisionConflict {
                            revision: envelope.revision,
                        });
                    }
                    if applied.finalized {
                        return Err(Error::Finalized {
                            session_id: envelope.session.id.clone(),
                            revision: applied.revision,
                        });
                    }
                    envelope.attach_to_existing
                }
                None if envelope.attach_to_existing => true,
                None => {
                    return Err(Error::SourceConflict {
                        session_id: envelope.session.id.clone(),
                        source_id: envelope.source_id.clone(),
                    });
                }
            }
        }
        None => false,
    };

    ensure_child_scopes(&mut tx, envelope).await?;
    write_session(&mut tx, envelope, &marker, existing, attached_existing).await?;
    write_documents(&mut tx, envelope, &marker).await?;
    write_attachments(&mut tx, envelope, &marker).await?;
    write_transcripts(&mut tx, envelope, &marker).await?;
    write_participants(&mut tx, envelope, &marker).await?;
    reconcile_children(&mut tx, envelope, &marker).await?;
    tx.commit().await?;

    Ok(ApplyOutcome::Applied)
}

async fn load_session(
    tx: &mut Transaction<'_, Sqlite>,
    session_id: &str,
) -> Result<Option<ExistingSession>, sqlx::Error> {
    sqlx::query_as::<_, (String, String, String, Option<String>)>(
        "SELECT workspace_id, owner_user_id, metadata_json, deleted_at FROM sessions WHERE id = ?",
    )
    .bind(session_id)
    .fetch_optional(&mut **tx)
    .await
    .map(|row| {
        row.map(
            |(workspace_id, owner_user_id, metadata_json, deleted_at)| ExistingSession {
                workspace_id,
                owner_user_id,
                metadata_json,
                deleted_at,
            },
        )
    })
}

fn ensure_session_scope(
    existing: &ExistingSession,
    envelope: &SessionIngestEnvelope,
) -> Result<(), Error> {
    if existing.workspace_id != envelope.workspace_id
        || existing.owner_user_id != envelope.owner_user_id
    {
        return Err(Error::OwnershipConflict {
            entity: "session",
            id: envelope.session.id.clone(),
        });
    }
    Ok(())
}

async fn ensure_child_scopes(
    tx: &mut Transaction<'_, Sqlite>,
    envelope: &SessionIngestEnvelope,
) -> Result<(), Error> {
    for document in &envelope.documents {
        ensure_session_child_scope(tx, "session_documents", "document", &document.id, envelope)
            .await?;
    }
    for transcript in &envelope.transcripts {
        ensure_session_child_scope(tx, "transcripts", "transcript", &transcript.id, envelope)
            .await?;
    }
    for participant in &envelope.participants {
        ensure_session_child_scope(
            tx,
            "session_participants",
            "participant",
            &participant.id,
            envelope,
        )
        .await?;
        if !participant.human_id.is_empty() {
            let row = sqlx::query_as::<_, (String, String)>(
                "SELECT workspace_id, owner_user_id FROM humans WHERE id = ?",
            )
            .bind(&participant.human_id)
            .fetch_optional(&mut **tx)
            .await?;
            if row.is_some_and(|(workspace_id, owner_user_id)| {
                workspace_id != envelope.workspace_id || owner_user_id != envelope.owner_user_id
            }) {
                return Err(Error::OwnershipConflict {
                    entity: "human",
                    id: participant.human_id.clone(),
                });
            }
        }
    }
    for attachment in &envelope.attachments {
        ensure_session_child_scope(
            tx,
            "session_attachments",
            "attachment",
            &attachment.id,
            envelope,
        )
        .await?;
    }
    Ok(())
}

async fn ensure_session_child_scope(
    tx: &mut Transaction<'_, Sqlite>,
    table: &'static str,
    entity: &'static str,
    id: &str,
    envelope: &SessionIngestEnvelope,
) -> Result<(), Error> {
    let statement = match table {
        "session_documents" => {
            "SELECT workspace_id, session_id, generation_metadata_json
             FROM session_documents WHERE id = ?"
        }
        "transcripts" => {
            "SELECT workspace_id, session_id, metadata_json FROM transcripts WHERE id = ?"
        }
        "session_participants" => {
            "SELECT workspace_id, session_id, metadata_json
             FROM session_participants WHERE id = ?"
        }
        "session_attachments" => {
            "SELECT workspace_id, session_id, metadata_json
             FROM session_attachments WHERE id = ?"
        }
        _ => unreachable!(),
    };
    let row = sqlx::query_as::<_, (String, String, String)>(statement)
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?;
    if let Some((workspace_id, session_id, metadata_json)) = row {
        if workspace_id != envelope.workspace_id || session_id != envelope.session.id {
            return Err(Error::OwnershipConflict {
                entity,
                id: id.to_string(),
            });
        }
        let source_matches = read_marker(entity, id, &metadata_json)?
            .is_some_and(|marker| marker.source_id == envelope.source_id);
        if !source_matches {
            return Err(Error::ChildSourceConflict {
                entity,
                id: id.to_string(),
                source_id: envelope.source_id.clone(),
            });
        }
    }
    Ok(())
}

async fn write_session(
    tx: &mut Transaction<'_, Sqlite>,
    envelope: &SessionIngestEnvelope,
    marker: &IngestMarker,
    existing: Option<ExistingSession>,
    attached_existing: bool,
) -> Result<(), Error> {
    let metadata_json = merge_metadata(
        "session",
        &envelope.session.id,
        existing
            .as_ref()
            .map(|session| session.metadata_json.as_str()),
        &envelope.session.metadata,
        marker,
    )?;
    let source_apps_json = serde_json::to_string(&[json!({ "app": SOURCE_TYPE })])?;
    let event_json = if envelope.session.event.is_null() {
        String::new()
    } else {
        serde_json::to_string(&envelope.session.event)?
    };

    if existing.is_none() {
        sqlx::query(
            "INSERT INTO sessions (
                id, workspace_id, owner_user_id, title, kind, status, created_at, updated_at,
                started_at, ended_at, timezone, language, event_id, external_event_id,
                external_provider, series_id, source_apps_json, event_json, metadata_json
             ) VALUES (?, ?, ?, ?, 'meeting', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&envelope.session.id)
        .bind(&envelope.workspace_id)
        .bind(&envelope.owner_user_id)
        .bind(&envelope.session.title)
        .bind(&envelope.session.status)
        .bind(&envelope.session.created_at)
        .bind(&envelope.session.updated_at)
        .bind(&envelope.session.started_at)
        .bind(&envelope.session.ended_at)
        .bind(&envelope.session.timezone)
        .bind(&envelope.session.language)
        .bind(&envelope.session.event_id)
        .bind(&envelope.session.external_event_id)
        .bind(&envelope.session.external_provider)
        .bind(&envelope.session.series_id)
        .bind(source_apps_json)
        .bind(event_json)
        .bind(metadata_json)
        .execute(&mut **tx)
        .await?;
    } else if attached_existing {
        sqlx::query(
            "UPDATE sessions SET
                status = ?,
                updated_at = ?,
                started_at = COALESCE(NULLIF(?, ''), started_at),
                ended_at = COALESCE(NULLIF(?, ''), ended_at),
                timezone = COALESCE(NULLIF(timezone, ''), ?),
                language = COALESCE(NULLIF(language, ''), ?),
                event_id = COALESCE(NULLIF(event_id, ''), ?),
                external_event_id = COALESCE(NULLIF(external_event_id, ''), ?),
                external_provider = COALESCE(NULLIF(external_provider, ''), ?),
                series_id = COALESCE(NULLIF(series_id, ''), ?),
                metadata_json = ?
             WHERE id = ?",
        )
        .bind(&envelope.session.status)
        .bind(&envelope.session.updated_at)
        .bind(&envelope.session.started_at)
        .bind(&envelope.session.ended_at)
        .bind(&envelope.session.timezone)
        .bind(&envelope.session.language)
        .bind(&envelope.session.event_id)
        .bind(&envelope.session.external_event_id)
        .bind(&envelope.session.external_provider)
        .bind(&envelope.session.series_id)
        .bind(metadata_json)
        .bind(&envelope.session.id)
        .execute(&mut **tx)
        .await?;
    } else {
        sqlx::query(
            "UPDATE sessions SET
                title = ?, status = ?, updated_at = ?, started_at = ?, ended_at = ?,
                timezone = ?, language = ?, event_id = ?, external_event_id = ?,
                external_provider = ?, series_id = ?, source_apps_json = ?, event_json = ?,
                metadata_json = ?
             WHERE id = ?",
        )
        .bind(&envelope.session.title)
        .bind(&envelope.session.status)
        .bind(&envelope.session.updated_at)
        .bind(&envelope.session.started_at)
        .bind(&envelope.session.ended_at)
        .bind(&envelope.session.timezone)
        .bind(&envelope.session.language)
        .bind(&envelope.session.event_id)
        .bind(&envelope.session.external_event_id)
        .bind(&envelope.session.external_provider)
        .bind(&envelope.session.series_id)
        .bind(source_apps_json)
        .bind(event_json)
        .bind(metadata_json)
        .bind(&envelope.session.id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn write_documents(
    tx: &mut Transaction<'_, Sqlite>,
    envelope: &SessionIngestEnvelope,
    marker: &IngestMarker,
) -> Result<(), Error> {
    for document in &envelope.documents {
        let existing = load_metadata(tx, "session_documents", &document.id).await?;
        let generation_metadata = merge_metadata(
            "document",
            &document.id,
            existing.as_deref(),
            &document.generation_metadata,
            marker,
        )?;
        sqlx::query(
            "INSERT INTO session_documents (
                id, workspace_id, session_id, kind, template_id, title, body_format, body,
                source_hash, generation_metadata_json, sort_order, created_by, updated_by,
                created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                kind = excluded.kind, template_id = excluded.template_id, title = excluded.title,
                body_format = excluded.body_format, body = excluded.body,
                source_hash = excluded.source_hash,
                generation_metadata_json = excluded.generation_metadata_json,
                sort_order = excluded.sort_order, updated_by = excluded.updated_by,
                updated_at = excluded.updated_at, deleted_at = NULL",
        )
        .bind(&document.id)
        .bind(&envelope.workspace_id)
        .bind(&envelope.session.id)
        .bind(document.kind.as_str())
        .bind(&document.template_id)
        .bind(&document.title)
        .bind(document.format.as_str())
        .bind(&document.body)
        .bind(&document.source_hash)
        .bind(generation_metadata)
        .bind(document.sort_order)
        .bind(SOURCE_TYPE)
        .bind(SOURCE_TYPE)
        .bind(&document.created_at)
        .bind(&document.updated_at)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn write_attachments(
    tx: &mut Transaction<'_, Sqlite>,
    envelope: &SessionIngestEnvelope,
    marker: &IngestMarker,
) -> Result<(), Error> {
    for attachment in &envelope.attachments {
        let existing = load_metadata(tx, "session_attachments", &attachment.id).await?;
        let metadata = merge_metadata(
            "attachment",
            &attachment.id,
            existing.as_deref(),
            &attachment.metadata,
            marker,
        )?;
        sqlx::query(
            "INSERT INTO session_attachments (
                id, workspace_id, session_id, filename, content_type, size_bytes, sha256,
                storage_kind, cloud_object_key, source_type, source_id, metadata_json,
                created_at, updated_at, cloud_sync_enabled
             ) VALUES (?, ?, ?, ?, ?, ?, ?, 'remote_object', ?, ?, ?, ?, ?, ?, 0)
             ON CONFLICT(id) DO UPDATE SET
                filename = excluded.filename, content_type = excluded.content_type,
                size_bytes = excluded.size_bytes, sha256 = excluded.sha256,
                storage_kind = excluded.storage_kind, cloud_object_key = excluded.cloud_object_key,
                source_type = excluded.source_type, source_id = excluded.source_id,
                metadata_json = excluded.metadata_json, updated_at = excluded.updated_at,
                cloud_sync_enabled = 0, deleted_at = NULL",
        )
        .bind(&attachment.id)
        .bind(&envelope.workspace_id)
        .bind(&envelope.session.id)
        .bind(&attachment.filename)
        .bind(&attachment.content_type)
        .bind(i64::try_from(attachment.size_bytes).expect("attachment size was validated"))
        .bind(&attachment.sha256)
        .bind(&attachment.object_key)
        .bind(SOURCE_TYPE)
        .bind(&envelope.source_id)
        .bind(metadata)
        .bind(&attachment.created_at)
        .bind(&attachment.updated_at)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn write_transcripts(
    tx: &mut Transaction<'_, Sqlite>,
    envelope: &SessionIngestEnvelope,
    marker: &IngestMarker,
) -> Result<(), Error> {
    for transcript in &envelope.transcripts {
        let existing = load_metadata(tx, "transcripts", &transcript.id).await?;
        let metadata = merge_metadata(
            "transcript",
            &transcript.id,
            existing.as_deref(),
            &transcript.metadata,
            marker,
        )?;
        sqlx::query(
            "INSERT INTO transcripts (
                id, workspace_id, owner_user_id, session_id, source, provider, model, language,
                started_at_ms, ended_at_ms, audio_attachment_id, memo, words_json,
                speaker_hints_json, metadata_json, created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                source = excluded.source, provider = excluded.provider, model = excluded.model,
                language = excluded.language, started_at_ms = excluded.started_at_ms,
                ended_at_ms = excluded.ended_at_ms,
                audio_attachment_id = excluded.audio_attachment_id, memo = excluded.memo,
                words_json = excluded.words_json,
                speaker_hints_json = excluded.speaker_hints_json,
                metadata_json = excluded.metadata_json, updated_at = excluded.updated_at,
                deleted_at = NULL",
        )
        .bind(&transcript.id)
        .bind(&envelope.workspace_id)
        .bind(&envelope.owner_user_id)
        .bind(&envelope.session.id)
        .bind(SOURCE_TYPE)
        .bind(&transcript.provider)
        .bind(&transcript.model)
        .bind(&transcript.language)
        .bind(transcript.started_at_ms)
        .bind(transcript.ended_at_ms)
        .bind(&transcript.audio_attachment_id)
        .bind(&transcript.memo)
        .bind(serde_json::to_string(&transcript.words)?)
        .bind(serde_json::to_string(&transcript.speaker_hints)?)
        .bind(metadata)
        .bind(&transcript.created_at)
        .bind(&transcript.updated_at)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn write_participants(
    tx: &mut Transaction<'_, Sqlite>,
    envelope: &SessionIngestEnvelope,
    marker: &IngestMarker,
) -> Result<(), Error> {
    for participant in &envelope.participants {
        if !participant.human_id.is_empty() {
            sqlx::query(
                "INSERT OR IGNORE INTO humans (
                    id, workspace_id, owner_user_id, name, email, created_at, updated_at
                 ) VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&participant.human_id)
            .bind(&envelope.workspace_id)
            .bind(&envelope.owner_user_id)
            .bind(&participant.display_name)
            .bind(&participant.email)
            .bind(&participant.created_at)
            .bind(&participant.updated_at)
            .execute(&mut **tx)
            .await?;
        }
        let existing = load_metadata(tx, "session_participants", &participant.id).await?;
        let metadata = merge_metadata(
            "participant",
            &participant.id,
            existing.as_deref(),
            &participant.metadata,
            marker,
        )?;
        sqlx::query(
            "INSERT INTO session_participants (
                id, workspace_id, owner_user_id, session_id, human_id, display_name, email,
                role, source, metadata_json, created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                human_id = excluded.human_id, display_name = excluded.display_name,
                email = excluded.email, role = excluded.role, source = excluded.source,
                metadata_json = excluded.metadata_json, updated_at = excluded.updated_at,
                deleted_at = NULL",
        )
        .bind(&participant.id)
        .bind(&envelope.workspace_id)
        .bind(&envelope.owner_user_id)
        .bind(&envelope.session.id)
        .bind(&participant.human_id)
        .bind(&participant.display_name)
        .bind(&participant.email)
        .bind(&participant.role)
        .bind(SOURCE_TYPE)
        .bind(metadata)
        .bind(&participant.created_at)
        .bind(&participant.updated_at)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn reconcile_children(
    tx: &mut Transaction<'_, Sqlite>,
    envelope: &SessionIngestEnvelope,
    marker: &IngestMarker,
) -> Result<(), Error> {
    reconcile_table(
        tx,
        "session_documents",
        "document",
        envelope,
        marker,
        envelope.documents.iter().map(|value| value.id.as_str()),
    )
    .await?;
    reconcile_table(
        tx,
        "transcripts",
        "transcript",
        envelope,
        marker,
        envelope.transcripts.iter().map(|value| value.id.as_str()),
    )
    .await?;
    reconcile_table(
        tx,
        "session_participants",
        "participant",
        envelope,
        marker,
        envelope.participants.iter().map(|value| value.id.as_str()),
    )
    .await?;
    reconcile_table(
        tx,
        "session_attachments",
        "attachment",
        envelope,
        marker,
        envelope.attachments.iter().map(|value| value.id.as_str()),
    )
    .await?;
    Ok(())
}

async fn reconcile_table<'a>(
    tx: &mut Transaction<'_, Sqlite>,
    table: &'static str,
    entity: &'static str,
    envelope: &SessionIngestEnvelope,
    marker: &IngestMarker,
    retained_ids: impl Iterator<Item = &'a str>,
) -> Result<(), Error> {
    let select = match table {
        "session_documents" => {
            "SELECT id, generation_metadata_json FROM session_documents WHERE session_id = ? AND deleted_at IS NULL"
        }
        "transcripts" => {
            "SELECT id, metadata_json FROM transcripts WHERE session_id = ? AND deleted_at IS NULL"
        }
        "session_participants" => {
            "SELECT id, metadata_json FROM session_participants WHERE session_id = ? AND deleted_at IS NULL"
        }
        "session_attachments" => {
            "SELECT id, metadata_json FROM session_attachments WHERE session_id = ? AND deleted_at IS NULL"
        }
        _ => unreachable!(),
    };
    let rows = sqlx::query_as::<_, (String, String)>(select)
        .bind(&envelope.session.id)
        .fetch_all(&mut **tx)
        .await?;
    let retained_ids = retained_ids.collect::<HashSet<_>>();
    for (id, metadata_json) in rows {
        if retained_ids.contains(id.as_str()) {
            continue;
        }
        let Some(applied) = read_marker(entity, &id, &metadata_json)? else {
            continue;
        };
        if applied.source_id != marker.source_id {
            continue;
        }
        let update = match table {
            "session_documents" => {
                "UPDATE session_documents SET deleted_at = ?, updated_at = ? WHERE id = ?"
            }
            "transcripts" => "UPDATE transcripts SET deleted_at = ?, updated_at = ? WHERE id = ?",
            "session_participants" => {
                "UPDATE session_participants SET deleted_at = ?, updated_at = ? WHERE id = ?"
            }
            "session_attachments" => {
                "UPDATE session_attachments SET deleted_at = ?, updated_at = ? WHERE id = ?"
            }
            _ => unreachable!(),
        };
        sqlx::query(update)
            .bind(&envelope.session.updated_at)
            .bind(&envelope.session.updated_at)
            .bind(&id)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

async fn load_metadata(
    tx: &mut Transaction<'_, Sqlite>,
    table: &'static str,
    id: &str,
) -> Result<Option<String>, sqlx::Error> {
    let statement = match table {
        "session_documents" => {
            "SELECT generation_metadata_json FROM session_documents WHERE id = ?"
        }
        "transcripts" => "SELECT metadata_json FROM transcripts WHERE id = ?",
        "session_participants" => "SELECT metadata_json FROM session_participants WHERE id = ?",
        "session_attachments" => "SELECT metadata_json FROM session_attachments WHERE id = ?",
        _ => unreachable!(),
    };
    sqlx::query_scalar(statement)
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
}

fn merge_metadata(
    entity: &'static str,
    id: &str,
    existing: Option<&str>,
    incoming: &Map<String, Value>,
    marker: &IngestMarker,
) -> Result<String, Error> {
    let mut metadata = match existing {
        Some(existing) => serde_json::from_str::<Map<String, Value>>(existing).map_err(|_| {
            Error::InvalidExistingMetadata {
                entity,
                id: id.to_string(),
            }
        })?,
        None => Map::new(),
    };
    metadata.extend(incoming.clone());
    metadata.insert(
        validate::reserved_metadata_key().to_string(),
        serde_json::to_value(marker)?,
    );
    serde_json::to_string(&metadata).map_err(Error::from)
}

fn read_marker(
    entity: &'static str,
    id: &str,
    metadata_json: &str,
) -> Result<Option<IngestMarker>, Error> {
    let metadata = serde_json::from_str::<Map<String, Value>>(metadata_json).map_err(|_| {
        Error::InvalidExistingMetadata {
            entity,
            id: id.to_string(),
        }
    })?;
    metadata
        .get(validate::reserved_metadata_key())
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(Error::from)
}

fn content_hash(envelope: &SessionIngestEnvelope) -> Result<String, serde_json::Error> {
    let mut canonical = serde_json::to_value(envelope)?;
    sort_json_keys(&mut canonical);
    Ok(sha256_hex(&serde_json::to_vec(&canonical)?))
}

fn sort_json_keys(value: &mut Value) {
    match value {
        Value::Array(values) => values.iter_mut().for_each(sort_json_keys),
        Value::Object(values) => {
            values.values_mut().for_each(sort_json_keys);
            values.sort_keys();
        }
        _ => {}
    }
}

fn sha256_hex(value: &[u8]) -> String {
    let mut output = String::with_capacity(64);
    for byte in Sha256::digest(value) {
        write!(output, "{byte:02x}").expect("writing to a string cannot fail");
    }
    output
}
