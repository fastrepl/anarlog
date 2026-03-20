use sqlx::SqlitePool;

use crate::{SlackChannelRow, SlackMessageRow, SlackTeamRow, SlackThreadRow};

pub async fn upsert_slack_team(
    pool: &SqlitePool,
    id: &str,
    connection_id: &str,
    team_id: &str,
    team_name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR REPLACE INTO slack_teams (id, connection_id, team_id, team_name) VALUES (?, ?, ?, ?)",
    )
    .bind(id)
    .bind(connection_id)
    .bind(team_id)
    .bind(team_name)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_slack_team(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<SlackTeamRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT id, connection_id, team_id, team_name, created_at, user_id FROM slack_teams WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(id, connection_id, team_id, team_name, created_at, user_id)| SlackTeamRow {
            id,
            connection_id,
            team_id,
            team_name,
            created_at,
            user_id,
        },
    ))
}

pub async fn upsert_slack_channel(
    pool: &SqlitePool,
    id: &str,
    slack_team_id: &str,
    channel_id: &str,
    name: &str,
    channel_type: &str,
    is_external: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR REPLACE INTO slack_channels (id, slack_team_id, channel_id, name, channel_type, is_external) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(slack_team_id)
    .bind(channel_id)
    .bind(name)
    .bind(channel_type)
    .bind(is_external as i32)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_slack_channel(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<SlackChannelRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, (String, String, String, String, String, i32, String, String)>(
        "SELECT id, slack_team_id, channel_id, name, channel_type, is_external, created_at, user_id FROM slack_channels WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(id, slack_team_id, channel_id, name, channel_type, is_external, created_at, user_id)| {
            SlackChannelRow {
                id,
                slack_team_id,
                channel_id,
                name,
                channel_type,
                is_external: is_external != 0,
                created_at,
                user_id,
            }
        },
    ))
}

pub async fn upsert_slack_thread(
    pool: &SqlitePool,
    id: &str,
    channel_id: &str,
    thread_ts: &str,
    started_at: &str,
    last_message_at: &str,
    message_count: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR REPLACE INTO slack_threads (id, channel_id, thread_ts, started_at, last_message_at, message_count) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(channel_id)
    .bind(thread_ts)
    .bind(started_at)
    .bind(last_message_at)
    .bind(message_count)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_slack_thread(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<SlackThreadRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, (String, String, String, String, String, i32, String, String)>(
        "SELECT id, channel_id, thread_ts, started_at, last_message_at, message_count, created_at, user_id FROM slack_threads WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(
        |(
            id,
            channel_id,
            thread_ts,
            started_at,
            last_message_at,
            message_count,
            created_at,
            user_id,
        )| {
            SlackThreadRow {
                id,
                channel_id,
                thread_ts,
                started_at,
                last_message_at,
                message_count,
                created_at,
                user_id,
            }
        },
    ))
}

pub async fn insert_slack_message(
    pool: &SqlitePool,
    id: &str,
    thread_id: &str,
    channel_id: &str,
    alias_id: &str,
    text: &str,
    ts: &str,
    raw_json: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO slack_messages (id, thread_id, channel_id, alias_id, text, ts, raw_json) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(thread_id)
    .bind(channel_id)
    .bind(alias_id)
    .bind(text)
    .bind(ts)
    .bind(raw_json)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_slack_messages_by_thread(
    pool: &SqlitePool,
    thread_id: &str,
) -> Result<Vec<SlackMessageRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String, String, String)>(
        "SELECT id, thread_id, channel_id, alias_id, text, ts, raw_json, created_at, user_id FROM slack_messages WHERE thread_id = ? ORDER BY ts",
    )
    .bind(thread_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, thread_id, channel_id, alias_id, text, ts, raw_json, created_at, user_id)| {
                SlackMessageRow {
                    id,
                    thread_id,
                    channel_id,
                    alias_id,
                    text,
                    ts,
                    raw_json,
                    created_at,
                    user_id,
                }
            },
        )
        .collect())
}

pub async fn upsert_slack_thread_participant(
    pool: &SqlitePool,
    id: &str,
    thread_id: &str,
    alias_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR REPLACE INTO slack_thread_participants (id, thread_id, alias_id) VALUES (?, ?, ?)",
    )
    .bind(id)
    .bind(thread_id)
    .bind(alias_id)
    .execute(pool)
    .await?;
    Ok(())
}
