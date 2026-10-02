use sqlx::SqliteConnection;

pub const HUMAN_NAME_IS_PLACEHOLDER_SQL: &str =
    "(trim(name) = '' OR (name LIKE '%@%.%' AND instr(trim(name), ' ') = 0))";

pub async fn revive_excluded_session_participant(
    conn: &mut SqliteConnection,
    session_id: &str,
    human_id: &str,
    source: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE session_participants
        SET source = ?, updated_at = ?
        WHERE id = (
          SELECT id
          FROM session_participants
          WHERE session_id = ?
            AND human_id = ?
            AND source = 'excluded'
            AND deleted_at IS NULL
            AND ? <> 'auto'
          ORDER BY created_at, id
          LIMIT 1
        )",
    )
    .bind(source)
    .bind(now)
    .bind(session_id)
    .bind(human_id)
    .bind(source)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}

pub async fn insert_manual_session_participant(
    conn: &mut SqliteConnection,
    participant_id: &str,
    session_id: &str,
    human_id: &str,
    source: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO session_participants (
          id, workspace_id, owner_user_id, session_id, human_id,
          display_name, email, role, source, metadata_json, created_at,
          updated_at, deleted_at
        )
        SELECT ?, session.workspace_id, session.owner_user_id, session.id, human.id,
          human.name, human.email, '', ?, '{}', ?, ?, NULL
        FROM sessions AS session
        JOIN humans AS human ON human.id = ? AND human.deleted_at IS NULL
        WHERE session.id = ?
          AND session.deleted_at IS NULL
          AND NOT EXISTS (
            SELECT 1
            FROM session_participants AS existing
            WHERE existing.session_id = session.id
              AND existing.human_id = human.id
              AND existing.deleted_at IS NULL
          )",
    )
    .bind(participant_id)
    .bind(source)
    .bind(now)
    .bind(now)
    .bind(human_id)
    .bind(session_id)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}

pub async fn remove_session_participant_mapping(
    conn: &mut SqliteConnection,
    mapping_id: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE session_participants
        SET
          source = CASE WHEN source = 'auto' THEN 'excluded' ELSE source END,
          deleted_at = CASE WHEN source = 'auto' THEN NULL ELSE ? END,
          updated_at = ?
        WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(now)
    .bind(now)
    .bind(mapping_id)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}

pub struct HumanEmailRow {
    pub id: String,
    pub email: String,
}

pub async fn find_human_ids_by_email(
    conn: &mut SqliteConnection,
    emails: &[String],
) -> Result<Vec<HumanEmailRow>, sqlx::Error> {
    if emails.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = emails.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT id, email
        FROM humans
        WHERE deleted_at IS NULL
          AND lower(email) IN ({placeholders})
        ORDER BY id"
    );
    let mut query = sqlx::query_as::<_, (String, String)>(sqlx::AssertSqlSafe(sql));
    for email in emails {
        query = query.bind(email);
    }
    let rows = query.fetch_all(&mut *conn).await?;

    Ok(rows
        .into_iter()
        .map(|(id, email)| HumanEmailRow { id, email })
        .collect())
}

pub struct EventParticipantInsert<'a> {
    pub organization_id: &'a str,
    pub company_name: &'a str,
    pub now: &'a str,
    pub session_id: &'a str,
    pub human_id: &'a str,
    pub email: &'a str,
}

pub async fn insert_new_participant_organization(
    conn: &mut SqliteConnection,
    input: &EventParticipantInsert<'_>,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO organizations (
          id, workspace_id, owner_user_id, name, memo, pinned, pin_order,
          metadata_json, created_at, updated_at, deleted_at
        )
        SELECT ?, session.workspace_id, session.owner_user_id, ?, '', 0, NULL,
          '{}', ?, ?, NULL
        FROM sessions AS session
        WHERE session.id = ? AND session.deleted_at IS NULL
          AND ? <> session.owner_user_id
          AND NOT EXISTS (
            SELECT 1
            FROM organizations
            WHERE lower(name) = lower(?) AND deleted_at IS NULL
          )
          AND NOT EXISTS (
            SELECT 1
            FROM humans
            WHERE lower(email) = lower(?) AND deleted_at IS NULL
          )",
    )
    .bind(input.organization_id)
    .bind(input.company_name)
    .bind(input.now)
    .bind(input.now)
    .bind(input.session_id)
    .bind(input.human_id)
    .bind(input.company_name)
    .bind(input.email)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}

pub async fn insert_existing_participant_organization(
    conn: &mut SqliteConnection,
    input: &EventParticipantInsert<'_>,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO organizations (
          id, workspace_id, owner_user_id, name, memo, pinned, pin_order,
          metadata_json, created_at, updated_at, deleted_at
        )
        SELECT ?, session.workspace_id, session.owner_user_id, ?, '', 0, NULL,
          '{}', ?, ?, NULL
        FROM sessions AS session
        WHERE session.id = ? AND session.deleted_at IS NULL
          AND ? <> session.owner_user_id
          AND EXISTS (
            SELECT 1
            FROM humans
            WHERE id = ? AND organization_id = '' AND deleted_at IS NULL
          )
          AND NOT EXISTS (
            SELECT 1
            FROM organizations
            WHERE lower(name) = lower(?) AND deleted_at IS NULL
          )",
    )
    .bind(input.organization_id)
    .bind(input.company_name)
    .bind(input.now)
    .bind(input.now)
    .bind(input.session_id)
    .bind(input.human_id)
    .bind(input.human_id)
    .bind(input.company_name)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}

