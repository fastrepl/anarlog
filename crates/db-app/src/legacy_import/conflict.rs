use sqlx::{Sqlite, Transaction};

use super::{InsertOutcome, LegacyDocument, LegacyImportRow, LegacyTranscript};

pub(super) async fn reconcile_content_conflict(
    transaction: &mut Transaction<'_, Sqlite>,
    row: &LegacyImportRow,
) -> Result<Option<InsertOutcome>, sqlx::Error> {
    match row {
        LegacyImportRow::Session(row) => {
            if row.recovery_status.is_some() {
                return Ok(None);
            }
            let Some((metadata_json, deleted_at)) = sqlx::query_as::<_, (String, Option<String>)>(
                "SELECT metadata_json, deleted_at FROM sessions WHERE id = ?",
            )
            .bind(&row.id)
            .fetch_optional(&mut **transaction)
            .await?
            else {
                return Ok(None);
            };
            if deleted_at.is_some() || !is_recovered_session_placeholder(&metadata_json) {
                return Ok(None);
            }

            let result = sqlx::query(
                "UPDATE sessions
                 SET owner_user_id = ?, title = ?, created_at = ?, updated_at = ?,
                     started_at = ?, ended_at = ?, event_id = ?, external_event_id = ?,
                     external_provider = ?, series_id = ?, event_json = ?, metadata_json = ?,
                     folder_path = ?
                 WHERE id = ? AND metadata_json IS ? AND deleted_at IS NULL",
            )
            .bind(&row.owner_user_id)
            .bind(&row.title)
            .bind(&row.created_at)
            .bind(&row.created_at)
            .bind(&row.started_at)
            .bind(&row.ended_at)
            .bind(&row.event_id)
            .bind(&row.external_event_id)
            .bind(&row.external_provider)
            .bind(&row.series_id)
            .bind(&row.event_json)
            .bind(&row.metadata_json)
            .bind(&row.folder_path)
            .bind(&row.id)
            .bind(&metadata_json)
            .execute(&mut **transaction)
            .await?;

            Ok((result.rows_affected() == 1).then_some(InsertOutcome::FilledFromLegacy))
        }
        LegacyImportRow::Document(row) => {
            let Some((session_id, title, body_format, body, deleted_at)) =
                sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
                    "SELECT session_id, title, body_format, body, deleted_at
                     FROM session_documents
                     WHERE id = ?",
                )
                .bind(&row.id)
                .fetch_optional(&mut **transaction)
                .await?
            else {
                return Ok(None);
            };

            if deleted_at.is_some() || session_id != row.session_id {
                return Ok(None);
            }

            let existing_empty = document_body_is_empty(&body_format, &body);
            let incoming_empty = document_body_is_empty(&row.body_format, &row.body);
            if !existing_empty && incoming_empty {
                return Ok(Some(InsertOutcome::RetainedExisting));
            }

            let bodies_resolve =
                document_bodies_are_equivalent(&body_format, &body, row) || existing_empty;
            let existing_title_empty = title.trim().is_empty();
            let incoming_title_empty = row.title.trim().is_empty();
            let titles_resolve = existing_title_empty || incoming_title_empty || title == row.title;
            if !bodies_resolve || !titles_resolve {
                return Ok(None);
            }

            let fill_body = existing_empty && !incoming_empty;
            let fill_title = existing_title_empty && !incoming_title_empty;
            if !fill_body && !fill_title {
                return Ok(Some(InsertOutcome::RetainedExisting));
            }

            if !fill_body {
                let result = sqlx::query(
                    "UPDATE session_documents
                     SET title = ?
                     WHERE id = ?
                       AND session_id = ?
                       AND title IS ?
                       AND body_format IS ?
                       AND body IS ?
                       AND deleted_at IS NULL",
                )
                .bind(&row.title)
                .bind(&row.id)
                .bind(&row.session_id)
                .bind(&title)
                .bind(&body_format)
                .bind(&body)
                .execute(&mut **transaction)
                .await?;

                return Ok((result.rows_affected() == 1).then_some(InsertOutcome::FilledFromLegacy));
            }

            let result = sqlx::query(
                "UPDATE session_documents
                 SET kind = ?,
                     template_id = ?,
                     title = CASE WHEN ? THEN title ELSE ? END,
                     body_format = ?,
                     body = ?,
                     source_hash = ?,
                     sort_order = ?,
                     created_by = CASE WHEN ? = '' THEN created_by ELSE ? END,
                     updated_by = CASE WHEN ? = '' THEN updated_by ELSE ? END,
                     created_at = CASE WHEN ? = '' THEN created_at ELSE ? END,
                     updated_at = CASE WHEN ? = '' THEN updated_at ELSE ? END
                 WHERE id = ?
                   AND session_id = ?
                   AND title IS ?
                   AND body_format IS ?
                   AND body IS ?
                   AND deleted_at IS NULL",
            )
            .bind(&row.kind)
            .bind(&row.template_id)
            .bind(incoming_title_empty)
            .bind(&row.title)
            .bind(&row.body_format)
            .bind(&row.body)
            .bind(&row.source_hash)
            .bind(row.sort_order)
            .bind(&row.created_by)
            .bind(&row.created_by)
            .bind(&row.created_by)
            .bind(&row.created_by)
            .bind(&row.created_at)
            .bind(&row.created_at)
            .bind(&row.updated_at)
            .bind(&row.updated_at)
            .bind(&row.id)
            .bind(&row.session_id)
            .bind(&title)
            .bind(&body_format)
            .bind(&body)
            .execute(&mut **transaction)
            .await?;

            Ok((result.rows_affected() == 1).then_some(InsertOutcome::FilledFromLegacy))
        }
        LegacyImportRow::Transcript(row) => {
            let Some((session_id, memo, words_json, speaker_hints_json, deleted_at)) =
                sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
                    "SELECT session_id, memo, words_json, speaker_hints_json, deleted_at
                     FROM transcripts
                     WHERE id = ?",
                )
                .bind(&row.id)
                .fetch_optional(&mut **transaction)
                .await?
            else {
                return Ok(None);
            };

            if deleted_at.is_some() || session_id != row.session_id {
                return Ok(None);
            }

            let existing_empty =
                transcript_payload_is_empty(&memo, &words_json, &speaker_hints_json);
            let incoming_empty =
                transcript_payload_is_empty(&row.memo, &row.words_json, &row.speaker_hints_json);
            if transcript_payloads_are_equivalent(&memo, &words_json, &speaker_hints_json, row)
                || incoming_empty
            {
                return Ok(Some(InsertOutcome::RetainedExisting));
            }
            if !existing_empty {
                return Ok(None);
            }

            let result = sqlx::query(
                "UPDATE transcripts
                 SET owner_user_id = CASE WHEN owner_user_id = '' THEN ? ELSE owner_user_id END,
                     started_at_ms = ?,
                     ended_at_ms = ?,
                     memo = ?,
                     words_json = ?,
                     speaker_hints_json = ?,
                     created_at = CASE WHEN ? = '' THEN created_at ELSE ? END,
                     updated_at = CASE WHEN ? = '' THEN updated_at ELSE ? END
                 WHERE id = ?
                   AND session_id = ?
                   AND memo IS ?
                   AND words_json IS ?
                   AND speaker_hints_json IS ?
                   AND deleted_at IS NULL",
            )
            .bind(&row.owner_user_id)
            .bind(row.started_at_ms)
            .bind(row.ended_at_ms)
            .bind(&row.memo)
            .bind(&row.words_json)
            .bind(&row.speaker_hints_json)
            .bind(&row.created_at)
            .bind(&row.created_at)
            .bind(&row.created_at)
            .bind(&row.created_at)
            .bind(&row.id)
            .bind(&row.session_id)
            .bind(&memo)
            .bind(&words_json)
            .bind(&speaker_hints_json)
            .execute(&mut **transaction)
            .await?;

            Ok((result.rows_affected() == 1).then_some(InsertOutcome::FilledFromLegacy))
        }
        _ => Ok(None),
    }
}

