use sqlx::SqliteConnection;

#[derive(sqlx::FromRow)]
pub struct EventSessionRow {
    pub id: String,
    pub tracking_id_event: String,
    pub calendar_id: String,
    pub title: String,
    pub started_at: String,
    pub ended_at: String,
    pub location: Option<String>,
    pub meeting_link: Option<String>,
    pub description: Option<String>,
    pub recurrence_series_id: Option<String>,
    pub has_recurrence_rules: i64,
    pub is_all_day: i64,
    pub provider: String,
    pub participants_json: Option<String>,
}

pub async fn insert_session(
    conn: &mut SqliteConnection,
    session_id: &str,
    title: &str,
    user_id: &str,
    event_json: &str,
    folder_path: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO sessions (
          id, workspace_id, owner_user_id, title, event_json, folder_path,
          created_at, updated_at, deleted_at
        ) VALUES (
          ?, COALESCE(
            (SELECT NULLIF(folder.workspace_id, '') FROM folders AS folder
              WHERE folder.deleted_at IS NULL AND folder.workspace_id <> ''
                AND folder.path = ?
              LIMIT 1),
            NULLIF((
              SELECT json_extract(value_json, '$.workspace_id')
              FROM app_settings
              WHERE id = 'cloudsync_workspace_binding'
            ), '')
          ), COALESCE(
            (SELECT library_workspace_id FROM local_library_connections WHERE active = 1),
            NULLIF(NULLIF(?, ''), '00000000-0000-0000-0000-000000000000'),
            NULLIF((
              SELECT json_extract(value_json, '$.workspace_id')
              FROM app_settings
              WHERE id = 'cloudsync_workspace_binding'
            ), '')
          ), ?, ?, ?, ?, ?, NULL
        )",
    )
    .bind(session_id)
    .bind(folder_path)
    .bind(user_id)
    .bind(title)
    .bind(event_json)
    .bind(folder_path)
    .bind(now)
    .bind(now)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}

pub async fn insert_empty_session_note(
    conn: &mut SqliteConnection,
    session_id: &str,
    body: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO session_documents (
          id, workspace_id, session_id, kind, body_format, body, created_by,
          updated_by, created_at, updated_at, deleted_at
        )
        SELECT ?, workspace_id, id, 'note', 'prosemirror_json', ?,
          owner_user_id, owner_user_id, ?, ?, NULL
        FROM sessions
        WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(session_id)
    .bind(body)
    .bind(now)
    .bind(now)
    .bind(session_id)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}

pub async fn upsert_session_owner_human(
    conn: &mut SqliteConnection,
    session_id: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO humans (
          id, workspace_id, owner_user_id, updated_at, deleted_at
        )
        SELECT session.owner_user_id, session.workspace_id,
          session.owner_user_id, ?, NULL
        FROM sessions AS session
        WHERE session.id = ? AND session.deleted_at IS NULL
        ON CONFLICT(id) DO UPDATE SET
          deleted_at = NULL,
          updated_at = excluded.updated_at",
    )
    .bind(now)
    .bind(session_id)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}

pub async fn insert_owner_session_participant(
    conn: &mut SqliteConnection,
    participant_id: &str,
    session_id: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO session_participants (
          id, workspace_id, owner_user_id, session_id, human_id, source,
          created_at, updated_at, deleted_at
        )
        SELECT ?, session.workspace_id, session.owner_user_id, session.id,
          session.owner_user_id, 'manual', ?, ?, NULL
        FROM sessions AS session
        WHERE session.id = ? AND session.deleted_at IS NULL",
    )
    .bind(participant_id)
    .bind(now)
    .bind(now)
    .bind(session_id)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}

pub async fn load_event_session_row(
    conn: &mut SqliteConnection,
    event_id: &str,
) -> Result<Option<EventSessionRow>, sqlx::Error> {
    sqlx::query_as::<_, EventSessionRow>(
        "SELECT
          id,
          tracking_id_event,
          calendar_id,
          title,
          started_at,
          ended_at,
          location,
          meeting_link,
          description,
          recurrence_series_id,
          has_recurrence_rules,
          is_all_day,
          provider,
          participants_json
        FROM events
        WHERE id = ? AND deleted_at IS NULL
        LIMIT 1",
    )
    .bind(event_id)
    .fetch_optional(&mut *conn)
    .await
}

pub async fn find_session_id_for_event(
    conn: &mut SqliteConnection,
    event_id: &str,
    tracking_id_event: &str,
    preferred_id: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        "SELECT id
        FROM sessions
        WHERE deleted_at IS NULL
          AND (event_id = ? OR (? <> '' AND external_event_id = ?))
        ORDER BY CASE WHEN id = ? THEN 0 ELSE 1 END, created_at, id
        LIMIT 1",
    )
    .bind(event_id)
    .bind(tracking_id_event)
    .bind(tracking_id_event)
    .bind(preferred_id)
    .fetch_optional(&mut *conn)
    .await
}

