use sqlx::SqliteConnection;

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct MergeHumanRow {
    pub id: String,
    pub name: String,
    pub owner_user_id: String,
    pub organization_id: String,
    pub email: String,
    pub job_title: String,
    pub linkedin_username: String,
    pub phone: String,
    pub memo: String,
    pub metadata_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AvatarChange {
    Set(String),
    Remove,
    Keep,
}

const WORKSPACE_BINDING_SQL: &str = "NULLIF((
              SELECT json_extract(value_json, '$.workspace_id')
              FROM app_settings
              WHERE id = 'cloudsync_workspace_binding'
            ), '')";

const VALID_METADATA_SQL: &str =
    "CASE WHEN json_valid(metadata_json) THEN metadata_json ELSE '{}' END";

const VALID_HUMANS_METADATA_SQL: &str =
    "CASE WHEN json_valid(humans.metadata_json) THEN humans.metadata_json ELSE '{}' END";

/// Guarded `json_each` over `$.additionalEmails`: one row per additional email,
/// empty when the metadata is malformed or the value is not an array.
pub fn human_additional_emails_json_each_sql(alias: &str) -> String {
    format!(
        "json_each(CASE WHEN json_valid({alias}.metadata_json) AND json_type({alias}.metadata_json, '$.additionalEmails') = 'array' THEN json_extract({alias}.metadata_json, '$.additionalEmails') ELSE '[]' END)"
    )
}

/// `email_sql` matches the human's primary or any additional email,
/// case-insensitively. `email_sql` is a SQL expression (e.g. `?`), not a literal.
pub fn human_has_email_sql(alias: &str, email_sql: &str) -> String {
    format!(
        "(lower({alias}.email) = lower({email_sql}) OR EXISTS (SELECT 1 FROM {} AS additional_email WHERE lower(additional_email.value) = lower({email_sql})))",
        human_additional_emails_json_each_sql(alias)
    )
}

pub fn human_additional_emails_from_metadata(metadata_json: &str) -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(metadata_json)
        .ok()
        .and_then(|value| {
            value.get("additionalEmails")?.as_array().map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_string))
                    .collect()
            })
        })
        .unwrap_or_default()
}

/// Trimmed, no empties, deduped case-insensitively (first spelling wins),
/// never equal to the primary (case-insensitive), order preserved.
pub fn normalize_additional_emails(
    primary_email: &str,
    emails: impl IntoIterator<Item = String>,
) -> Vec<String> {
    let primary = primary_email.trim().to_lowercase();
    let mut seen = std::collections::HashSet::new();
    let mut normalized = Vec::new();
    for email in emails {
        let trimmed = email.trim();
        let key = trimmed.to_lowercase();
        if key.is_empty() || key == primary || !seen.insert(key) {
            continue;
        }
        normalized.push(trimmed.to_string());
    }
    normalized
}