fn document_bodies_are_equivalent(
    existing_format: &str,
    existing_body: &str,
    incoming: &LegacyDocument,
) -> bool {
    existing_format == incoming.body_format && existing_body == incoming.body
}

fn document_body_is_empty(body_format: &str, body: &str) -> bool {
    let body = body.trim();
    if body.is_empty() || body == "&nbsp;" {
        return true;
    }
    if body_format != "prosemirror_json" {
        return false;
    }

    let Ok(serde_json::Value::Object(document)) = serde_json::from_str(body) else {
        return false;
    };
    if document.get("type").and_then(serde_json::Value::as_str) != Some("doc") {
        return false;
    }
    let Some(content) = document.get("content") else {
        return true;
    };
    let Some(content) = content.as_array() else {
        return false;
    };

    content.is_empty()
        || content.iter().all(|node| {
            let Some(node) = node.as_object() else {
                return false;
            };
            node.get("type").and_then(serde_json::Value::as_str) == Some("paragraph")
                && node
                    .get("content")
                    .is_none_or(|content| content.as_array().is_some_and(Vec::is_empty))
        })
}

fn transcript_payloads_are_equivalent(
    existing_memo: &str,
    existing_words_json: &str,
    existing_speaker_hints_json: &str,
    incoming: &LegacyTranscript,
) -> bool {
    existing_memo == incoming.memo
        && json_payloads_are_equivalent(existing_words_json, &incoming.words_json)
        && json_payloads_are_equivalent(existing_speaker_hints_json, &incoming.speaker_hints_json)
}

