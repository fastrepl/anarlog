use std::collections::HashSet;

use crate::{
    MAX_SESSION_INGEST_BYTES, SESSION_INGEST_SCHEMA_VERSION, SessionIngestEnvelope, apply::Error,
};

const MAX_ID_BYTES: usize = 255;
const MAX_TIMESTAMP_BYTES: usize = 64;
const RESERVED_METADATA_KEY: &str = "anarlog_capture_ingest";

pub(crate) fn validate(
    expected_workspace_id: &str,
    envelope: &SessionIngestEnvelope,
    serialized: &[u8],
) -> Result<(), Error> {
    if serialized.len() > MAX_SESSION_INGEST_BYTES {
        return Err(Error::EnvelopeTooLarge {
            actual: serialized.len(),
            maximum: MAX_SESSION_INGEST_BYTES,
        });
    }
    if envelope.schema_version != SESSION_INGEST_SCHEMA_VERSION {
        return Err(Error::UnsupportedSchema {
            actual: envelope.schema_version,
            supported: SESSION_INGEST_SCHEMA_VERSION,
        });
    }
    if envelope.revision == 0 {
        return invalid("revision must be greater than zero");
    }
    validate_id("expected workspace id", expected_workspace_id)?;
    validate_id("workspace id", &envelope.workspace_id)?;
    if expected_workspace_id != envelope.workspace_id {
        return Err(Error::WorkspaceMismatch {
            expected: expected_workspace_id.to_string(),
            actual: envelope.workspace_id.clone(),
        });
    }
    validate_id("owner user id", &envelope.owner_user_id)?;
    validate_id("source id", &envelope.source_id)?;
    validate_id("session id", &envelope.session.id)?;
    validate_text("session title", &envelope.session.title, 1024)?;
    validate_text("session status", &envelope.session.status, 64)?;
    validate_timestamp("session created_at", &envelope.session.created_at, false)?;
    validate_timestamp("session updated_at", &envelope.session.updated_at, false)?;
    validate_timestamp("session started_at", &envelope.session.started_at, true)?;
    validate_timestamp("session ended_at", &envelope.session.ended_at, true)?;
    validate_metadata(&envelope.session.metadata)?;

    validate_unique_ids(
        "document",
        envelope
            .documents
            .iter()
            .map(|document| document.id.as_str()),
    )?;
    for document in &envelope.documents {
        validate_id("document id", &document.id)?;
        validate_timestamp("document created_at", &document.created_at, false)?;
        validate_timestamp("document updated_at", &document.updated_at, false)?;
        validate_metadata(&document.generation_metadata)?;
        if matches!(document.format, crate::DocumentFormat::ProsemirrorJson) {
            serde_json::from_str::<serde_json::Value>(&document.body).map_err(|error| {
                Error::InvalidEnvelope(format!(
                    "document '{}' contains invalid ProseMirror JSON: {error}",
                    document.id
                ))
            })?;
        }
    }

    validate_unique_ids(
        "transcript",
        envelope
            .transcripts
            .iter()
            .map(|transcript| transcript.id.as_str()),
    )?;
    for transcript in &envelope.transcripts {
        validate_id("transcript id", &transcript.id)?;
        validate_timestamp("transcript created_at", &transcript.created_at, false)?;
        validate_timestamp("transcript updated_at", &transcript.updated_at, false)?;
        validate_metadata(&transcript.metadata)?;
        if transcript.started_at_ms < 0
            || transcript
                .ended_at_ms
                .is_some_and(|ended| ended < transcript.started_at_ms)
        {
            return invalid(format!(
                "transcript '{}' has an invalid time range",
                transcript.id
            ));
        }
        validate_unique_ids("word", transcript.words.iter().map(|word| word.id.as_str()))?;
        for word in &transcript.words {
            validate_id("word id", &word.id)?;
            if word.start_ms < 0 || word.end_ms < word.start_ms {
                return invalid(format!("word '{}' has an invalid time range", word.id));
            }
        }
        validate_unique_ids(
            "speaker hint",
            transcript.speaker_hints.iter().map(|hint| hint.id.as_str()),
        )?;
        let word_ids = transcript
            .words
            .iter()
            .map(|word| word.id.as_str())
            .collect::<HashSet<_>>();
        for hint in &transcript.speaker_hints {
            validate_id("speaker hint id", &hint.id)?;
            if !word_ids.contains(hint.word_id.as_str()) {
                return invalid(format!(
                    "speaker hint '{}' references unknown word '{}'",
                    hint.id, hint.word_id
                ));
            }
        }
    }

    validate_unique_ids(
        "participant",
        envelope
            .participants
            .iter()
            .map(|participant| participant.id.as_str()),
    )?;
    for participant in &envelope.participants {
        validate_id("participant id", &participant.id)?;
        if !participant.human_id.is_empty() {
            validate_id("participant human id", &participant.human_id)?;
        }
        validate_timestamp("participant created_at", &participant.created_at, false)?;
        validate_timestamp("participant updated_at", &participant.updated_at, false)?;
        validate_metadata(&participant.metadata)?;
    }

    validate_unique_ids(
        "attachment",
        envelope
            .attachments
            .iter()
            .map(|attachment| attachment.id.as_str()),
    )?;
    let attachment_ids = envelope
        .attachments
        .iter()
        .map(|attachment| attachment.id.as_str())
        .collect::<HashSet<_>>();
    for attachment in &envelope.attachments {
        validate_id("attachment id", &attachment.id)?;
        validate_text("attachment filename", &attachment.filename, 1024)?;
        validate_text("attachment object key", &attachment.object_key, 2048)?;
        i64::try_from(attachment.size_bytes).map_err(|_| {
            Error::InvalidEnvelope(format!(
                "attachment '{}' exceeds the supported size",
                attachment.id
            ))
        })?;
        validate_timestamp("attachment created_at", &attachment.created_at, false)?;
        validate_timestamp("attachment updated_at", &attachment.updated_at, false)?;
        validate_metadata(&attachment.metadata)?;
    }
    for transcript in &envelope.transcripts {
        if !transcript.audio_attachment_id.is_empty()
            && !attachment_ids.contains(transcript.audio_attachment_id.as_str())
        {
            return invalid(format!(
                "transcript '{}' references unknown attachment '{}'",
                transcript.id, transcript.audio_attachment_id
            ));
        }
    }

    Ok(())
}