pub async fn create_human(
    conn: &mut SqliteConnection,
    id: &str,
    owner_user_id: &str,
    name: &str,
    email: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(concat!(
        "INSERT INTO humans (
            id, workspace_id, owner_user_id, organization_id, name, email,
            phone, job_title, linkedin_username, memo, pinned, pin_order,
            metadata_json, created_at, updated_at, deleted_at
          ) VALUES (
            ?, NULLIF((
              SELECT json_extract(value_json, '$.workspace_id')
              FROM app_settings
              WHERE id = 'cloudsync_workspace_binding'
            ), ''), COALESCE(
              (SELECT library_workspace_id FROM local_library_connections WHERE active = 1),
              NULLIF(NULLIF(?, ''), '",
        "00000000-0000-0000-0000-000000000000",
        "'),
              NULLIF((
                SELECT json_extract(value_json, '$.workspace_id')
                FROM app_settings
                WHERE id = 'cloudsync_workspace_binding'
              ), ''),
              '",
        "00000000-0000-0000-0000-000000000000",
        "'
            ), '', ?, ?, '', '', '', '', 0, NULL, '{}', ?, ?, NULL
          )"
    ))
    .bind(id)
    .bind(owner_user_id)
    .bind(name)
    .bind(email)
    .bind(now)
    .bind(now)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn create_organization(
    conn: &mut SqliteConnection,
    id: &str,
    owner_user_id: &str,
    name: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(concat!(
        "INSERT INTO organizations (
            id, workspace_id, owner_user_id, name, memo, pinned, pin_order,
            metadata_json, created_at, updated_at, deleted_at
          ) VALUES (
            ?, NULLIF((
              SELECT json_extract(value_json, '$.workspace_id')
              FROM app_settings
              WHERE id = 'cloudsync_workspace_binding'
            ), ''), COALESCE(
              (SELECT library_workspace_id FROM local_library_connections WHERE active = 1),
              NULLIF(NULLIF(?, ''), '",
        "00000000-0000-0000-0000-000000000000",
        "'),
              NULLIF((
                SELECT json_extract(value_json, '$.workspace_id')
                FROM app_settings
                WHERE id = 'cloudsync_workspace_binding'
              ), ''),
              '",
        "00000000-0000-0000-0000-000000000000",
        "'
            ), ?, '', 0, NULL, '{}', ?, ?, NULL
          )"
    ))
    .bind(id)
    .bind(owner_user_id)
    .bind(name)
    .bind(now)
    .bind(now)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

#[allow(clippy::too_many_arguments)]
pub async fn upsert_personal_contact(
    conn: &mut SqliteConnection,
    human_id: &str,
    name: &str,
    email: &str,
    phone: &str,
    job_title: &str,
    linkedin_username: &str,
    memo: &str,
    organization_id: &str,
    avatar: &AvatarChange,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let insert_metadata = match avatar {
        AvatarChange::Set(_) => "json_object('avatarDataUrl', ?)",
        AvatarChange::Remove | AvatarChange::Keep => "'{}'",
    };
    let update_metadata = match avatar {
        AvatarChange::Set(_) => {
            format!("json_set({VALID_HUMANS_METADATA_SQL}, '$.avatarDataUrl', ?)")
        }
        AvatarChange::Remove => {
            format!("json_remove({VALID_HUMANS_METADATA_SQL}, '$.avatarDataUrl')")
        }
        AvatarChange::Keep => VALID_HUMANS_METADATA_SQL.to_string(),
    };
    let sql = format!(
        "INSERT INTO humans (
          id, workspace_id, owner_user_id, name, email, phone, job_title,
          linkedin_username, memo, organization_id, metadata_json, created_at, updated_at, deleted_at
        ) VALUES (
          ?, {WORKSPACE_BINDING_SQL},
          COALESCE((SELECT library_workspace_id FROM local_library_connections WHERE active = 1), ?),
          ?, ?, ?, ?, ?, ?, ?, {insert_metadata}, ?, ?, NULL
        )
        ON CONFLICT(id) DO UPDATE SET
          name = excluded.name, email = excluded.email, phone = excluded.phone,
          job_title = excluded.job_title, linkedin_username = excluded.linkedin_username,
          memo = excluded.memo, organization_id = excluded.organization_id,
          metadata_json = {update_metadata},
          updated_at = excluded.updated_at, deleted_at = NULL"
    );
    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
    query = query
        .bind(human_id)
        .bind(human_id)
        .bind(name)
        .bind(email)
        .bind(phone)
        .bind(job_title)
        .bind(linkedin_username)
        .bind(memo)
        .bind(organization_id);
    if let AvatarChange::Set(value) = avatar {
        query = query.bind(value);
    }
    query = query.bind(now).bind(now);
    if let AvatarChange::Set(value) = avatar {
        query = query.bind(value);
    }
    let result = query.execute(&mut *conn).await?;
    Ok(result.rows_affected())
}

pub async fn update_contact_fields(
    conn: &mut SqliteConnection,
    table: &'static str,
    assignments: &[(&'static str, String)],
    now: &str,
    contact_id: &str,
) -> Result<u64, sqlx::Error> {
    let mut sql = String::from("UPDATE ");
    sql.push_str(table);
    sql.push_str(" SET ");
    for (index, (assignment, _)) in assignments.iter().enumerate() {
        if index > 0 {
            sql.push_str(", ");
        }
        sql.push_str(assignment);
    }
    sql.push_str(", updated_at = ? WHERE id = ? AND deleted_at IS NULL");
    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
    for (_, value) in assignments {
        query = query.bind(value.as_str());
    }
    let result = query.bind(now).bind(contact_id).execute(&mut *conn).await?;
    Ok(result.rows_affected())
}

pub async fn soft_delete_contact(
    conn: &mut SqliteConnection,
    table: &'static str,
    contact_id: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let sql = format!(
        "UPDATE {table}
          SET deleted_at = ?, updated_at = ?
          WHERE id = ? AND deleted_at IS NULL"
    );
    let result = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(now)
        .bind(now)
        .bind(contact_id)
        .execute(&mut *conn)
        .await?;
    Ok(result.rows_affected())
}

pub async fn update_contact_avatar(
    conn: &mut SqliteConnection,
    table: &'static str,
    avatar_data_url: Option<&str>,
    contact_id: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let set = match avatar_data_url {
        None => format!("json_remove({VALID_METADATA_SQL}, '$.avatarDataUrl')"),
        Some(_) => format!("json_set({VALID_METADATA_SQL}, '$.avatarDataUrl', ?)"),
    };
    let sql = format!(
        "UPDATE {table}
          SET
            metadata_json = {set},
            updated_at = ?
          WHERE id = ? AND deleted_at IS NULL"
    );
    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
    if let Some(value) = avatar_data_url {
        query = query.bind(value);
    }
    query = query.bind(now).bind(contact_id);
    let result = query.execute(&mut *conn).await?;
    Ok(result.rows_affected())
}

pub async fn update_human_contact_summary(
    conn: &mut SqliteConnection,
    summary_json: &str,
    now: &str,
    human_id: &str,
) -> Result<u64, sqlx::Error> {
    let sql = format!(
        "UPDATE humans
          SET
            metadata_json = json_set(
              {VALID_METADATA_SQL},
              '$.contactSummary',
              json(?)
            ),
            updated_at = ?
          WHERE id = ? AND deleted_at IS NULL"
    );
    let result = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(summary_json)
        .bind(now)
        .bind(human_id)
        .execute(&mut *conn)
        .await?;
    Ok(result.rows_affected())
}

pub async fn toggle_contact_pin(
    conn: &mut SqliteConnection,
    table: &'static str,
    contact_id: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let sql = format!(
        "UPDATE {table}
          SET
            pin_order = CASE
              WHEN pinned = 1 THEN NULL
              ELSE COALESCE((
                SELECT MAX(pin_order)
                FROM (
                  SELECT pin_order FROM humans WHERE deleted_at IS NULL
                  UNION ALL
                  SELECT pin_order FROM organizations WHERE deleted_at IS NULL
                )
              ), 0) + 1
            END,
            pinned = CASE WHEN pinned = 1 THEN 0 ELSE 1 END,
            updated_at = ?
          WHERE id = ? AND deleted_at IS NULL"
    );
    let result = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(now)
        .bind(contact_id)
        .execute(&mut *conn)
        .await?;
    Ok(result.rows_affected())
}

pub async fn set_pinned_order(
    conn: &mut SqliteConnection,
    table: &'static str,
    pin_order: i64,
    contact_id: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let sql = format!(
        "UPDATE {table}
          SET pin_order = ?, updated_at = ?
          WHERE id = ? AND pinned = 1 AND deleted_at IS NULL"
    );
    let result = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(pin_order)
        .bind(now)
        .bind(contact_id)
        .execute(&mut *conn)
        .await?;
    Ok(result.rows_affected())
}

pub async fn list_merge_humans(
    conn: &mut SqliteConnection,
    selected_human_id: &str,
    duplicate_human_id: &str,
) -> Result<Vec<MergeHumanRow>, sqlx::Error> {
    sqlx::query_as::<_, MergeHumanRow>(
        "SELECT
          id, owner_user_id, created_at, organization_id, name, email, phone,
          job_title, linkedin_username, memo, pinned, pin_order, metadata_json
        FROM humans
        WHERE id IN (?, ?) AND deleted_at IS NULL",
    )
    .bind(selected_human_id)
    .bind(duplicate_human_id)
    .fetch_all(&mut *conn)
    .await
}

pub async fn tombstone_duplicate_participant_mappings(
    conn: &mut SqliteConnection,
    now: &str,
    duplicate_id: &str,
    primary_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE session_participants AS duplicate_mapping
          SET deleted_at = ?, updated_at = ?
          WHERE duplicate_mapping.human_id = ?
            AND duplicate_mapping.deleted_at IS NULL
            AND EXISTS (
              SELECT 1
              FROM session_participants AS primary_mapping
              WHERE primary_mapping.session_id = duplicate_mapping.session_id
                AND primary_mapping.human_id = ?
                AND primary_mapping.deleted_at IS NULL
            )",
    )
    .bind(now)
    .bind(now)
    .bind(duplicate_id)
    .bind(primary_id)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn reassign_participant_mappings(
    conn: &mut SqliteConnection,
    primary_id: &str,
    duplicate_id: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE session_participants
          SET human_id = ?, updated_at = ?
          WHERE human_id = ? AND deleted_at IS NULL",
    )
    .bind(primary_id)
    .bind(now)
    .bind(duplicate_id)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

#[allow(clippy::too_many_arguments)]
pub async fn update_merged_human(
    conn: &mut SqliteConnection,
    job_title: &str,
    linkedin_username: &str,
    phone: &str,
    memo: &str,
    organization_id: &str,
    email: &str,
    additional_emails_json: Option<&str>,
    now: &str,
    primary_id: &str,
) -> Result<u64, sqlx::Error> {
    let metadata = match additional_emails_json {
        None => format!("json_remove({VALID_METADATA_SQL}, '$.additionalEmails')"),
        Some(_) => {
            format!("json_set({VALID_METADATA_SQL}, '$.additionalEmails', json(?))")
        }
    };
    let sql = format!(
        "UPDATE humans
          SET
            job_title = ?,
            linkedin_username = ?,
            phone = ?,
            memo = ?,
            organization_id = ?,
            email = ?,
            metadata_json = {metadata},
            updated_at = ?
          WHERE id = ? AND deleted_at IS NULL"
    );
    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(job_title)
        .bind(linkedin_username)
        .bind(phone)
        .bind(memo)
        .bind(organization_id)
        .bind(email);
    if let Some(json) = additional_emails_json {
        query = query.bind(json);
    }
    let result = query.bind(now).bind(primary_id).execute(&mut *conn).await?;
    Ok(result.rows_affected())
}

/// `additional_emails_json`: `Some` sets `$.additionalEmails` to that JSON
/// array, `None` removes the key. Other metadata keys are preserved.
pub async fn update_human_additional_emails(
    conn: &mut SqliteConnection,
    additional_emails_json: Option<&str>,
    now: &str,
    human_id: &str,
) -> Result<u64, sqlx::Error> {
    let set = match additional_emails_json {
        None => format!("json_remove({VALID_METADATA_SQL}, '$.additionalEmails')"),
        Some(_) => {
            format!("json_set({VALID_METADATA_SQL}, '$.additionalEmails', json(?))")
        }
    };
    let sql = format!(
        "UPDATE humans
          SET
            metadata_json = {set},
            updated_at = ?
          WHERE id = ? AND deleted_at IS NULL"
    );
    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
    if let Some(json) = additional_emails_json {
        query = query.bind(json);
    }
    let result = query.bind(now).bind(human_id).execute(&mut *conn).await?;
    Ok(result.rows_affected())
}

pub async fn upsert_revived_human(
    conn: &mut SqliteConnection,
    id: &str,
    owner_user_id: &str,
    name: &str,
    email: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(concat!(
        "INSERT INTO humans (
            id, workspace_id, owner_user_id, organization_id, name, email,
            phone, job_title, linkedin_username, memo, pinned, pin_order,
            metadata_json, created_at, updated_at, deleted_at
          ) VALUES (
            ?, NULLIF((
              SELECT json_extract(value_json, '$.workspace_id')
              FROM app_settings
              WHERE id = 'cloudsync_workspace_binding'
            ), ''), COALESCE(
              (SELECT library_workspace_id FROM local_library_connections WHERE active = 1),
              NULLIF(NULLIF(?, ''), '",
        "00000000-0000-0000-0000-000000000000",
        "'),
              NULLIF((
                SELECT json_extract(value_json, '$.workspace_id')
                FROM app_settings
                WHERE id = 'cloudsync_workspace_binding'
              ), ''),
              '",
        "00000000-0000-0000-0000-000000000000",
        "'
            ), '', ?, ?, '', '', '', '', 0, NULL, '{}', ?, ?, NULL
          )
          ON CONFLICT(id) DO UPDATE SET
            deleted_at = NULL,
            updated_at = excluded.updated_at
          WHERE humans.deleted_at IS NOT NULL"
    ))
    .bind(id)
    .bind(owner_user_id)
    .bind(name)
    .bind(email)
    .bind(now)
    .bind(now)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn insert_organization_by_name_if_missing(
    conn: &mut SqliteConnection,
    id: &str,
    owner_user_id: &str,
    name: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let sql = format!(
        "INSERT INTO organizations (
            id, workspace_id, owner_user_id, name, memo, pinned, pin_order,
            metadata_json, created_at, updated_at, deleted_at
          )
          SELECT ?, {WORKSPACE_BINDING_SQL}, COALESCE((SELECT library_workspace_id FROM local_library_connections WHERE active = 1), ?), ?, '', 0, NULL, '{{}}', ?, ?, NULL
          WHERE NOT EXISTS (
            SELECT 1
            FROM organizations
            WHERE lower(name) = lower(?) AND deleted_at IS NULL
          )"
    );
    let result = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(owner_user_id)
        .bind(name)
        .bind(now)
        .bind(now)
        .bind(name)
        .execute(&mut *conn)
        .await?;
    Ok(result.rows_affected())
}
