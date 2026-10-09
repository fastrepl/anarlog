use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;
use sqlx::{SqliteConnection, SqlitePool};

use crate::contacts::run_write;
use crate::storage::transaction_utils::js_iso8601_timestamp;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct MergeHumansRequest {
    pub selected_human_id: String,
    pub duplicate_human_id: String,
}

fn merge_text(primary: &str, duplicate: &str) -> String {
    if duplicate.is_empty() {
        return primary.to_string();
    }
    if !primary.is_empty() {
        return format!("{primary}, {duplicate}");
    }
    duplicate.to_string()
}

fn reassign_human_id(value: &mut Value, primary_id: &str, duplicate_id: &str) -> bool {
    if value.get("human_id").and_then(Value::as_str) != Some(duplicate_id) {
        return false;
    }
    value["human_id"] = Value::String(primary_id.to_owned());
    true
}

async fn reassign_speaker_references(
    conn: &mut SqliteConnection,
    primary_id: &str,
    primary_name: &str,
    duplicate_id: &str,
    now: &str,
) -> Result<(), String> {
    let transcripts: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, speaker_hints_json FROM transcripts WHERE instr(speaker_hints_json, ?) > 0",
    )
    .bind(duplicate_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(|error| error.to_string())?;
    for (id, json) in transcripts {
        let Ok(mut hints) = serde_json::from_str::<Value>(&json) else {
            continue;
        };
        let Some(hints_array) = hints.as_array_mut() else {
            continue;
        };
        let mut changed = false;
        for hint in hints_array {
            if !matches!(
                hint.get("type").and_then(Value::as_str),
                Some("automatic_speaker_assignment" | "user_speaker_assignment")
            ) {
                continue;
            }
            let Some(value) = hint.get_mut("value") else {
                continue;
            };
            // Older imports encode assignment values as JSON strings.
            if let Some(encoded) = value.as_str() {
                let Ok(mut assignment) = serde_json::from_str::<Value>(encoded) else {
                    continue;
                };
                if reassign_human_id(&mut assignment, primary_id, duplicate_id) {
                    *value = Value::String(assignment.to_string());
                    changed = true;
                }
            } else {
                changed |= reassign_human_id(value, primary_id, duplicate_id);
            }
        }
        if changed {
            sqlx::query(
                "UPDATE transcripts SET speaker_hints_json = ?, updated_at = ? WHERE id = ?",
            )
            .bind(hints.to_string())
            .bind(now)
            .bind(id)
            .execute(&mut *conn)
            .await
            .map_err(|error| error.to_string())?;
        }
    }

    let sessions: Vec<(String, String)> =
        sqlx::query_as("SELECT id, metadata_json FROM sessions WHERE instr(metadata_json, ?) > 0")
            .bind(duplicate_id)
            .fetch_all(&mut *conn)
            .await
            .map_err(|error| error.to_string())?;
    for (id, json) in sessions {
        let Ok(mut metadata) = serde_json::from_str::<Value>(&json) else {
            continue;
        };
        let Some(intervals) = metadata
            .pointer_mut("/speaker_context/intervals")
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        let mut changed = false;
        for interval in intervals {
            if let Some(participants) = interval
                .get_mut("participants")
                .and_then(Value::as_array_mut)
            {
                for participant in participants {
                    changed |= reassign_human_id(participant, primary_id, duplicate_id);
                    if !primary_name.trim().is_empty()
                        && participant.get("human_id").and_then(Value::as_str) == Some(primary_id)
                        && participant.get("name").and_then(Value::as_str) != Some(primary_name)
                    {
                        participant["name"] = Value::String(primary_name.to_owned());
                        changed = true;
                    }
                }
            }
        }
        if changed {
            sqlx::query("UPDATE sessions SET metadata_json = ?, updated_at = ? WHERE id = ?")
                .bind(metadata.to_string())
                .bind(now)
                .bind(id)
                .execute(&mut *conn)
                .await
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

pub async fn merge_humans(pool: &SqlitePool, request: MergeHumansRequest) -> Result<(), String> {
    let now = js_iso8601_timestamp();
    let selected = request.selected_human_id;
    let duplicate = request.duplicate_human_id;
    run_write(pool, |conn| {
        let now = now.clone();
        Box::pin(async move {
            let rows = anlg_db_app::list_merge_humans(conn, &selected, &duplicate)
                .await
                .map_err(|error| error.to_string())?;
            let self_human_id = rows
                .iter()
                .find(|row| row.id == row.owner_user_id)
                .map(|row| row.id.clone())
                .unwrap_or_else(|| {
                    if duplicate == anlg_db_app::DEFAULT_USER_ID {
                        duplicate.clone()
                    } else {
                        selected.clone()
                    }
                });
            let primary_id = if self_human_id == duplicate {
                duplicate.clone()
            } else {
                selected.clone()
            };
            let duplicate_id = if primary_id == selected {
                duplicate.clone()
            } else {
                selected.clone()
            };
            let Some(primary) = rows.iter().find(|row| row.id == primary_id) else {
                return Err("Both contacts must exist before they can be merged".to_string());
            };
            let Some(duplicate_row) = rows.iter().find(|row| row.id == duplicate_id) else {
                return Err("Both contacts must exist before they can be merged".to_string());
            };
            let organization_id = if primary.organization_id.is_empty() {
                duplicate_row.organization_id.clone()
            } else {
                primary.organization_id.clone()
            };
            let job_title = merge_text(&primary.job_title, &duplicate_row.job_title);
            let linkedin_username =
                merge_text(&primary.linkedin_username, &duplicate_row.linkedin_username);
            let phone = merge_text(&primary.phone, &duplicate_row.phone);
            let memo = merge_text(&primary.memo, &duplicate_row.memo);

            let mut extra_emails =
                anlg_db_app::human_additional_emails_from_metadata(&primary.metadata_json);
            let email = if primary.email.trim().is_empty() {
                duplicate_row.email.clone()
            } else {
                extra_emails.push(duplicate_row.email.clone());
                primary.email.clone()
            };
            extra_emails.extend(anlg_db_app::human_additional_emails_from_metadata(
                &duplicate_row.metadata_json,
            ));
            let additional_emails = anlg_db_app::normalize_additional_emails(&email, extra_emails);
            let additional_emails_json = if additional_emails.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&additional_emails).map_err(|error| error.to_string())?)
            };

            anlg_db_app::tombstone_duplicate_participant_mappings(
                conn,
                &now,
                &duplicate_id,
                &primary_id,
            )
            .await
            .map_err(|error| error.to_string())?;
            anlg_db_app::reassign_participant_mappings(conn, &primary_id, &duplicate_id, &now)
                .await
                .map_err(|error| error.to_string())?;
            reassign_speaker_references(conn, &primary_id, &primary.name, &duplicate_id, &now)
                .await?;
            anlg_db_app::update_merged_human(
                conn,
                &job_title,
                &linkedin_username,
                &phone,
                &memo,
                &organization_id,
                &email,
                additional_emails_json.as_deref(),
                &now,
                &primary_id,
            )
            .await
            .map_err(|error| error.to_string())?;
            anlg_db_app::soft_delete_contact(conn, "humans", &duplicate_id, &now)
                .await
                .map_err(|error| error.to_string())?;
            Ok(())
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_db_core::Db;

    async fn test_db() -> Db {
        let db = Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        db
    }

    async fn seed_human(db: &Db, id: &str, owner: &str, org: &str, title: &str, memo: &str) {
        sqlx::query(
            "INSERT INTO humans (id, owner_user_id, name, organization_id, job_title, memo)
             VALUES (?, ?, 'N', ?, ?, ?)",
        )
        .bind(id)
        .bind(owner)
        .bind(org)
        .bind(title)
        .bind(memo)
        .execute(db.pool())
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn merging_contacts_preserves_transcript_assignments_and_context() {
        let db = test_db().await;
        seed_human(&db, "primary", "u", "", "Engineer", "Primary").await;
        seed_human(&db, "duplicate", "u", "org-1", "Founder", "Duplicate").await;
        sqlx::query("INSERT INTO sessions (id, owner_user_id) VALUES ('s1', 'u'), ('s2', 'u')")
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO session_participants (id, session_id, human_id, source)
             VALUES ('m1', 's1', 'duplicate', 'invite'),
                    ('m2', 's1', 'primary', 'invite'),
                    ('m3', 's2', 'duplicate', 'invite')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let hints = serde_json::json!([
            {"word_id": "w1", "type": "automatic_speaker_assignment", "value": {"human_id": "duplicate", "scope": "speaker", "channel": 1, "speaker_index": 0}},
            {"word_id": "w2", "type": "user_speaker_assignment", "value": {"human_id": "duplicate", "scope": "segment", "word_ids": ["w2", "w3"], "extra": true}},
            {"word_id": "w4", "type": "user_speaker_assignment", "value": "{\"human_id\":\"duplicate\",\"scope\":\"speaker\",\"channel\":0}"},
            {"word_id": "w5", "type": "user_speaker_assignment", "value": {"human_id": "other"}},
            {"word_id": "w6", "type": "provider_speaker_index", "value": {"human_id": "duplicate", "speaker_index": 1}}
        ]);
        sqlx::query(
            "INSERT INTO transcripts (id, session_id, speaker_hints_json) VALUES ('t1', 's1', ?)",
        )
        .bind(hints.to_string())
        .execute(db.pool())
        .await
        .unwrap();
        let metadata = serde_json::json!({
            "unrelated": {"human_id": "duplicate"},
            "speaker_context": {"intervals": [{"start_ms": 0, "participants": [
                {"human_id": "duplicate", "name": "Historical name"},
                {"human_id": "other", "name": "Other"}
            ]}]}
        });
        sqlx::query("UPDATE sessions SET metadata_json = ? WHERE id = 's1'")
            .bind(metadata.to_string())
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query(
            "UPDATE sessions SET metadata_json = ?, deleted_at = '2026-10-01' WHERE id = 's2'",
        )
        .bind(metadata.to_string())
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query("INSERT INTO transcripts (id, session_id, speaker_hints_json, deleted_at) VALUES ('t2', 's2', ?, '2026-10-01')")
            .bind(hints.to_string())
            .execute(db.pool()).await.unwrap();

        merge_humans(
            db.pool(),
            MergeHumansRequest {
                selected_human_id: "primary".to_string(),
                duplicate_human_id: "duplicate".to_string(),
            },
        )
        .await
        .unwrap();

        // Mapping m1: duplicate already had primary in s1 -> tombstoned.
        // Mapping m3: no primary counterpart -> reassigned to primary.
        let mappings: Vec<(String, String, bool)> = sqlx::query_as(
            "SELECT id, human_id, deleted_at IS NOT NULL FROM session_participants ORDER BY id",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(
            mappings,
            vec![
                ("m1".to_string(), "duplicate".to_string(), true),
                ("m2".to_string(), "primary".to_string(), false),
                ("m3".to_string(), "primary".to_string(), false),
            ]
        );

        let (job_title, memo, org, deleted): (String, String, String, Option<String>) =
            sqlx::query_as(
                "SELECT job_title, memo, organization_id, deleted_at FROM humans WHERE id = 'primary'",
            )
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(job_title, "Engineer, Founder");
        assert_eq!(memo, "Primary, Duplicate");
        assert_eq!(org, "org-1");
        assert!(deleted.is_none());

        let dup_deleted: Option<String> =
            sqlx::query_scalar("SELECT deleted_at FROM humans WHERE id = 'duplicate'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert!(dup_deleted.is_some());

        let stored_hints: String =
            sqlx::query_scalar("SELECT speaker_hints_json FROM transcripts WHERE id = 't1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let mut expected_hints = hints;
        expected_hints[0]["value"]["human_id"] = serde_json::json!("primary");
        expected_hints[1]["value"]["human_id"] = serde_json::json!("primary");
        expected_hints[2]["value"] = serde_json::json!(
            serde_json::json!({"human_id": "primary", "scope": "speaker", "channel": 0})
                .to_string()
        );
        assert_eq!(
            serde_json::from_str::<Value>(&stored_hints).unwrap(),
            expected_hints
        );
        let stored_metadata: String =
            sqlx::query_scalar("SELECT metadata_json FROM sessions WHERE id = 's1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let mut expected_metadata = metadata;
        expected_metadata["speaker_context"]["intervals"][0]["participants"][0]["human_id"] =
            serde_json::json!("primary");
        expected_metadata["speaker_context"]["intervals"][0]["participants"][0]["name"] =
            serde_json::json!("N");
        assert_eq!(
            serde_json::from_str::<Value>(&stored_metadata).unwrap(),
            expected_metadata
        );
        let (deleted_hints, deleted_at): (String, String) = sqlx::query_as(
            "SELECT speaker_hints_json, deleted_at FROM transcripts WHERE id = 't2'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&deleted_hints).unwrap(),
            expected_hints
        );
        assert_eq!(deleted_at, "2026-10-01");
        sqlx::query("UPDATE sessions SET deleted_at = NULL WHERE id = 's2'")
            .execute(db.pool())
            .await
            .unwrap();
        let restored_context: String = sqlx::query_scalar(
            "SELECT metadata_json FROM sessions WHERE id = 's2' AND deleted_at IS NULL",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&restored_context).unwrap(),
            expected_metadata
        );
        let mut conn = db.pool().acquire().await.unwrap();
        assert_eq!(
            anlg_db_app::load_render_humans(&mut conn, &["primary".to_string()])
                .await
                .unwrap(),
            vec![("primary".to_string(), "N".to_string())]
        );
    }

    #[tokio::test]
    async fn malformed_speaker_data_does_not_block_contact_merges() {
        let db = test_db().await;
        seed_human(&db, "primary", "u", "", "", "").await;
        seed_human(&db, "duplicate", "u", "", "", "").await;
        let hints = serde_json::json!([
            {"type": "user_speaker_assignment", "value": {"human_id": "duplicate"}},
            {"type": "user_speaker_assignment", "value": "unknown"}
        ]);
        for (id, json) in [
            ("valid", hints.to_string()),
            ("malformed", "{duplicate".to_owned()),
            ("wrong-shape", r#"{"human_id":"duplicate"}"#.to_owned()),
        ] {
            sqlx::query(
                "INSERT INTO sessions (id, owner_user_id, metadata_json) VALUES (?, 'u', ?)",
            )
            .bind(id)
            .bind(&json)
            .execute(db.pool())
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO transcripts (id, session_id, speaker_hints_json) VALUES (?, ?, ?)",
            )
            .bind(id)
            .bind(id)
            .bind(&json)
            .execute(db.pool())
            .await
            .unwrap();
        }

        merge_humans(
            db.pool(),
            MergeHumansRequest {
                selected_human_id: "primary".to_owned(),
                duplicate_human_id: "duplicate".to_owned(),
            },
        )
        .await
        .unwrap();

        let stored: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT transcripts.id, speaker_hints_json, sessions.metadata_json FROM transcripts JOIN sessions ON sessions.id = transcripts.session_id ORDER BY transcripts.id",
        ).fetch_all(db.pool()).await.unwrap();
        assert_eq!(stored[0].1, "{duplicate");
        assert_eq!(stored[0].2, "{duplicate");
        let mut expected = hints.clone();
        expected[0]["value"]["human_id"] = serde_json::json!("primary");
        assert_eq!(
            serde_json::from_str::<Value>(&stored[1].1).unwrap(),
            expected
        );
        assert_eq!(serde_json::from_str::<Value>(&stored[1].2).unwrap(), hints);
        assert_eq!(stored[2].1, r#"{"human_id":"duplicate"}"#);
        assert_eq!(stored[2].2, r#"{"human_id":"duplicate"}"#);
        let deleted: bool =
            sqlx::query_scalar("SELECT deleted_at IS NOT NULL FROM humans WHERE id = 'duplicate'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert!(deleted);
    }

    #[tokio::test]
    async fn merging_contacts_keeps_both_contacts_emails() {
        let db = test_db().await;
        sqlx::query(
            "INSERT INTO humans (id, owner_user_id, name, email, metadata_json)
             VALUES
               ('primary', 'u', 'N', 'primary@example.com',
                 json_object('additionalEmails', json_array('alias@example.com'), 'other', 'keep')),
               ('duplicate', 'u', 'N', 'dup@example.com',
                 json_object('additionalEmails', json_array('dup-alias@example.com', 'DUP@example.com')))",
        )
        .execute(db.pool())
        .await
        .unwrap();

        merge_humans(
            db.pool(),
            MergeHumansRequest {
                selected_human_id: "primary".to_string(),
                duplicate_human_id: "duplicate".to_string(),
            },
        )
        .await
        .unwrap();

        let (email, metadata_json): (String, String) =
            sqlx::query_as("SELECT email, metadata_json FROM humans WHERE id = 'primary'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(email, "primary@example.com");
        let metadata: serde_json::Value = serde_json::from_str(&metadata_json).unwrap();
        assert_eq!(metadata["other"], "keep");
        assert_eq!(
            metadata["additionalEmails"],
            serde_json::json!([
                "alias@example.com",
                "dup@example.com",
                "dup-alias@example.com"
            ])
        );
    }

    #[tokio::test]
    async fn merging_into_a_contact_without_email_takes_the_duplicates_primary() {
        let db = test_db().await;
        sqlx::query(
            "INSERT INTO humans (id, owner_user_id, name, email, metadata_json)
             VALUES
               ('primary', 'u', 'N', '', '{}'),
               ('duplicate', 'u', 'N', 'dup@example.com',
                 json_object('additionalEmails', json_array('dup-alias@example.com')))",
        )
        .execute(db.pool())
        .await
        .unwrap();

        merge_humans(
            db.pool(),
            MergeHumansRequest {
                selected_human_id: "primary".to_string(),
                duplicate_human_id: "duplicate".to_string(),
            },
        )
        .await
        .unwrap();

        let (email, metadata_json): (String, String) =
            sqlx::query_as("SELECT email, metadata_json FROM humans WHERE id = 'primary'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(email, "dup@example.com");
        let metadata: serde_json::Value = serde_json::from_str(&metadata_json).unwrap();
        assert_eq!(
            metadata["additionalEmails"],
            serde_json::json!(["dup-alias@example.com"])
        );
    }

    #[tokio::test]
    async fn keeps_the_bound_self_human_when_selected_as_duplicate() {
        let db = test_db().await;
        seed_human(&db, "other", "user-1", "", "", "").await;
        // Bound self human: id == owner_user_id.
        seed_human(&db, "user-1", "user-1", "", "", "").await;

        merge_humans(
            db.pool(),
            MergeHumansRequest {
                selected_human_id: "other".to_string(),
                duplicate_human_id: "user-1".to_string(),
            },
        )
        .await
        .unwrap();

        let deleted: Vec<(String, Option<String>)> =
            sqlx::query_as("SELECT id, deleted_at FROM humans ORDER BY id")
                .fetch_all(db.pool())
                .await
                .unwrap();
        assert_eq!(deleted[0].0, "other");
        assert!(deleted[0].1.is_some());
        assert_eq!(deleted[1].0, "user-1");
        assert!(deleted[1].1.is_none());
    }

    #[tokio::test]
    async fn missing_contact_errors_without_writes() {
        let db = test_db().await;
        seed_human(&db, "primary", "u", "", "", "").await;

        let error = merge_humans(
            db.pool(),
            MergeHumansRequest {
                selected_human_id: "primary".to_string(),
                duplicate_human_id: "missing".to_string(),
            },
        )
        .await
        .unwrap_err();
        assert_eq!(error, "Both contacts must exist before they can be merged");

        let deleted: Option<String> =
            sqlx::query_scalar("SELECT deleted_at FROM humans WHERE id = 'primary'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert!(deleted.is_none());
    }

    #[tokio::test]
    async fn a_mid_transaction_failure_rolls_back_earlier_statements() {
        let db = test_db().await;
        seed_human(&db, "primary", "u", "", "Eng", "").await;
        seed_human(&db, "duplicate", "u", "", "", "").await;
        sqlx::query("INSERT INTO sessions (id, owner_user_id) VALUES ('s1', 'u')")
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO session_participants (id, session_id, human_id, source)
             VALUES ('m1', 's1', 'duplicate', 'invite')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        let hints = r#"[{"word_id":"w1","type":"user_speaker_assignment","value":{"human_id":"duplicate"}}]"#;
        sqlx::query(
            "INSERT INTO transcripts (id, session_id, speaker_hints_json) VALUES ('t1', 's1', ?)",
        )
        .bind(hints)
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "CREATE TRIGGER block_human_updates BEFORE UPDATE ON humans
             WHEN NEW.deleted_at IS NOT NULL BEGIN SELECT RAISE(ABORT, 'boom'); END",
        )
        .execute(db.pool())
        .await
        .unwrap();

        merge_humans(
            db.pool(),
            MergeHumansRequest {
                selected_human_id: "primary".to_string(),
                duplicate_human_id: "duplicate".to_string(),
            },
        )
        .await
        .unwrap_err();

        let mapping: (String, Option<String>) =
            sqlx::query_as("SELECT human_id, deleted_at FROM session_participants WHERE id = 'm1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(mapping.0, "duplicate");
        assert!(mapping.1.is_none());
        let dup_deleted: Option<String> =
            sqlx::query_scalar("SELECT deleted_at FROM humans WHERE id = 'duplicate'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert!(dup_deleted.is_none());
        let stored_hints: String =
            sqlx::query_scalar("SELECT speaker_hints_json FROM transcripts WHERE id = 't1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(stored_hints, hints);
    }
}