fn validate_unique_ids<'a>(entity: &str, ids: impl Iterator<Item = &'a str>) -> Result<(), Error> {
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id) {
            return invalid(format!("duplicate {entity} id '{id}'"));
        }
    }
    Ok(())
}

fn validate_id(field: &str, value: &str) -> Result<(), Error> {
    validate_text(field, value, MAX_ID_BYTES)
}

fn validate_timestamp(field: &str, value: &str, allow_empty: bool) -> Result<(), Error> {
    if allow_empty && value.is_empty() {
        return Ok(());
    }
    validate_text(field, value, MAX_TIMESTAMP_BYTES)
}

fn validate_text(field: &str, value: &str, maximum: usize) -> Result<(), Error> {
    if value.trim().is_empty() {
        return invalid(format!("{field} must not be empty"));
    }
    if value.len() > maximum {
        return invalid(format!("{field} exceeds {maximum} bytes"));
    }
    Ok(())
}

fn validate_metadata(metadata: &serde_json::Map<String, serde_json::Value>) -> Result<(), Error> {
    if metadata.contains_key(RESERVED_METADATA_KEY) {
        return invalid(format!(
            "metadata key '{RESERVED_METADATA_KEY}' is reserved"
        ));
    }
    Ok(())
}

fn invalid<T>(message: impl Into<String>) -> Result<T, Error> {
    Err(Error::InvalidEnvelope(message.into()))
}

pub(crate) const fn reserved_metadata_key() -> &'static str {
    RESERVED_METADATA_KEY
}