fn transcript_payload_is_empty(memo: &str, words_json: &str, speaker_hints_json: &str) -> bool {
    memo.trim().is_empty()
        && json_array_is_empty(words_json)
        && json_array_is_empty(speaker_hints_json)
}

fn json_array_is_empty(value: &str) -> bool {
    let value = value.trim();
    value.is_empty()
        || matches!(
            serde_json::from_str::<serde_json::Value>(value),
            Ok(serde_json::Value::Array(items)) if items.is_empty()
        )
}

fn json_payloads_are_equivalent(left: &str, right: &str) -> bool {
    match (
        serde_json::from_str::<serde_json::Value>(left),
        serde_json::from_str::<serde_json::Value>(right),
    ) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn is_recovered_session_placeholder(metadata_json: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(metadata_json)
        .ok()
        .and_then(|metadata| {
            metadata
                .get("legacy_recovery")?
                .get("reason")?
                .as_str()
                .map(|reason| reason == "missing_session_metadata")
        })
        .unwrap_or(false)
}

pub(super) async fn row_matches_existing(
    transaction: &mut Transaction<'_, Sqlite>,
    row: &LegacyImportRow,
) -> Result<bool, sqlx::Error> {
    match row {
        LegacyImportRow::Calendar(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM calendars
               WHERE id = ?
                 AND tracking_id_calendar IS ?
                 AND name IS ?
                 AND enabled IS ?
                 AND provider IS ?
                 AND source IS ?
                 AND color IS ?
                 AND connection_id IS ?
             )",
            )
            .bind(&row.id)
            .bind(&row.tracking_id_calendar)
            .bind(&row.name)
            .bind(row.enabled)
            .bind(&row.provider)
            .bind(&row.source)
            .bind(&row.color)
            .bind(&row.connection_id)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::Event(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM events
               WHERE id = ?
                 AND tracking_id_event IS ?
                 AND calendar_id IS ?
                 AND title IS ?
                 AND started_at IS ?
                 AND ended_at IS ?
                 AND location IS ?
                 AND meeting_link IS ?
                 AND description IS ?
                 AND note IS ?
                 AND recurrence_series_id IS ?
                 AND has_recurrence_rules IS ?
                 AND is_all_day IS ?
                 AND provider IS ?
                 AND participants_json IS ?
             )",
            )
            .bind(&row.id)
            .bind(&row.tracking_id_event)
            .bind(&row.calendar_id)
            .bind(&row.title)
            .bind(&row.started_at)
            .bind(&row.ended_at)
            .bind(&row.location)
            .bind(&row.meeting_link)
            .bind(&row.description)
            .bind(&row.note)
            .bind(&row.recurrence_series_id)
            .bind(row.has_recurrence_rules)
            .bind(row.is_all_day)
            .bind(&row.provider)
            .bind(&row.participants_json)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::Template(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM templates
               WHERE id = ?
                 AND title IS ?
                 AND description IS ?
                 AND pinned IS ?
                 AND pin_order IS ?
                 AND category IS ?
                 AND targets_json IS ?
                 AND sections_json IS ?
             )",
            )
            .bind(&row.id)
            .bind(&row.title)
            .bind(&row.description)
            .bind(row.pinned)
            .bind(row.pin_order)
            .bind(&row.category)
            .bind(&row.targets_json)
            .bind(&row.sections_json)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::Organization(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM organizations
               WHERE id = ?
                 AND owner_user_id IS ?
                 AND name IS ?
                 AND memo IS ?
                 AND pinned IS ?
                 AND pin_order IS ?
             )",
            )
            .bind(&row.id)
            .bind(&row.owner_user_id)
            .bind(&row.name)
            .bind(&row.memo)
            .bind(row.pinned)
            .bind(row.pin_order)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::Human(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM humans
               WHERE id = ?
                 AND owner_user_id IS ?
                 AND organization_id IS ?
                 AND name IS ?
                 AND email IS ?
                 AND phone IS ?
                 AND job_title IS ?
                 AND linkedin_username IS ?
                 AND memo IS ?
                 AND pinned IS ?
                 AND pin_order IS ?
             )",
            )
            .bind(&row.id)
            .bind(&row.owner_user_id)
            .bind(&row.organization_id)
            .bind(&row.name)
            .bind(&row.email)
            .bind(&row.phone)
            .bind(&row.job_title)
            .bind(&row.linkedin_username)
            .bind(&row.memo)
            .bind(row.pinned)
            .bind(row.pin_order)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::Session(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM sessions
               WHERE id = ?
                 AND owner_user_id IS ?
                 AND title IS ?
                 AND created_at IS ?
                 AND started_at IS ?
                 AND ended_at IS ?
                 AND event_id IS ?
                 AND external_event_id IS ?
                 AND external_provider IS ?
                 AND series_id IS ?
                 AND event_json IS ?
                 AND metadata_json IS ?
                 AND folder_path IS ?
                 AND deleted_at IS NULL
             )",
            )
            .bind(&row.id)
            .bind(&row.owner_user_id)
            .bind(&row.title)
            .bind(&row.created_at)
            .bind(&row.started_at)
            .bind(&row.ended_at)
            .bind(&row.event_id)
            .bind(&row.external_event_id)
            .bind(&row.external_provider)
            .bind(&row.series_id)
            .bind(&row.event_json)
            .bind(&row.metadata_json)
            .bind(&row.folder_path)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::Document(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM session_documents
               WHERE id = ?
                 AND session_id IS ?
                 AND kind IS ?
                 AND template_id IS ?
                 AND title IS ?
                 AND body_format IS ?
                 AND body IS ?
                 AND source_hash IS ?
                 AND sort_order IS ?
                 AND created_by IS ?
                 AND created_at IS ?
                 AND updated_at IS ?
                 AND generation_metadata_json IS ?
                 AND deleted_at IS NULL
             )",
            )
            .bind(&row.id)
            .bind(&row.session_id)
            .bind(&row.kind)
            .bind(&row.template_id)
            .bind(&row.title)
            .bind(&row.body_format)
            .bind(&row.body)
            .bind(&row.source_hash)
            .bind(row.sort_order)
            .bind(&row.created_by)
            .bind(&row.created_at)
            .bind(&row.updated_at)
            .bind(&row.generation_metadata_json)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::Transcript(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM transcripts
               WHERE id = ?
                 AND owner_user_id IS ?
                 AND session_id IS ?
                 AND started_at_ms IS ?
                 AND ended_at_ms IS ?
                 AND memo IS ?
                 AND words_json IS ?
                 AND speaker_hints_json IS ?
                 AND created_at IS ?
                 AND metadata_json IS ?
                 AND deleted_at IS NULL
             )",
            )
            .bind(&row.id)
            .bind(&row.owner_user_id)
            .bind(&row.session_id)
            .bind(row.started_at_ms)
            .bind(row.ended_at_ms)
            .bind(&row.memo)
            .bind(&row.words_json)
            .bind(&row.speaker_hints_json)
            .bind(&row.created_at)
            .bind(&row.metadata_json)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::Participant(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM session_participants
               WHERE id = ?
                 AND owner_user_id IS ?
                 AND session_id IS ?
                 AND human_id IS ?
                 AND source IS ?
                 AND deleted_at IS NULL
             )",
            )
            .bind(&row.id)
            .bind(&row.owner_user_id)
            .bind(&row.session_id)
            .bind(&row.human_id)
            .bind(&row.source)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::ActionItem(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM action_items
               WHERE id = ?
                 AND created_by IS ?
                 AND session_id IS ?
                 AND source_type IS ?
                 AND source_id IS ?
                 AND source_order IS ?
                 AND status IS ?
                 AND text IS ?
                 AND body_json IS ?
                 AND due_at IS ?
                 AND deleted_at IS NULL
             )",
            )
            .bind(&row.id)
            .bind(&row.owner_user_id)
            .bind(&row.session_id)
            .bind(&row.source_type)
            .bind(&row.source_id)
            .bind(row.source_order)
            .bind(&row.status)
            .bind(&row.text)
            .bind(&row.body_json)
            .bind(&row.due_at)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::Attachment(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM session_attachments
               WHERE id = ?
                 AND session_id IS ?
                 AND filename IS ?
                 AND relative_path IS ?
                 AND content_type IS ?
                 AND size_bytes IS ?
                 AND sha256 IS ?
                 AND source_id IS ?
                 AND deleted_at IS NULL
             )",
            )
            .bind(&row.id)
            .bind(&row.session_id)
            .bind(&row.filename)
            .bind(&row.relative_path)
            .bind(&row.content_type)
            .bind(row.size_bytes)
            .bind(&row.sha256)
            .bind(&row.source_id)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::Tag(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM tags
               WHERE id = ? AND owner_user_id IS ? AND name IS ?
             )",
            )
            .bind(&row.id)
            .bind(&row.owner_user_id)
            .bind(&row.name)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::SessionTag(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM session_tags
               WHERE id = ?
                 AND owner_user_id IS ?
                 AND session_id IS ?
                 AND tag_id IS ?
                 AND deleted_at IS NULL
             )",
            )
            .bind(&row.id)
            .bind(&row.owner_user_id)
            .bind(&row.session_id)
            .bind(&row.tag_id)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::ChatGroup(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM chat_groups
               WHERE id = ?
                 AND owner_user_id IS ?
                 AND title IS ?
                 AND created_at IS ?
                 AND deleted_at IS NULL
             )",
            )
            .bind(&row.id)
            .bind(&row.owner_user_id)
            .bind(&row.title)
            .bind(&row.created_at)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::ChatMessage(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM chat_messages
               WHERE id = ?
                 AND chat_group_id IS ?
                 AND owner_user_id IS ?
                 AND role IS ?
                 AND content IS ?
                 AND metadata_json IS ?
                 AND parts_json IS ?
                 AND status IS ?
                 AND created_at IS ?
                 AND deleted_at IS NULL
             )",
            )
            .bind(&row.id)
            .bind(&row.chat_group_id)
            .bind(&row.owner_user_id)
            .bind(&row.role)
            .bind(&row.content)
            .bind(&row.metadata_json)
            .bind(&row.parts_json)
            .bind(&row.status)
            .bind(&row.created_at)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::DailyNote(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM daily_notes
               WHERE id = ?
                 AND owner_user_id IS ?
                 AND note_date IS ?
                 AND body_format IS ?
                 AND body IS ?
                 AND deleted_at IS NULL
             )",
            )
            .bind(&row.id)
            .bind(&row.owner_user_id)
            .bind(&row.note_date)
            .bind(&row.body_format)
            .bind(&row.body)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::AppSetting(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM app_settings
               WHERE id = ? AND value_json IS ?
             )",
            )
            .bind(&row.id)
            .bind(&row.value_json)
            .fetch_one(&mut **transaction)
            .await
        }
    }
}