pub async fn relink_session_to_event(
    conn: &mut SqliteConnection,
    session_id: &str,
    event_id: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE sessions
        SET event_id = ?, updated_at = ?
        WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(event_id)
    .bind(now)
    .bind(session_id)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn insert_event_session(
    conn: &mut SqliteConnection,
    session_id: &str,
    user_id: &str,
    title: &str,
    now: &str,
    event: &EventSessionRow,
    event_json: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO sessions (
          id, workspace_id, owner_user_id, title, created_at, updated_at,
          started_at, ended_at, event_id, external_event_id, external_provider,
          series_id, event_json, deleted_at
        )
        SELECT ?, NULLIF((
          SELECT json_extract(value_json, '$.workspace_id')
          FROM app_settings
          WHERE id = 'cloudsync_workspace_binding'
        ), ''), COALESCE(
          (SELECT library_workspace_id FROM local_library_connections WHERE active = 1),
          NULLIF(NULLIF(?, ''), '00000000-0000-0000-0000-000000000000'),
          NULLIF((
            SELECT json_extract(value_json, '$.workspace_id')
            FROM app_settings
            WHERE id = 'cloudsync_workspace_binding'
          ), '')
        ), ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL
        WHERE NOT EXISTS (
          SELECT 1
          FROM sessions
          WHERE deleted_at IS NULL
            AND (event_id = ? OR (? <> '' AND external_event_id = ?))
        )",
    )
    .bind(session_id)
    .bind(user_id)
    .bind(title)
    .bind(now)
    .bind(now)
    .bind(&event.started_at)
    .bind(&event.ended_at)
    .bind(&event.id)
    .bind(&event.tracking_id_event)
    .bind(&event.provider)
    .bind(&event.recurrence_series_id)
    .bind(event_json)
    .bind(&event.id)
    .bind(&event.tracking_id_event)
    .bind(&event.tracking_id_event)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected())
}

pub struct SessionDeleteRow {
    pub id: String,
    pub title: String,
}

pub async fn load_session_delete_row(
    conn: &mut SqliteConnection,
    session_id: &str,
) -> Result<Option<SessionDeleteRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT id, title FROM sessions WHERE id = ? AND deleted_at IS NULL LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(&mut *conn)
    .await?;

    Ok(row.map(|(id, title)| SessionDeleteRow { id, title }))
}

pub async fn session_is_alive(
    conn: &mut SqliteConnection,
    session_id: &str,
) -> Result<bool, sqlx::Error> {
    let id = sqlx::query_scalar::<_, String>(
        "SELECT id FROM sessions WHERE id = ? AND deleted_at IS NULL LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(&mut *conn)
    .await?;

    Ok(id.is_some())
}

pub async fn update_session_child_tombstone(
    conn: &mut SqliteConnection,
    table: &str,
    session_id: &str,
    tombstone: &str,
    restore: bool,
) -> Result<u64, sqlx::Error> {
    let predicate = if restore {
        "deleted_at = ?"
    } else {
        "deleted_at IS NULL"
    };
    let sql = format!(
        "UPDATE {table}
        SET deleted_at = ?, updated_at = ?
        WHERE session_id = ? AND {predicate}"
    );
    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
    if restore {
        query = query.bind(Option::<&str>::None);
    } else {
        query = query.bind(tombstone);
    }
    let mut query = query.bind(tombstone).bind(session_id);
    if restore {
        query = query.bind(tombstone);
    }
    let result = query.execute(&mut *conn).await?;

    Ok(result.rows_affected())
}

pub async fn update_entity_mentions_tombstone(
    conn: &mut SqliteConnection,
    session_id: &str,
    tombstone: &str,
    restore: bool,
) -> Result<u64, sqlx::Error> {
    let predicate = if restore {
        "deleted_at = ?"
    } else {
        "deleted_at IS NULL"
    };
    let sql = format!(
        "UPDATE entity_mentions
        SET deleted_at = ?, updated_at = ?
        WHERE (
          (source_type = 'session' AND source_id = ?)
          OR (target_type = 'session' AND target_id = ?)
        ) AND {predicate}"
    );
    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
    if restore {
        query = query.bind(Option::<&str>::None);
    } else {
        query = query.bind(tombstone);
    }
    let mut query = query.bind(tombstone).bind(session_id).bind(session_id);
    if restore {
        query = query.bind(tombstone);
    }
    let result = query.execute(&mut *conn).await?;

    Ok(result.rows_affected())
}

pub async fn update_sessions_tombstone(
    conn: &mut SqliteConnection,
    session_id: &str,
    tombstone: &str,
    restore: bool,
) -> Result<u64, sqlx::Error> {
    let predicate = if restore {
        "deleted_at = ?"
    } else {
        "deleted_at IS NULL"
    };
    let sql = format!(
        "UPDATE sessions
        SET deleted_at = ?, updated_at = ?
        WHERE id = ? AND {predicate}"
    );
    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
    if restore {
        query = query.bind(Option::<&str>::None);
    } else {
        query = query.bind(tombstone);
    }
    let mut query = query.bind(tombstone).bind(session_id);
    if restore {
        query = query.bind(tombstone);
    }
    let result = query.execute(&mut *conn).await?;

    Ok(result.rows_affected())
}
