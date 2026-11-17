//! Contact writes, matching the shipping calendar plugin's contact commands over `db-app`.

use desktop_runtime::{Reply, Result, RuntimeHandle, ServiceError};
use serde_json::Value;
use sqlx::{SqliteConnection, SqlitePool};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Human,
    Organization,
}

impl Kind {
    fn table(self) -> &'static str {
        match self {
            Self::Human => "humans",
            Self::Organization => "organizations",
        }
    }
}

/// Editable person fields and their `humans` columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Name,
    JobTitle,
    Email,
    Phone,
    LinkedIn,
    Memo,
    Organization,
}

impl Field {
    fn assignment(self) -> &'static str {
        match self {
            Self::Name => "name = ?",
            Self::JobTitle => "job_title = ?",
            Self::Email => "email = ?",
            Self::Phone => "phone = ?",
            Self::LinkedIn => "linkedin_username = ?",
            Self::Memo => "memo = ?",
            Self::Organization => "organization_id = ?",
        }
    }
}

#[derive(Clone, Debug)]
pub enum Write {
    CreateHuman {
        id: String,
        owner: String,
        name: String,
    },
    CreateOrganization {
        id: String,
        owner: String,
        name: String,
    },
    UpdateHuman {
        id: String,
        field: Field,
        value: String,
    },
    AdditionalEmails {
        id: String,
        emails: Vec<String>,
    },
    RenameOrganization {
        id: String,
        name: String,
    },
    TogglePin(Kind, String),
    Reorder(Vec<(Kind, String)>),
    Delete(Kind, String),
    Avatar(Kind, String, Option<String>),
    Merge {
        selected: String,
        duplicate: String,
    },
    /// The calendar plugin's `apply_contact_enhancement`: (re)creates the person when asked,
    /// fills the given fields and links the company when the person has none.
    Enhance {
        id: String,
        owner: String,
        create: bool,
        name: Option<String>,
        email: Option<String>,
        company: Option<String>,
    },
}

fn failure(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Failed(error.to_string().into())
}

pub fn submit(runtime: &RuntimeHandle, write: Write) -> Result<Reply<()>> {
    runtime.submit(move |services| async move { apply(services.db.pool(), write).await })
}

pub async fn apply(pool: &SqlitePool, write: Write) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.map_err(failure)?;
    match run(&mut transaction, write, &now).await {
        Ok(()) => transaction.commit().await.map_err(failure),
        Err(error) => {
            let _ = transaction.rollback().await;
            Err(error)
        }
    }
}

