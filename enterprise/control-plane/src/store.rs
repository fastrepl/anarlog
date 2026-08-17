use std::{sync::Arc, time::Duration};

use anlg_meeting_capture::{BotState, CaptureEvent, CaptureEventPayload};
use anlg_session_ingest::{
    AcknowledgeRequest, DeliveryItem, DeliveryPage, SessionIngestEnvelope, SessionRead,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row, postgres::PgPoolOptions};

use crate::{
    capture::{CaptureJob, CaptureJobCheckpoint, CaptureJobStatus, ProjectionPublication},
    projector,
};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

#[async_trait]
pub trait ControlPlaneStore: Send + Sync {
    async fn readiness(&self) -> Result<(), StoreError>;

    async fn create_capture_job(&self, job: &CaptureJob) -> Result<CaptureJobStatus, StoreError>;

    async fn read_capture_checkpoint(
        &self,
        workspace_id: &str,
        job_id: &str,
    ) -> Result<CaptureJobCheckpoint, StoreError>;

    async fn append_capture_event(
        &self,
        workspace_id: &str,
        job_id: &str,
        event: &CaptureEvent,
    ) -> Result<ProjectionPublication, StoreError>;

    async fn list_deliveries(
        &self,
        workspace_id: &str,
        consumer_id: &str,
        after: u64,
        limit: u16,
    ) -> Result<DeliveryPage, StoreError>;

    async fn acknowledge(
        &self,
        workspace_id: &str,
        job_id: &str,
        request: &AcknowledgeRequest,
    ) -> Result<(), StoreError>;

    async fn read_session(
        &self,
        workspace_id: &str,
        job_id: &str,
    ) -> Result<SessionRead, StoreError>;
}

#[derive(Clone)]
pub struct PostgresStore {
    pool: PgPool,
}

impl PostgresStore {
    pub async fn connect(
        database_url: &str,
        max_connections: u32,
        acquire_timeout: Duration,
    ) -> Result<Self, StoreError> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(acquire_timeout)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<(), StoreError> {
        MIGRATOR.run(&self.pool).await?;
        Ok(())
    }

    pub fn into_shared(self) -> Arc<dyn ControlPlaneStore> {
        Arc::new(self)
    }
}