pub struct EventHumanInsert<'a> {
    pub human_id: &'a str,
    pub name: &'a str,
    pub email: &'a str,
    pub company_name: &'a str,
    pub now: &'a str,
    pub session_id: &'a str,
}

pub async fn insert_event_human(
    conn: &mut SqliteConnection,
    input: &EventHumanInsert<'_>,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO humans (
          id, workspace_id, owner_user_id, name, email, organization_id,
          created_at, updated_at, deleted_at
        )
        SELECT ?, session.workspace_id, session.owner_user_id, ?, ?,
          COALESCE((
            SELECT id
            FROM organizations
            WHERE deleted_at IS NULL AND ? <> '' AND lower(name) = lower(?)
            ORDER BY created_at, id
            LIMIT 1
          ), ''),
          ?, ?, NULL
        FROM sessions AS session
        WHERE session.id = ? AND session.deleted_at IS NULL
          AND ? <> session.owner_user_id
          AND NOT EXISTS (
            SELECT 1
            FROM humans
            WHERE lower(email) = lower(?) AND deleted_at IS NULL
          )",
    )
    .bind(input.human_id)
    .bind(input.name)
    .bind(input.email)
    .bind(input.company_name)
    .bind(input.company_name)
    .bind(input.now)
    .bind(input.now)
    .bind(input.session_id)
    .bind(input.human_id)
    .bind(input.email)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}

pub async fn update_event_human(
    conn: &mut SqliteConnection,
    input: &EventHumanInsert<'_>,
) -> Result<u64, sqlx::Error> {
    let sql = format!(
        "UPDATE humans
        SET
          name = CASE
            WHEN {HUMAN_NAME_IS_PLACEHOLDER_SQL} THEN ? ELSE name
          END,
          organization_id = CASE
            WHEN organization_id = '' THEN COALESCE((
              SELECT id
              FROM organizations
              WHERE deleted_at IS NULL AND ? <> '' AND lower(name) = lower(?)
              ORDER BY created_at, id
              LIMIT 1
            ), '')
            ELSE organization_id
          END,
          updated_at = ?
        WHERE id = ? AND deleted_at IS NULL
          AND (
            {HUMAN_NAME_IS_PLACEHOLDER_SQL}
            OR (organization_id = '' AND ? <> '')
          )
          AND EXISTS (
            SELECT 1
            FROM sessions AS session
            WHERE session.id = ? AND session.deleted_at IS NULL
              AND ? <> session.owner_user_id
          )"
    );
    let result = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(input.name)
        .bind(input.company_name)
        .bind(input.company_name)
        .bind(input.now)
        .bind(input.human_id)
        .bind(input.company_name)
        .bind(input.session_id)
        .bind(input.human_id)
        .execute(&mut *conn)
        .await?;

    Ok(result.rows_affected())
}

pub async fn insert_event_session_participant(
    conn: &mut SqliteConnection,
    participant_id: &str,
    input: &EventHumanInsert<'_>,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO session_participants (
          id, workspace_id, owner_user_id, session_id, human_id, display_name,
          email, source, created_at, updated_at, deleted_at
        )
        SELECT ?, session.workspace_id, session.owner_user_id, session.id,
          ?, ?, ?, 'auto', ?, ?, NULL
        FROM sessions AS session
        WHERE session.id = ? AND session.deleted_at IS NULL
          AND ? <> session.owner_user_id
          AND NOT EXISTS (
            SELECT 1
            FROM humans AS owner
            WHERE owner.id = session.owner_user_id
              AND owner.deleted_at IS NULL
              AND NULLIF(lower(owner.email), '') IS NOT NULL
              AND lower(owner.email) = lower(?)
          )
          AND NOT EXISTS (
            SELECT 1
            FROM session_participants AS existing
            WHERE existing.session_id = session.id
              AND existing.deleted_at IS NULL
              AND (
                existing.human_id = ?
                OR (
                  existing.human_id = session.owner_user_id
                  AND NULLIF(lower(existing.email), '') IS NOT NULL
                  AND lower(existing.email) = lower(?)
                )
                OR (
                  NULLIF(lower(existing.email), '') IS NOT NULL
                  AND lower(existing.email) = lower(?)
                )
              )
          )",
    )
    .bind(participant_id)
    .bind(input.human_id)
    .bind(input.name)
    .bind(input.email)
    .bind(input.now)
    .bind(input.now)
    .bind(input.session_id)
    .bind(input.human_id)
    .bind(input.email)
    .bind(input.human_id)
    .bind(input.email)
    .bind(input.email)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}