async fn run(conn: &mut SqliteConnection, write: Write, now: &str) -> Result<()> {
    use anlg_db_app as db;
    match write {
        Write::CreateHuman { id, owner, name } => {
            db::create_human(conn, &id, &owner, &name, "", now)
                .await
                .map_err(failure)?;
        }
        Write::CreateOrganization { id, owner, name } => {
            db::create_organization(conn, &id, &owner, &name, now)
                .await
                .map_err(failure)?;
        }
        Write::UpdateHuman { id, field, value } => {
            db::update_contact_fields(
                conn,
                "humans",
                &[(field.assignment(), value.clone())],
                now,
                &id,
            )
            .await
            .map_err(failure)?;
            // A new primary must not also stay in the additional list.
            if field == Field::Email {
                let stored = stored_additional_emails(conn, &id).await?;
                let normalized = db::normalize_additional_emails(&value, stored.iter().cloned());
                if normalized != stored {
                    save_additional_emails(conn, &id, &normalized, now).await?;
                }
            }
        }
        Write::AdditionalEmails { id, emails } => {
            let primary: String =
                sqlx::query_scalar("SELECT email FROM humans WHERE id = ? AND deleted_at IS NULL")
                    .bind(&id)
                    .fetch_optional(&mut *conn)
                    .await
                    .map_err(failure)?
                    .unwrap_or_default();
            let normalized = db::normalize_additional_emails(&primary, emails);
            save_additional_emails(conn, &id, &normalized, now).await?;
        }
        Write::RenameOrganization { id, name } => {
            db::update_contact_fields(conn, "organizations", &[("name = ?", name)], now, &id)
                .await
                .map_err(failure)?;
        }
        Write::TogglePin(kind, id) => {
            db::toggle_contact_pin(conn, kind.table(), &id, now)
                .await
                .map_err(failure)?;
        }
        Write::Reorder(entries) => {
            for (index, (kind, id)) in entries.iter().enumerate() {
                db::set_pinned_order(conn, kind.table(), index as i64, id, now)
                    .await
                    .map_err(failure)?;
            }
        }
        Write::Delete(kind, id) => {
            db::soft_delete_contact(conn, kind.table(), &id, now)
                .await
                .map_err(failure)?;
        }
        Write::Avatar(kind, id, avatar) => {
            db::update_contact_avatar(conn, kind.table(), avatar.as_deref(), &id, now)
                .await
                .map_err(failure)?;
        }
        Write::Merge {
            selected,
            duplicate,
        } => merge(conn, &selected, &duplicate, now).await?,
        Write::Enhance {
            id,
            owner,
            create,
            name,
            email,
            company,
        } => {
            if create {
                db::upsert_revived_human(
                    conn,
                    &id,
                    &owner,
                    name.as_deref().unwrap_or_default(),
                    email.as_deref().unwrap_or_default(),
                    now,
                )
                .await
                .map_err(failure)?;
            }
            let company = company.unwrap_or_default();
            if !company.is_empty() {
                db::insert_organization_by_name_if_missing(
                    conn,
                    &uuid::Uuid::new_v4().to_string(),
                    &owner,
                    &company,
                    now,
                )
                .await
                .map_err(failure)?;
            }
            let mut assignments: Vec<(&'static str, String)> = Vec::new();
            assignments.extend(name.map(|name| ("name = ?", name)));
            assignments.extend(email.map(|email| ("email = ?", email)));
            if !company.is_empty() {
                assignments.push((
                    "organization_id = CASE WHEN organization_id = '' THEN COALESCE((SELECT id FROM organizations WHERE lower(name) = lower(?) AND deleted_at IS NULL ORDER BY created_at, id LIMIT 1), organization_id) ELSE organization_id END",
                    company,
                ));
            }
            if !assignments.is_empty() {
                db::update_contact_fields(conn, "humans", &assignments, now, &id)
                    .await
                    .map_err(failure)?;
            }
        }
    }
    Ok(())
}

async fn stored_additional_emails(conn: &mut SqliteConnection, id: &str) -> Result<Vec<String>> {
    let metadata: Option<String> =
        sqlx::query_scalar("SELECT metadata_json FROM humans WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(failure)?;
    Ok(anlg_db_app::human_additional_emails_from_metadata(
        metadata.as_deref().unwrap_or("{}"),
    ))
}

async fn save_additional_emails(
    conn: &mut SqliteConnection,
    id: &str,
    emails: &[String],
    now: &str,
) -> Result<()> {
    let json = (!emails.is_empty())
        .then(|| serde_json::to_string(emails))
        .transpose()
        .map_err(failure)?;
    anlg_db_app::update_human_additional_emails(conn, json.as_deref(), now, id)
        .await
        .map_err(failure)?;
    Ok(())
}

fn merge_text(primary: &str, duplicate: &str) -> String {
    match (primary.is_empty(), duplicate.is_empty()) {
        (_, true) => primary.to_owned(),
        (true, false) => duplicate.to_owned(),
        (false, false) => format!("{primary}, {duplicate}"),
    }
}

/// `merge_humans`: the self contact always survives; the duplicate's details, emails, note
/// participation and speaker assignments move onto the primary before it is deleted.
async fn merge(
    conn: &mut SqliteConnection,
    selected: &str,
    duplicate: &str,
    now: &str,
) -> Result<()> {
    use anlg_db_app as db;
    let rows = db::list_merge_humans(conn, selected, duplicate)
        .await
        .map_err(failure)?;
    let self_id = rows
        .iter()
        .find(|row| row.id == row.owner_user_id)
        .map(|row| row.id.as_str())
        .unwrap_or(if duplicate == db::DEFAULT_USER_ID {
            duplicate
        } else {
            selected
        });
    let (primary_id, duplicate_id) = if self_id == duplicate {
        (duplicate, selected)
    } else {
        (selected, duplicate)
    };
    let missing = || failure("Both contacts must exist before they can be merged");
    let primary = rows
        .iter()
        .find(|row| row.id == primary_id)
        .ok_or_else(missing)?;
    let other = rows
        .iter()
        .find(|row| row.id == duplicate_id)
        .ok_or_else(missing)?;
    let organization = if primary.organization_id.is_empty() {
        &other.organization_id
    } else {
        &primary.organization_id
    };
    let mut emails = db::human_additional_emails_from_metadata(&primary.metadata_json);
    let email = if primary.email.trim().is_empty() {
        other.email.clone()
    } else {
        emails.push(other.email.clone());
        primary.email.clone()
    };
    emails.extend(db::human_additional_emails_from_metadata(
        &other.metadata_json,
    ));
    let emails = db::normalize_additional_emails(&email, emails);
    let emails = (!emails.is_empty())
        .then(|| serde_json::to_string(&emails))
        .transpose()
        .map_err(failure)?;
    db::tombstone_duplicate_participant_mappings(conn, now, duplicate_id, primary_id)
        .await
        .map_err(failure)?;
    db::reassign_participant_mappings(conn, primary_id, duplicate_id, now)
        .await
        .map_err(failure)?;
    reassign_speakers(conn, primary_id, &primary.name, duplicate_id, now).await?;
    db::update_merged_human(
        conn,
        &merge_text(&primary.job_title, &other.job_title),
        &merge_text(&primary.linkedin_username, &other.linkedin_username),
        &merge_text(&primary.phone, &other.phone),
        &merge_text(&primary.memo, &other.memo),
        organization,
        &email,
        emails.as_deref(),
        now,
        primary_id,
    )
    .await
    .map_err(failure)?;
    db::soft_delete_contact(conn, "humans", duplicate_id, now)
        .await
        .map_err(failure)?;
    Ok(())
}

fn reassign(value: &mut Value, primary: &str, duplicate: &str) -> bool {
    if value.get("human_id").and_then(Value::as_str) != Some(duplicate) {
        return false;
    }
    value["human_id"] = Value::String(primary.to_owned());
    true
}

async fn reassign_speakers(
    conn: &mut SqliteConnection,
    primary: &str,
    primary_name: &str,
    duplicate: &str,
    now: &str,
) -> Result<()> {
    let transcripts: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, speaker_hints_json FROM transcripts WHERE instr(speaker_hints_json, ?) > 0",
    )
    .bind(duplicate)
    .fetch_all(&mut *conn)
    .await
    .map_err(failure)?;
    for (id, json) in transcripts {
        let Ok(mut hints) = serde_json::from_str::<Value>(&json) else {
            continue;
        };
        let mut changed = false;
        for hint in hints.as_array_mut().into_iter().flatten() {
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
                if reassign(&mut assignment, primary, duplicate) {
                    *value = Value::String(assignment.to_string());
                    changed = true;
                }
            } else {
                changed |= reassign(value, primary, duplicate);
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
            .map_err(failure)?;
        }
    }
    let sessions: Vec<(String, String)> =
        sqlx::query_as("SELECT id, metadata_json FROM sessions WHERE instr(metadata_json, ?) > 0")
            .bind(duplicate)
            .fetch_all(&mut *conn)
            .await
            .map_err(failure)?;
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
        for participant in intervals
            .iter_mut()
            .filter_map(|interval| interval.get_mut("participants")?.as_array_mut())
            .flatten()
        {
            changed |= reassign(participant, primary, duplicate);
            if !primary_name.trim().is_empty()
                && participant.get("human_id").and_then(Value::as_str) == Some(primary)
                && participant.get("name").and_then(Value::as_str) != Some(primary_name)
            {
                participant["name"] = Value::String(primary_name.to_owned());
                changed = true;
            }
        }
        if changed {
            sqlx::query("UPDATE sessions SET metadata_json = ?, updated_at = ? WHERE id = ?")
                .bind(metadata.to_string())
                .bind(now)
                .bind(id)
                .execute(&mut *conn)
                .await
                .map_err(failure)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use desktop_runtime::Profile;
    use futures::executor::block_on;

    use super::*;

    async fn start() -> (tempfile::TempDir, RuntimeHandle) {
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(Profile {
            database: directory.path().join("contacts.sqlite"),
        })
        .unwrap();
        ready.receive().await.unwrap();
        (directory, runtime)
    }

    async fn apply(runtime: &RuntimeHandle, write: Write) {
        submit(runtime, write).unwrap().receive().await.unwrap();
    }

    async fn sql(runtime: &RuntimeHandle, sql: &'static str) -> Value {
        runtime
            .submit(move |services| async move {
                services
                    .executor
                    .execute(sql.into(), vec![])
                    .await
                    .map_err(failure)
            })
            .unwrap()
            .receive()
            .await
            .unwrap()
            .into_iter()
            .next()
            .map(|row| row["value"].clone())
            .unwrap_or(Value::Null)
    }

    #[test]
    fn a_new_primary_email_leaves_the_additional_list() {
        block_on(async {
            let (_directory, runtime) = start().await;
            apply(
                &runtime,
                Write::CreateHuman {
                    id: "h".into(),
                    owner: "u".into(),
                    name: "Ada".into(),
                },
            )
            .await;
            apply(
                &runtime,
                Write::AdditionalEmails {
                    id: "h".into(),
                    emails: vec![
                        "a@x.com".into(),
                        " A@X.com ".into(),
                        "b@x.com".into(),
                        "".into(),
                    ],
                },
            )
            .await;
            assert_eq!(
                sql(
                    &runtime,
                    "SELECT json_extract(metadata_json, '$.additionalEmails') AS value FROM humans"
                )
                .await,
                r#"["a@x.com","b@x.com"]"#
            );
            apply(
                &runtime,
                Write::UpdateHuman {
                    id: "h".into(),
                    field: Field::Email,
                    value: "B@x.com".into(),
                },
            )
            .await;
            assert_eq!(
                sql(&runtime, "SELECT email || ' ' || json_extract(metadata_json, '$.additionalEmails') AS value FROM humans").await,
                r#"B@x.com ["a@x.com"]"#
            );
            runtime.shutdown().await.unwrap();
        });
    }

    #[test]
    fn merging_into_someone_else_keeps_yourself_and_moves_their_notes() {
        block_on(async {
            let (_directory, runtime) = start().await;
            runtime
                .submit(|services| async move {
                    sqlx::raw_sql(
                        "INSERT INTO humans (id, owner_user_id, name, email, phone, memo) VALUES
                           ('me', 'me', 'Me', 'me@x.com', '', 'Mine'),
                           ('dup', 'me', 'Me too', 'alt@x.com', '123', 'Theirs');
                         INSERT INTO sessions (id) VALUES ('s1'), ('s2');
                         INSERT INTO session_participants (id, session_id, human_id) VALUES
                           ('p1', 's1', 'dup'), ('p2', 's1', 'me'), ('p3', 's2', 'dup');",
                    )
                    .execute(services.db.pool())
                    .await
                    .map_err(failure)
                    .map(|_| ())
                })
                .unwrap()
                .receive()
                .await
                .unwrap();
            apply(
                &runtime,
                Write::Merge {
                    selected: "dup".into(),
                    duplicate: "me".into(),
                },
            )
            .await;
            assert_eq!(
                sql(
                    &runtime,
                    "SELECT group_concat(id) AS value FROM humans WHERE deleted_at IS NULL"
                )
                .await,
                "me"
            );
            assert_eq!(
                sql(&runtime, "SELECT phone || '|' || memo || '|' || json_extract(metadata_json, '$.additionalEmails') AS value FROM humans WHERE id = 'me'").await,
                r#"123|Mine, Theirs|["alt@x.com"]"#
            );
            assert_eq!(
                sql(&runtime, "SELECT group_concat(session_id) AS value FROM (SELECT session_id FROM session_participants WHERE human_id = 'me' AND deleted_at IS NULL ORDER BY session_id)").await,
                "s1,s2"
            );
            runtime.shutdown().await.unwrap();
        });
    }

    #[test]
    fn pins_toggle_across_both_tables_and_reorder_by_position() {
        block_on(async {
            let (_directory, runtime) = start().await;
            runtime
                .submit(|services| async move {
                    sqlx::raw_sql(
                        "INSERT INTO humans (id, name) VALUES ('h', 'H');
                         INSERT INTO organizations (id, name, pinned, pin_order) VALUES ('o', 'O', 1, 4);",
                    )
                    .execute(services.db.pool())
                    .await
                    .map_err(failure)
                    .map(|_| ())
                })
                .unwrap()
                .receive()
                .await
                .unwrap();
            apply(&runtime, Write::TogglePin(Kind::Human, "h".into())).await;
            assert_eq!(
                sql(
                    &runtime,
                    "SELECT pinned || ':' || pin_order AS value FROM humans"
                )
                .await,
                "1:5"
            );
            apply(
                &runtime,
                Write::Reorder(vec![
                    (Kind::Human, "h".into()),
                    (Kind::Organization, "o".into()),
                ]),
            )
            .await;
            assert_eq!(
                sql(&runtime, "SELECT (SELECT pin_order FROM humans) || (SELECT pin_order FROM organizations) AS value").await,
                "01"
            );
            runtime.shutdown().await.unwrap();
        });
    }
}