#[async_trait]
impl ControlPlaneStore for PostgresStore {
    async fn readiness(&self) -> Result<(), StoreError> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    async fn create_capture_job(&self, job: &CaptureJob) -> Result<CaptureJobStatus, StoreError> {
        let provider = enum_name(job.provider)?;
        let meeting = serde_json::to_value(&job.meeting)?;
        let result = sqlx::query(
            r#"
            INSERT INTO capture_jobs (
                workspace_id,
                job_id,
                bot_id,
                owner_user_id,
                requesting_actor_id,
                session_id,
                session_title,
                provider,
                meeting,
                created_at,
                updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(&job.workspace_id)
        .bind(&job.job_id)
        .bind(&job.bot_id)
        .bind(&job.owner_user_id)
        .bind(&job.requesting_actor_id)
        .bind(&job.session_id)
        .bind(&job.session_title)
        .bind(&provider)
        .bind(&meeting)
        .bind(job.created_at)
        .execute(&self.pool)
        .await?;
        let created = result.rows_affected() == 1;

        let stored = sqlx::query(
            r#"
            SELECT
                bot_id,
                owner_user_id,
                requesting_actor_id,
                session_id,
                session_title,
                provider,
                meeting,
                state,
                created_at
            FROM capture_jobs
            WHERE workspace_id = $1 AND job_id = $2
            "#,
        )
        .bind(&job.workspace_id)
        .bind(&job.job_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(stored) = stored else {
            let bot_owner = sqlx::query_scalar::<_, String>(
                r#"
                SELECT job_id
                FROM capture_jobs
                WHERE workspace_id = $1 AND bot_id = $2
                "#,
            )
            .bind(&job.workspace_id)
            .bind(&job.bot_id)
            .fetch_optional(&self.pool)
            .await?;
            return Err(if bot_owner.is_some() {
                StoreError::CaptureBotConflict
            } else {
                StoreError::CaptureJobConflict
            });
        };
        let stored_created_at = stored.try_get::<DateTime<Utc>, _>("created_at")?;
        if stored.try_get::<String, _>("bot_id")? != job.bot_id
            || stored.try_get::<String, _>("owner_user_id")? != job.owner_user_id
            || stored.try_get::<String, _>("requesting_actor_id")? != job.requesting_actor_id
            || stored.try_get::<String, _>("session_id")? != job.session_id
            || stored.try_get::<String, _>("session_title")? != job.session_title
            || stored.try_get::<String, _>("provider")? != provider
            || stored.try_get::<serde_json::Value, _>("meeting")? != meeting
            || stored_created_at.timestamp_micros() != job.created_at.timestamp_micros()
        {
            return Err(StoreError::CaptureJobConflict);
        }

        Ok(CaptureJobStatus {
            job_id: job.job_id.clone(),
            created,
            state: parse_enum(&stored.try_get::<String, _>("state")?)?,
        })
    }

    async fn read_capture_checkpoint(
        &self,
        workspace_id: &str,
        job_id: &str,
    ) -> Result<CaptureJobCheckpoint, StoreError> {
        let row = sqlx::query(
            r#"
            SELECT
                workspace_id,
                job_id,
                bot_id,
                owner_user_id,
                requesting_actor_id,
                session_id,
                session_title,
                provider,
                meeting,
                state,
                last_sequence,
                created_at
            FROM capture_jobs
            WHERE workspace_id = $1 AND job_id = $2
            "#,
        )
        .bind(workspace_id)
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)?;
        let state = parse_enum(&row.try_get::<String, _>("state")?)?;
        let next_sequence = row
            .try_get::<i64, _>("last_sequence")?
            .checked_add(1)
            .ok_or(StoreError::OutOfRange("capture event sequence"))?;
        Ok(CaptureJobCheckpoint {
            job: capture_job(row)?,
            state,
            next_sequence: from_i64(next_sequence, "capture event sequence")?,
        })
    }

    async fn append_capture_event(
        &self,
        workspace_id: &str,
        job_id: &str,
        event: &CaptureEvent,
    ) -> Result<ProjectionPublication, StoreError> {
        let mut transaction = self.pool.begin().await?;
        let job_row = sqlx::query(
            r#"
            SELECT
                workspace_id,
                job_id,
                bot_id,
                owner_user_id,
                requesting_actor_id,
                session_id,
                session_title,
                provider,
                meeting,
                created_at,
                last_sequence
            FROM capture_jobs
            WHERE workspace_id = $1 AND job_id = $2
            FOR UPDATE
            "#,
        )
        .bind(workspace_id)
        .bind(job_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(StoreError::NotFound)?;
        let last_sequence = job_row.try_get::<i64, _>("last_sequence")?;
        let job = capture_job(job_row)?;
        if event.bot_id != job.bot_id {
            return Err(StoreError::InvalidCaptureEvent(
                "capture event belongs to a different bot".into(),
            ));
        }

        let event_value = serde_json::to_value(event)?;
        let sequence = to_i64(event.sequence, "capture event sequence")?;
        let existing = sqlx::query(
            r#"
            SELECT sequence, event_id, event
            FROM capture_events
            WHERE workspace_id = $1
              AND job_id = $2
              AND (sequence = $3 OR event_id = $4)
            "#,
        )
        .bind(workspace_id)
        .bind(job_id)
        .bind(sequence)
        .bind(&event.id)
        .fetch_optional(&mut *transaction)
        .await?;
        if let Some(existing) = existing {
            let identical = existing.try_get::<i64, _>("sequence")? == sequence
                && existing.try_get::<String, _>("event_id")? == event.id
                && existing.try_get::<serde_json::Value, _>("event")? == event_value;
            if !identical {
                return Err(StoreError::CaptureEventConflict);
            }
            let revision = event
                .sequence
                .checked_add(1)
                .ok_or(StoreError::OutOfRange("revision"))?;
            let publication =
                read_publication(&mut transaction, workspace_id, job_id, revision).await?;
            transaction.commit().await?;
            return Ok(publication);
        }

        let expected = last_sequence
            .checked_add(1)
            .ok_or(StoreError::OutOfRange("capture event sequence"))?;
        if sequence != expected {
            return Err(StoreError::CaptureSequenceConflict {
                expected: from_i64(expected, "capture event sequence")?,
                actual: event.sequence,
            });
        }

        let rows = sqlx::query(
            r#"
            SELECT event
            FROM capture_events
            WHERE workspace_id = $1 AND job_id = $2
            ORDER BY sequence ASC
            "#,
        )
        .bind(workspace_id)
        .bind(job_id)
        .fetch_all(&mut *transaction)
        .await?;
        let mut events = rows
            .into_iter()
            .map(|row| -> Result<CaptureEvent, StoreError> {
                let value = row.try_get("event")?;
                Ok(serde_json::from_value(value)?)
            })
            .collect::<Result<Vec<CaptureEvent>, StoreError>>()?;
        events.push(event.clone());
        let envelope = projector::project(&job, &events)
            .map_err(|error| StoreError::InvalidCaptureEvent(error.to_string()))?;
        let content_hash = projector::content_hash(&envelope)?;
        let revision = to_i64(envelope.revision, "revision")?;
        let finalized = envelope.finalized;
        let envelope_value = serde_json::to_value(&envelope)?;

        sqlx::query(
            r#"
            INSERT INTO capture_events (
                workspace_id,
                job_id,
                sequence,
                event_id,
                occurred_at,
                event
            ) VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(workspace_id)
        .bind(job_id)
        .bind(sequence)
        .bind(&event.id)
        .bind(event.occurred_at)
        .bind(&event_value)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO session_envelopes (
                workspace_id,
                job_id,
                revision,
                finalized,
                content_hash,
                envelope
            ) VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(workspace_id)
        .bind(job_id)
        .bind(revision)
        .bind(finalized)
        .bind(&content_hash)
        .bind(&envelope_value)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO capture_projection_checkpoints (
                workspace_id,
                job_id,
                last_sequence,
                revision,
                content_hash
            ) VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (workspace_id, job_id)
            DO UPDATE SET
                last_sequence = EXCLUDED.last_sequence,
                revision = EXCLUDED.revision,
                content_hash = EXCLUDED.content_hash,
                updated_at = now()
            WHERE capture_projection_checkpoints.last_sequence < EXCLUDED.last_sequence
            "#,
        )
        .bind(workspace_id)
        .bind(job_id)
        .bind(sequence)
        .bind(revision)
        .bind(&content_hash)
        .execute(&mut *transaction)
        .await?;
        let (state, terminal_reason) = projected_lifecycle(&events);
        sqlx::query(
            r#"
            UPDATE capture_jobs
            SET
                state = $3,
                terminal_reason = $4,
                last_sequence = $5,
                updated_at = $6
            WHERE workspace_id = $1 AND job_id = $2
            "#,
        )
        .bind(workspace_id)
        .bind(job_id)
        .bind(enum_name(state)?)
        .bind(
            terminal_reason
                .as_ref()
                .map(serde_json::to_value)
                .transpose()?,
        )
        .bind(sequence)
        .bind(event.occurred_at)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;

        Ok(ProjectionPublication {
            job_id: job_id.to_string(),
            revision: envelope.revision,
            finalized,
            content_hash,
            envelope,
        })
    }

    async fn list_deliveries(
        &self,
        workspace_id: &str,
        consumer_id: &str,
        after: u64,
        limit: u16,
    ) -> Result<DeliveryPage, StoreError> {
        let after = to_i64(after, "delivery cursor")?;
        let fetch_limit = i64::from(limit) + 1;
        let rows = sqlx::query(
            r#"
            SELECT
                envelopes.cursor,
                envelopes.job_id,
                envelopes.revision,
                envelopes.finalized,
                envelopes.content_hash,
                envelopes.envelope,
                to_char(
                    envelopes.created_at AT TIME ZONE 'UTC',
                    'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"'
                ) AS created_at,
                acknowledgements.job_id IS NOT NULL AS acknowledged
            FROM session_envelopes AS envelopes
            LEFT JOIN session_envelope_acknowledgements AS acknowledgements
              ON acknowledgements.workspace_id = envelopes.workspace_id
             AND acknowledgements.consumer_id = $2
             AND acknowledgements.job_id = envelopes.job_id
             AND acknowledgements.revision = envelopes.revision
            WHERE envelopes.workspace_id = $1
              AND envelopes.cursor > $3
            ORDER BY envelopes.cursor ASC
            LIMIT $4
            "#,
        )
        .bind(workspace_id)
        .bind(consumer_id)
        .bind(after)
        .bind(fetch_limit)
        .fetch_all(&self.pool)
        .await?;

        let has_more = rows.len() > usize::from(limit);
        let mut items = Vec::with_capacity(rows.len().min(usize::from(limit)));
        for row in rows.into_iter().take(usize::from(limit)) {
            items.push(delivery_item(workspace_id, row)?);
        }
        let next_cursor = items.last().map_or(after as u64, |item| item.cursor);
        Ok(DeliveryPage {
            items,
            next_cursor,
            has_more,
        })
    }

    async fn acknowledge(
        &self,
        workspace_id: &str,
        job_id: &str,
        request: &AcknowledgeRequest,
    ) -> Result<(), StoreError> {
        let revision = to_i64(request.revision, "revision")?;
        let acknowledged = sqlx::query_scalar::<_, bool>(
            r#"
            INSERT INTO session_envelope_acknowledgements (
                workspace_id,
                consumer_id,
                job_id,
                revision,
                content_hash
            )
            SELECT
                envelopes.workspace_id,
                $3,
                envelopes.job_id,
                envelopes.revision,
                envelopes.content_hash
            FROM session_envelopes AS envelopes
            WHERE envelopes.workspace_id = $1
              AND envelopes.job_id = $2
              AND envelopes.revision = $4
              AND envelopes.content_hash = $5
            ON CONFLICT (workspace_id, consumer_id, job_id, revision)
            DO UPDATE SET
                content_hash = EXCLUDED.content_hash,
                acknowledged_at = now()
            RETURNING TRUE
            "#,
        )
        .bind(workspace_id)
        .bind(job_id)
        .bind(&request.consumer_id)
        .bind(revision)
        .bind(&request.content_hash)
        .fetch_optional(&self.pool)
        .await?;
        if acknowledged.is_some() {
            return Ok(());
        }

        let current = sqlx::query(
            r#"
            SELECT revision, content_hash
            FROM session_envelopes
            WHERE workspace_id = $1 AND job_id = $2
            ORDER BY revision DESC
            LIMIT 1
            "#,
        )
        .bind(workspace_id)
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await?;
        match current {
            None => Err(StoreError::NotFound),
            Some(_) => Err(StoreError::RevisionConflict),
        }
    }

    async fn read_session(
        &self,
        workspace_id: &str,
        job_id: &str,
    ) -> Result<SessionRead, StoreError> {
        let row = sqlx::query(
            r#"
            SELECT job_id, revision, finalized, content_hash, envelope
            FROM session_envelopes
            WHERE workspace_id = $1 AND job_id = $2
            ORDER BY revision DESC
            LIMIT 1
            "#,
        )
        .bind(workspace_id)
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)?;

        let revision = from_i64(row.try_get("revision")?, "revision")?;
        let finalized = row.try_get("finalized")?;
        let envelope = parse_envelope(workspace_id, revision, finalized, row.try_get("envelope")?)?;
        Ok(SessionRead {
            job_id: row.try_get("job_id")?,
            revision,
            finalized,
            content_hash: row.try_get("content_hash")?,
            envelope,
        })
    }
}

fn delivery_item(
    workspace_id: &str,
    row: sqlx::postgres::PgRow,
) -> Result<DeliveryItem, StoreError> {
    let cursor = from_i64(row.try_get("cursor")?, "delivery cursor")?;
    let revision = from_i64(row.try_get("revision")?, "revision")?;
    let finalized = row.try_get("finalized")?;
    let envelope = parse_envelope(workspace_id, revision, finalized, row.try_get("envelope")?)?;
    Ok(DeliveryItem {
        cursor,
        job_id: row.try_get("job_id")?,
        revision,
        finalized,
        content_hash: row.try_get("content_hash")?,
        acknowledged: row.try_get("acknowledged")?,
        created_at: row.try_get("created_at")?,
        envelope,
    })
}

fn parse_envelope(
    workspace_id: &str,
    revision: u64,
    finalized: bool,
    value: serde_json::Value,
) -> Result<SessionIngestEnvelope, StoreError> {
    let envelope = serde_json::from_value::<SessionIngestEnvelope>(value)
        .map_err(|_| StoreError::CorruptEnvelope)?;
    if envelope.workspace_id != workspace_id
        || envelope.revision != revision
        || envelope.finalized != finalized
    {
        return Err(StoreError::CorruptEnvelope);
    }
    Ok(envelope)
}

fn capture_job(row: sqlx::postgres::PgRow) -> Result<CaptureJob, StoreError> {
    Ok(CaptureJob {
        workspace_id: row.try_get("workspace_id")?,
        job_id: row.try_get("job_id")?,
        bot_id: row.try_get("bot_id")?,
        owner_user_id: row.try_get("owner_user_id")?,
        requesting_actor_id: row.try_get("requesting_actor_id")?,
        session_id: row.try_get("session_id")?,
        session_title: row.try_get("session_title")?,
        provider: parse_enum(&row.try_get::<String, _>("provider")?)?,
        meeting: serde_json::from_value(row.try_get("meeting")?)?,
        created_at: row.try_get("created_at")?,
    })
}

async fn read_publication(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace_id: &str,
    job_id: &str,
    revision: u64,
) -> Result<ProjectionPublication, StoreError> {
    let row = sqlx::query(
        r#"
        SELECT revision, finalized, content_hash, envelope
        FROM session_envelopes
        WHERE workspace_id = $1 AND job_id = $2 AND revision = $3
        "#,
    )
    .bind(workspace_id)
    .bind(job_id)
    .bind(to_i64(revision, "revision")?)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(StoreError::CorruptEnvelope)?;
    let finalized = row.try_get("finalized")?;
    let envelope = parse_envelope(workspace_id, revision, finalized, row.try_get("envelope")?)?;
    Ok(ProjectionPublication {
        job_id: job_id.to_string(),
        revision,
        finalized,
        content_hash: row.try_get("content_hash")?,
        envelope,
    })
}

fn projected_lifecycle(
    events: &[CaptureEvent],
) -> (BotState, Option<anlg_meeting_capture::TerminalReason>) {
    events
        .iter()
        .fold((BotState::Queued, None), |current, event| {
            if let CaptureEventPayload::Lifecycle(transition) = &event.payload {
                (transition.to, transition.reason.clone())
            } else {
                current
            }
        })
}

fn enum_name<T: serde::Serialize>(value: T) -> Result<String, StoreError> {
    serde_json::to_value(value)?
        .as_str()
        .map(str::to_owned)
        .ok_or(StoreError::CorruptEnvelope)
}

fn parse_enum<T: serde::de::DeserializeOwned>(value: &str) -> Result<T, StoreError> {
    Ok(serde_json::from_value(serde_json::Value::String(
        value.to_string(),
    ))?)
}

fn to_i64(value: u64, field: &'static str) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| StoreError::OutOfRange(field))
}

fn from_i64(value: i64, field: &'static str) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::OutOfRange(field))
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("database migration failed")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("delivery was not found")]
    NotFound,
    #[error("delivery revision or content hash does not match")]
    RevisionConflict,
    #[error("stored delivery envelope is invalid")]
    CorruptEnvelope,
    #[error("capture job already exists with different immutable fields")]
    CaptureJobConflict,
    #[error("capture bot is already assigned to a different job")]
    CaptureBotConflict,
    #[error("capture event id or sequence already exists with different content")]
    CaptureEventConflict,
    #[error("capture event sequence conflict: expected {expected}, got {actual}")]
    CaptureSequenceConflict { expected: u64, actual: u64 },
    #[error("capture event is invalid: {0}")]
    InvalidCaptureEvent(String),
    #[error("stored capture JSON is invalid")]
    Json(#[from] serde_json::Error),
    #[error("{0} is outside the supported range")]
    OutOfRange(&'static str),
}
