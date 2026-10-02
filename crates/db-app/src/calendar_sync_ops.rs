use sqlx::{QueryBuilder, Sqlite, SqliteConnection};

use crate::session_participant_ops::HUMAN_NAME_IS_PLACEHOLDER_SQL;

pub const DEFAULT_USER_ID: &str = "00000000-0000-0000-0000-000000000000";

pub const WORKSPACE_ID_SQL: &str = "NULLIF((
  SELECT json_extract(value_json, '$.workspace_id')
  FROM app_settings
  WHERE id = 'cloudsync_workspace_binding'
), '')";

pub const ORGANIZATION_ID_BY_NAME_SQL: &str = "IFNULL((
  SELECT id
  FROM organizations
  WHERE deleted_at IS NULL AND ? <> '' AND lower(name) = lower(?)
  ORDER BY created_at, id
  LIMIT 1
), '')";

pub const OWNER_USER_ID_SQL: &str = "COALESCE(\n    NULLIF(NULLIF(?, ''), '00000000-0000-0000-0000-000000000000'),\n    NULLIF((\n  SELECT json_extract(value_json, '$.workspace_id')\n  FROM app_settings\n  WHERE id = 'cloudsync_workspace_binding'\n), ''),\n    '00000000-0000-0000-0000-000000000000'\n  )";

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct CalendarSyncRow {
    pub id: String,
    pub tracking_id_calendar: String,
    pub name: String,
    pub enabled: bool,
    pub provider: String,
    pub source: String,
    pub color: String,
    pub connection_id: String,
    pub created_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct EventSyncRow {
    pub id: String,
    pub tracking_id_event: String,
    pub calendar_id: String,
    pub title: String,
    pub started_at: String,
    pub ended_at: String,
    pub location: String,
    pub meeting_link: String,
    pub description: String,
    pub note: String,
    pub recurrence_series_id: String,
    pub has_recurrence_rules: bool,
    pub is_all_day: bool,
    pub provider: String,
    pub created_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SessionSyncRow {
    pub id: String,
    pub owner_user_id: String,
    pub event_json: String,
    pub tracking_id: String,
    pub calendar_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct ParticipantHumanRow {
    pub id: String,
    pub email: String,
    pub name: String,
    pub organization_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct ParticipantMappingRow {
    pub id: String,
    pub session_id: String,
    pub human_id: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct AppSettingRow {
    pub id: String,
    pub value_json: String,
}

const CALENDAR_COLUMNS: &str = "
        id,
        tracking_id_calendar,
        name,
        enabled,
        provider,
        source,
        color,
        connection_id,
        created_at,
        deleted_at";

pub async fn list_enabled_calendars(
    conn: &mut SqliteConnection,
    provider: &str,
    connection_id: &str,
) -> Result<Vec<CalendarSyncRow>, sqlx::Error> {
    let sql = format!(
        "SELECT{CALENDAR_COLUMNS}
      FROM calendars
      WHERE provider = ?
        AND connection_id = ?
        AND enabled = 1
        AND deleted_at IS NULL
      ORDER BY created_at, id"
    );
    sqlx::query_as::<_, CalendarSyncRow>(sqlx::AssertSqlSafe(sql))
        .bind(provider)
        .bind(connection_id)
        .fetch_all(&mut *conn)
        .await
}

pub async fn list_provider_calendars(
    conn: &mut SqliteConnection,
    provider: &str,
) -> Result<Vec<CalendarSyncRow>, sqlx::Error> {
    let sql = format!(
        "SELECT{CALENDAR_COLUMNS}
      FROM calendars
      WHERE provider = ?
      ORDER BY created_at, id"
    );
    sqlx::query_as::<_, CalendarSyncRow>(sqlx::AssertSqlSafe(sql))
        .bind(provider)
        .fetch_all(&mut *conn)
        .await
}

pub async fn soft_delete_calendar(
    conn: &mut SqliteConnection,
    now: &str,
    calendar_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE calendars
          SET deleted_at = ?, updated_at = ?
          WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(now)
    .bind(now)
    .bind(calendar_id)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn tombstone_events_for_calendars(
    conn: &mut SqliteConnection,
    now: &str,
    calendar_ids: &[String],
) -> Result<u64, sqlx::Error> {
    let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new(
        "UPDATE events
        SET deleted_at = ",
    );
    builder.push_bind(now);
    builder.push(", updated_at = ");
    builder.push_bind(now);
    builder.push(
        "
        WHERE deleted_at IS NULL
          AND calendar_id IN (",
    );
    let mut separated = builder.separated(", ");
    for calendar_id in calendar_ids {
        separated.push_bind(calendar_id);
    }
    builder.push(")");
    let result = builder.build().execute(&mut *conn).await?;
    Ok(result.rows_affected())
}

#[allow(clippy::too_many_arguments)]
pub async fn upsert_inventory_calendar(
    conn: &mut SqliteConnection,
    id: &str,
    tracking_id_calendar: &str,
    name: &str,
    provider: &str,
    source: &str,
    color: &str,
    connection_id: &str,
    created_at: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO calendars (
            id,
            tracking_id_calendar,
            name,
            enabled,
            provider,
            source,
            color,
            connection_id,
            created_at,
            updated_at,
            deleted_at
          )
          VALUES (?, ?, ?, 0, ?, ?, ?, ?, ?, ?, NULL)
          ON CONFLICT(id) DO UPDATE SET
            tracking_id_calendar = excluded.tracking_id_calendar,
            name = excluded.name,
            enabled = CASE
              WHEN calendars.deleted_at IS NULL THEN calendars.enabled
              ELSE 0
            END,
            provider = excluded.provider,
            source = excluded.source,
            color = excluded.color,
            connection_id = excluded.connection_id,
            updated_at = excluded.updated_at,
            deleted_at = NULL",
    )
    .bind(id)
    .bind(tracking_id_calendar)
    .bind(name)
    .bind(provider)
    .bind(source)
    .bind(color)
    .bind(connection_id)
    .bind(created_at)
    .bind(now)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn tombstone_connection_events(
    conn: &mut SqliteConnection,
    now: &str,
    provider: &str,
    connection_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE events
        SET deleted_at = ?, updated_at = ?
        WHERE deleted_at IS NULL
          AND calendar_id IN (
            SELECT id
            FROM calendars
            WHERE provider = ?
              AND connection_id = ?
              AND deleted_at IS NULL
          )",
    )
    .bind(now)
    .bind(now)
    .bind(provider)
    .bind(connection_id)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn tombstone_connection_calendars(
    conn: &mut SqliteConnection,
    now: &str,
    provider: &str,
    connection_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE calendars
        SET deleted_at = ?, updated_at = ?
        WHERE provider = ?
          AND connection_id = ?
          AND deleted_at IS NULL",
    )
    .bind(now)
    .bind(now)
    .bind(provider)
    .bind(connection_id)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn update_calendar_enabled(
    conn: &mut SqliteConnection,
    enabled: bool,
    now: &str,
    calendar_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE calendars
          SET enabled = ?, updated_at = ?
          WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(enabled)
    .bind(now)
    .bind(calendar_id)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn tombstone_events_when_calendar_disabled(
    conn: &mut SqliteConnection,
    now: &str,
    calendar_id: &str,
    enabled: bool,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE events
          SET deleted_at = ?, updated_at = ?
          WHERE calendar_id = ? AND deleted_at IS NULL AND ? = 0",
    )
    .bind(now)
    .bind(now)
    .bind(calendar_id)
    .bind(enabled)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn list_events_for_sync(
    conn: &mut SqliteConnection,
    calendar_ids: &[String],
    to: &str,
    from: &str,
    tracking_ids: &[String],
) -> Result<Vec<EventSyncRow>, sqlx::Error> {
    if calendar_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new(
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
        note,
        recurrence_series_id,
        has_recurrence_rules,
        is_all_day,
        provider,
        created_at,
        deleted_at
      FROM events
      WHERE calendar_id IN (",
    );
    {
        let mut separated = builder.separated(", ");
        for calendar_id in calendar_ids {
            separated.push_bind(calendar_id);
        }
    }
    builder.push(
        ")
        AND (
          (
            deleted_at IS NULL
            AND julianday(started_at) <= julianday(",
    );
    builder.push_bind(to);
    builder.push(
        ")
            AND julianday(CASE WHEN ended_at = '' THEN started_at ELSE ended_at END)
              >= julianday(",
    );
    builder.push_bind(from);
    builder.push(")\n          )");
    if !tracking_ids.is_empty() {
        builder.push("OR tracking_id_event IN (");
        let mut separated = builder.separated(", ");
        for tracking_id in tracking_ids {
            separated.push_bind(tracking_id);
        }
        builder.push(")");
    }
    builder.push(
        "
        )
      ORDER BY
        EXISTS (SELECT 1 FROM sessions WHERE sessions.event_id = events.id AND sessions.deleted_at IS NULL) DESC,
        deleted_at IS NOT NULL, created_at, id",
    );
    builder
        .build_query_as::<EventSyncRow>()
        .fetch_all(&mut *conn)
        .await
}

pub async fn list_sessions_for_tracking_ids(
    conn: &mut SqliteConnection,
    tracking_ids: &[String],
) -> Result<Vec<SessionSyncRow>, sqlx::Error> {
    if tracking_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT id, owner_user_id, event_json, tracking_id, calendar_id
      FROM (
        SELECT
          session.id,
          session.owner_user_id,
          session.event_json,
          session.created_at,
          COALESCE(
            NULLIF(event.tracking_id_event, ''),
            CASE
              WHEN json_valid(session.event_json)
              THEN NULLIF(
                CAST(json_extract(session.event_json, '$.tracking_id') AS TEXT),
                ''
              )
              ELSE NULL
            END,
            NULLIF(session.external_event_id, '')
          ) AS tracking_id,
          COALESCE(
            NULLIF(event.calendar_id, ''),
            CASE WHEN json_valid(session.event_json)
              THEN json_extract(session.event_json, '$.calendar_id')
              ELSE NULL END,
            ''
          ) AS calendar_id
        FROM sessions AS session
        LEFT JOIN events AS event
          ON event.id = session.event_id
        WHERE session.deleted_at IS NULL
      ) AS session_with_event
      WHERE tracking_id IN (",
    );
    {
        let mut separated = builder.separated(", ");
        for tracking_id in tracking_ids {
            separated.push_bind(tracking_id);
        }
    }
    builder.push(")\n      ORDER BY created_at, id");
    builder
        .build_query_as::<SessionSyncRow>()
        .fetch_all(&mut *conn)
        .await
}

pub async fn list_humans_by_emails(
    conn: &mut SqliteConnection,
    emails: &[String],
) -> Result<Vec<ParticipantHumanRow>, sqlx::Error> {
    if emails.is_empty() {
        return Ok(Vec::new());
    }
    let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT id, email, name, organization_id
            FROM humans
            WHERE deleted_at IS NULL
              AND lower(email) IN (",
    );
    {
        let mut separated = builder.separated(", ");
        for email in emails {
            separated.push_bind(email);
        }
    }
    builder.push(")\n            ORDER BY created_at, id");
    builder
        .build_query_as::<ParticipantHumanRow>()
        .fetch_all(&mut *conn)
        .await
}

pub async fn list_session_participant_mappings(
    conn: &mut SqliteConnection,
    session_ids: &[String],
) -> Result<Vec<ParticipantMappingRow>, sqlx::Error> {
    if session_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT id, session_id, human_id, source
            FROM session_participants
            WHERE deleted_at IS NULL
              AND session_id IN (",
    );
    {
        let mut separated = builder.separated(", ");
        for session_id in session_ids {
            separated.push_bind(session_id);
        }
    }
    builder.push(")\n            ORDER BY created_at, id");
    builder
        .build_query_as::<ParticipantMappingRow>()
        .fetch_all(&mut *conn)
        .await
}

pub async fn soft_delete_event(
    conn: &mut SqliteConnection,
    now: &str,
    event_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE events
        SET deleted_at = ?, updated_at = ?
        WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(now)
    .bind(now)
    .bind(event_id)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

#[allow(clippy::too_many_arguments)]
pub async fn update_synced_event(
    conn: &mut SqliteConnection,
    id: &str,
    tracking_id_event: &str,
    calendar_id: &str,
    title: &str,
    started_at: &str,
    ended_at: &str,
    location: &str,
    meeting_link: &str,
    description: &str,
    recurrence_series_id: &str,
    has_recurrence_rules: bool,
    is_all_day: bool,
    provider: &str,
    participants_json: Option<&str>,
    attendance_json: Option<&str>,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE events
        SET
          tracking_id_event = ?,
          calendar_id = ?,
          title = ?,
          started_at = ?,
          ended_at = ?,
          location = ?,
          meeting_link = ?,
          description = ?,
          recurrence_series_id = ?,
          has_recurrence_rules = ?,
          is_all_day = ?,
          provider = ?,
          participants_json = ?,
          attendance_json = ?,
          updated_at = ?,
          deleted_at = NULL
        WHERE id = ?",
    )
    .bind(tracking_id_event)
    .bind(calendar_id)
    .bind(title)
    .bind(started_at)
    .bind(ended_at)
    .bind(location)
    .bind(meeting_link)
    .bind(description)
    .bind(recurrence_series_id)
    .bind(has_recurrence_rules)
    .bind(is_all_day)
    .bind(provider)
    .bind(participants_json)
    .bind(attendance_json)
    .bind(now)
    .bind(id)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_synced_event(
    conn: &mut SqliteConnection,
    id: &str,
    tracking_id_event: &str,
    calendar_id: &str,
    title: &str,
    started_at: &str,
    ended_at: &str,
    location: &str,
    meeting_link: &str,
    description: &str,
    recurrence_series_id: &str,
    has_recurrence_rules: bool,
    is_all_day: bool,
    provider: &str,
    participants_json: Option<&str>,
    attendance_json: Option<&str>,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO events (
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
          participants_json,
          attendance_json,
          created_at,
          updated_at,
          deleted_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)",
    )
    .bind(id)
    .bind(tracking_id_event)
    .bind(calendar_id)
    .bind(title)
    .bind(started_at)
    .bind(ended_at)
    .bind(location)
    .bind(meeting_link)
    .bind(description)
    .bind(recurrence_series_id)
    .bind(has_recurrence_rules)
    .bind(is_all_day)
    .bind(provider)
    .bind(participants_json)
    .bind(attendance_json)
    .bind(now)
    .bind(now)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

#[allow(clippy::too_many_arguments)]
pub async fn update_session_for_synced_event(
    conn: &mut SqliteConnection,
    session_id: &str,
    event_id: &str,
    tracking_id: &str,
    provider: &str,
    series_id: &str,
    event_json: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE sessions
        SET
          event_id = CASE WHEN ? = '' THEN event_id ELSE ? END,
          external_event_id = ?,
          external_provider = ?,
          series_id = ?,
          event_json = ?,
          updated_at = ?
        WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(event_id)
    .bind(event_id)
    .bind(tracking_id)
    .bind(provider)
    .bind(series_id)
    .bind(event_json)
    .bind(now)
    .bind(session_id)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn migrate_ignored_event_ids(
    conn: &mut SqliteConnection,
    legacy_main_values_id: &str,
    legacy_settings_id: &str,
    aliases_json: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "WITH current AS (
        SELECT COALESCE(
          (SELECT value_json FROM app_settings WHERE id = 'ignored_events'),
          (SELECT CASE WHEN json_valid(value_json) THEN json_extract(value_json, '$.ignored_events') END
            FROM app_settings WHERE id = ?),
          (SELECT CASE WHEN json_valid(value_json) THEN json_extract(value_json, '$.ignored_events') END
            FROM app_settings WHERE id = ?),
          '[]'
        ) AS value_json
      ), aliases AS (
        SELECT json_extract(value, '$[0]') AS old_id, json_extract(value, '$[1]') AS new_id
        FROM json_each(?)
      ), entries AS (
        SELECT item.key AS position, item.value, aliases.new_id
        FROM current, json_each(CASE WHEN json_valid(current.value_json) THEN current.value_json ELSE '[]' END) AS item
        LEFT JOIN aliases ON aliases.old_id = json_extract(item.value, '$.tracking_id')
      ), normalized AS (
        SELECT position, CASE WHEN new_id IS NULL THEN value
          ELSE json_set(value, '$.tracking_id', new_id) END AS value
        FROM entries
      ), ranked AS (
        SELECT value, position, ROW_NUMBER() OVER (
          PARTITION BY json_extract(value, '$.tracking_id'),
            CASE WHEN json_extract(value, '$.tracking_id') IS NULL THEN position END
          ORDER BY json_extract(value, '$.last_seen') DESC, position
        ) AS rank
        FROM normalized
      )
      INSERT INTO app_settings (id, value_json, updated_at)
      SELECT 'ignored_events', (
        SELECT json_group_array(json(value)) FROM (
          SELECT value FROM ranked WHERE rank = 1 ORDER BY position
        )
      ), ?
      WHERE EXISTS (SELECT 1 FROM entries WHERE new_id IS NOT NULL)
      ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
    )
    .bind(legacy_main_values_id)
    .bind(legacy_settings_id)
    .bind(aliases_json)
    .bind(now)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn insert_organization_if_still_needed(
    conn: &mut SqliteConnection,
    id: &str,
    owner_user_id: &str,
    name: &str,
    now: &str,
    new_human_emails: &[String],
    enrich_human_ids: &[String],
) -> Result<u64, sqlx::Error> {
    let mut still_needed: Vec<String> = Vec::new();
    if !new_human_emails.is_empty() {
        still_needed.push(format!(
            "EXISTS (
          SELECT 1
          FROM (VALUES {}) AS planned
          WHERE NOT EXISTS (
            SELECT 1
            FROM humans
            WHERE lower(email) = planned.column1 AND deleted_at IS NULL
          )
        )",
            new_human_emails
                .iter()
                .map(|_| "(?)")
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !enrich_human_ids.is_empty() {
        still_needed.push(format!(
            "EXISTS (
          SELECT 1
          FROM humans
          WHERE id IN ({})
            AND organization_id = '' AND deleted_at IS NULL
        )",
            enrich_human_ids
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let still_needed_sql = format!("AND ({})", still_needed.join(" OR "));
    let sql = format!(
        "INSERT INTO organizations (
          id, workspace_id, owner_user_id, name, memo, pinned, pin_order,
          metadata_json, created_at, updated_at, deleted_at
        )
        SELECT
          ?,
          {WORKSPACE_ID_SQL},
          {OWNER_USER_ID_SQL},
          ?, '', 0, NULL, '{{}}', ?, ?, NULL
        WHERE NOT EXISTS (
          SELECT 1
          FROM organizations
          WHERE lower(name) = lower(?) AND deleted_at IS NULL
        )
        {still_needed_sql}"
    );
    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(owner_user_id)
        .bind(name)
        .bind(now)
        .bind(now)
        .bind(name);
    for email in new_human_emails {
        query = query.bind(email);
    }
    for human_id in enrich_human_ids {
        query = query.bind(human_id);
    }
    let result = query.execute(&mut *conn).await?;
    Ok(result.rows_affected())
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_human_if_missing(
    conn: &mut SqliteConnection,
    id: &str,
    owner_user_id: &str,
    name: &str,
    email: &str,
    company_name: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let sql = format!(
        "INSERT INTO humans (
          id,
          workspace_id,
          owner_user_id,
          name,
          email,
          organization_id,
          created_at,
          updated_at,
          deleted_at
        )
        SELECT
          ?,
          {WORKSPACE_ID_SQL},
          {OWNER_USER_ID_SQL},
          ?,
          ?,
          {ORGANIZATION_ID_BY_NAME_SQL},
          ?,
          ?,
          NULL
        WHERE NOT EXISTS (
          SELECT 1
          FROM humans
          WHERE deleted_at IS NULL AND lower(email) = lower(?)
        )"
    );
    let result = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(owner_user_id)
        .bind(name)
        .bind(email)
        .bind(company_name)
        .bind(company_name)
        .bind(now)
        .bind(now)
        .bind(email)
        .execute(&mut *conn)
        .await?;
    Ok(result.rows_affected())
}

pub async fn update_human_enrichment(
    conn: &mut SqliteConnection,
    id: &str,
    name: Option<&str>,
    company_name: Option<&str>,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let mut assignments: Vec<String> = Vec::new();
    let mut binds: Vec<String> = Vec::new();
    if let Some(name) = name {
        assignments.push(format!(
            "name = CASE WHEN {HUMAN_NAME_IS_PLACEHOLDER_SQL} THEN ? ELSE name END"
        ));
        binds.push(name.to_string());
    }
    if let Some(company_name) = company_name {
        assignments.push(format!(
            "organization_id = CASE
          WHEN organization_id = '' THEN {ORGANIZATION_ID_BY_NAME_SQL}
          ELSE organization_id
        END"
        ));
        binds.push(company_name.to_string());
        binds.push(company_name.to_string());
    }
    if assignments.is_empty() {
        return Ok(0);
    }
    let sql = format!(
        "UPDATE humans
        SET {}, updated_at = ?
        WHERE id = ? AND deleted_at IS NULL",
        assignments.join(", ")
    );
    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
    for bind in binds {
        query = query.bind(bind);
    }
    let result = query.bind(now).bind(id).execute(&mut *conn).await?;
    Ok(result.rows_affected())
}

pub async fn soft_delete_auto_participant_mapping(
    conn: &mut SqliteConnection,
    now: &str,
    mapping_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE session_participants
        SET deleted_at = ?, updated_at = ?
        WHERE id = ? AND source = 'auto' AND deleted_at IS NULL",
    )
    .bind(now)
    .bind(now)
    .bind(mapping_id)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn insert_session_participant_mapping(
    conn: &mut SqliteConnection,
    id: &str,
    now: &str,
    human_id: &str,
    email: &str,
    session_id: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO session_participants (
          id,
          workspace_id,
          owner_user_id,
          session_id,
          human_id,
          display_name,
          email,
          role,
          source,
          metadata_json,
          created_at,
          updated_at,
          deleted_at
        )
        SELECT
          ?,
          session.workspace_id,
          session.owner_user_id,
          session.id,
          human.id,
          human.name,
          human.email,
          '',
          'auto',
          '{}',
          ?,
          ?,
          NULL
        FROM sessions AS session
        JOIN humans AS human ON human.id = (
          SELECT candidate.id
          FROM humans AS candidate
          WHERE candidate.deleted_at IS NULL
            AND (candidate.id = ? OR lower(candidate.email) = lower(?))
          ORDER BY candidate.id <> ?, candidate.created_at, candidate.id
          LIMIT 1
        )
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
    .bind(id)
    .bind(now)
    .bind(now)
    .bind(human_id)
    .bind(email)
    .bind(human_id)
    .bind(session_id)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn list_app_setting_rows(
    conn: &mut SqliteConnection,
    ids: &[&str],
) -> Result<Vec<AppSettingRow>, sqlx::Error> {
    let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT id, value_json
          FROM app_settings
          WHERE id IN (",
    );
    {
        let mut separated = builder.separated(", ");
        for id in ids {
            separated.push_bind(*id);
        }
    }
    builder.push(")");
    builder
        .build_query_as::<AppSettingRow>()
        .fetch_all(&mut *conn)
        .await
}

pub async fn update_app_setting_value(
    conn: &mut SqliteConnection,
    id: &str,
    next_json: &str,
    now: &str,
    expected_json: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE app_settings
                SET value_json = ?, updated_at = ?
                WHERE id = ? AND value_json = ?",
    )
    .bind(next_json)
    .bind(now)
    .bind(id)
    .bind(expected_json)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

pub async fn insert_app_setting(
    conn: &mut SqliteConnection,
    id: &str,
    next_json: &str,
    now: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO app_settings (id, value_json, updated_at)
                VALUES (?, ?, ?)
                ON CONFLICT(id) DO NOTHING",
    )
    .bind(id)
    .bind(next_json)
    .bind(now)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}
