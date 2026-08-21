use std::{sync::Arc, time::Duration};

use anlg_meeting_capture::{BotState, CaptureEvent, CaptureEventPayload, CaptureProviderKind};
use anlg_session_ingest::{
    AcknowledgeRequest, DeliveryItem, DeliveryPage, SessionIngestEnvelope, SessionRead,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row, postgres::PgPoolOptions};

use crate::{
    capture::{
        CaptureDispatch, CaptureJob, CaptureJobCheckpoint, CaptureJobLease,
        CaptureJobLeaseIdentity, CaptureJobStatus, ProjectionPublication,
    },
    projector,
    schedule::{
        CalendarEventInput, CapturePolicy, ScheduleDecision, ScheduledCapture,
        ScheduledCaptureStatus, decide_schedule, scheduled_job_id,
    },
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

    async fn find_dispatchable_capture_checkpoint(
        &self,
        workspace_id: &str,
        provider: CaptureProviderKind,
        external_ids: &[String],
    ) -> Result<CaptureJobCheckpoint, StoreError>;

    async fn save_capture_dispatch(&self, dispatch: &CaptureDispatch) -> Result<(), StoreError>;

    async fn list_capture_dispatches(
        &self,
        provider: CaptureProviderKind,
    ) -> Result<Vec<CaptureDispatch>, StoreError>;

    async fn find_capture_dispatch(
        &self,
        provider: CaptureProviderKind,
        dispatch_id: &str,
    ) -> Result<CaptureDispatch, StoreError>;

    async fn next_capture_transcript_sequence(
        &self,
        workspace_id: &str,
        job_id: &str,
    ) -> Result<u64, StoreError>;

    async fn claim_capture_job(
        &self,
        workspace_id: &str,
        job_id: &str,
        worker_id: &str,
        lease_id: &str,
        lease_duration: Duration,
    ) -> Result<CaptureJobLease, StoreError>;

    async fn renew_capture_job_lease(
        &self,
        workspace_id: &str,
        job_id: &str,
        lease: &CaptureJobLeaseIdentity,
        lease_duration: Duration,
    ) -> Result<CaptureJobLease, StoreError>;

    async fn append_capture_event(
        &self,
        workspace_id: &str,
        job_id: &str,
        lease: &CaptureJobLeaseIdentity,
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

    async fn get_capture_policy(&self, workspace_id: &str) -> Result<CapturePolicy, StoreError>;

    async fn upsert_capture_policy(
        &self,
        policy: &CapturePolicy,
    ) -> Result<CapturePolicy, StoreError>;

    async fn upsert_calendar_events(
        &self,
        workspace_id: &str,
        events: &[CalendarEventInput],
    ) -> Result<Vec<ScheduledCapture>, StoreError>;

    async fn list_scheduled_captures(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<ScheduledCapture>, StoreError>;

    async fn cancel_scheduled_capture(
        &self,
        workspace_id: &str,
        calendar_event_id: &str,
    ) -> Result<ScheduledCapture, StoreError>;

    async fn dispatch_due_scheduled_captures(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<CaptureJobStatus>, StoreError>;
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
        capture_checkpoint(row)
    }

    async fn find_dispatchable_capture_checkpoint(
        &self,
        workspace_id: &str,
        provider: CaptureProviderKind,
        external_ids: &[String],
    ) -> Result<CaptureJobCheckpoint, StoreError> {
        if external_ids.is_empty() {
            return Err(StoreError::NotFound);
        }
        let rows = sqlx::query(
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
            WHERE workspace_id = $1
              AND provider = $2
              AND state = 'queued'
              AND meeting->>'external_id' = ANY($3)
            ORDER BY created_at ASC, job_id ASC
            LIMIT 2
            "#,
        )
        .bind(workspace_id)
        .bind(enum_name(provider)?)
        .bind(external_ids)
        .fetch_all(&self.pool)
        .await?;
        match rows.len() {
            0 => Err(StoreError::NotFound),
            1 => capture_checkpoint(rows.into_iter().next().expect("row exists")),
            _ => Err(StoreError::CaptureExternalIdConflict),
        }
    }

    async fn save_capture_dispatch(&self, dispatch: &CaptureDispatch) -> Result<(), StoreError> {
        let provider = enum_name(dispatch.provider)?;
        let mut transaction = self.pool.begin().await?;
        let row = sqlx::query(
            r#"
            SELECT provider, state, dispatch_id, dispatch_payload
            FROM capture_jobs
            WHERE workspace_id = $1 AND job_id = $2
            FOR UPDATE
            "#,
        )
        .bind(&dispatch.workspace_id)
        .bind(&dispatch.job_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(StoreError::NotFound)?;
        if parse_enum::<BotState>(&row.try_get::<String, _>("state")?)?.is_terminal() {
            return Err(StoreError::CaptureJobTerminal);
        }
        if row.try_get::<String, _>("provider")? != provider {
            return Err(StoreError::InvalidCaptureEvent(
                "capture dispatch belongs to a different provider".into(),
            ));
        }
        let current_id = row.try_get::<Option<String>, _>("dispatch_id")?;
        let current_payload = row.try_get::<Option<serde_json::Value>, _>("dispatch_payload")?;
        if current_id.as_deref() == Some(dispatch.dispatch_id.as_str()) {
            if current_payload.as_ref() != Some(&dispatch.payload) {
                sqlx::query(
                    r#"
                    UPDATE capture_jobs
                    SET
                        dispatch_payload = $3,
                        dispatch_accepted_at = clock_timestamp()
                    WHERE workspace_id = $1 AND job_id = $2
                    "#,
                )
                .bind(&dispatch.workspace_id)
                .bind(&dispatch.job_id)
                .bind(&dispatch.payload)
                .execute(&mut *transaction)
                .await?;
            }
            transaction.commit().await?;
            return Ok(());
        }
        if current_id.is_some() || current_payload.is_some() {
            return Err(StoreError::CaptureDispatchConflict);
        }
        sqlx::query(
            r#"
            UPDATE capture_jobs
            SET
                dispatch_id = $3,
                dispatch_payload = $4,
                dispatch_accepted_at = clock_timestamp()
            WHERE workspace_id = $1 AND job_id = $2
            "#,
        )
        .bind(&dispatch.workspace_id)
        .bind(&dispatch.job_id)
        .bind(&dispatch.dispatch_id)
        .bind(&dispatch.payload)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    async fn list_capture_dispatches(
        &self,
        provider: CaptureProviderKind,
    ) -> Result<Vec<CaptureDispatch>, StoreError> {
        let rows = sqlx::query(
            r#"
            SELECT workspace_id, job_id, provider, dispatch_id, dispatch_payload
            FROM capture_jobs
            WHERE provider = $1
              AND dispatch_id IS NOT NULL
              AND state NOT IN ('completed', 'failed', 'canceled')
            ORDER BY dispatch_accepted_at ASC, workspace_id ASC, job_id ASC
            "#,
        )
        .bind(enum_name(provider)?)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(CaptureDispatch {
                    workspace_id: row.try_get("workspace_id")?,
                    job_id: row.try_get("job_id")?,
                    provider: parse_enum(&row.try_get::<String, _>("provider")?)?,
                    dispatch_id: row.try_get("dispatch_id")?,
                    payload: row.try_get("dispatch_payload")?,
                })
            })
            .collect()
    }

    async fn find_capture_dispatch(
        &self,
        provider: CaptureProviderKind,
        dispatch_id: &str,
    ) -> Result<CaptureDispatch, StoreError> {
        let rows = sqlx::query(
            r#"
            SELECT workspace_id, job_id, provider, dispatch_id, dispatch_payload
            FROM capture_jobs
            WHERE provider = $1
              AND dispatch_id = $2
              AND state NOT IN ('completed', 'failed', 'canceled')
            ORDER BY dispatch_accepted_at ASC, workspace_id ASC, job_id ASC
            LIMIT 2
            "#,
        )
        .bind(enum_name(provider)?)
        .bind(dispatch_id)
        .fetch_all(&self.pool)
        .await?;
        match rows.len() {
            0 => Err(StoreError::NotFound),
            1 => {
                let row = rows.into_iter().next().expect("row exists");
                Ok(CaptureDispatch {
                    workspace_id: row.try_get("workspace_id")?,
                    job_id: row.try_get("job_id")?,
                    provider: parse_enum(&row.try_get::<String, _>("provider")?)?,
                    dispatch_id: row.try_get("dispatch_id")?,
                    payload: row.try_get("dispatch_payload")?,
                })
            }
            _ => Err(StoreError::CaptureDispatchConflict),
        }
    }

    async fn next_capture_transcript_sequence(
        &self,
        workspace_id: &str,
        job_id: &str,
    ) -> Result<u64, StoreError> {
        let rows = sqlx::query(
            r#"
            SELECT event
            FROM capture_events
            WHERE workspace_id = $1
              AND job_id = $2
              AND event->'payload'->>'type' = 'transcript'
            "#,
        )
        .bind(workspace_id)
        .bind(job_id)
        .fetch_all(&self.pool)
        .await?;
        let mut next_sequence = 0;
        for row in rows {
            let event = serde_json::from_value::<CaptureEvent>(row.try_get("event")?)?;
            if let CaptureEventPayload::Transcript(transcript) = event.payload {
                next_sequence = next_sequence.max(
                    transcript
                        .sequence
                        .checked_add(1)
                        .ok_or(StoreError::OutOfRange("transcript sequence"))?,
                );
            }
        }
        Ok(next_sequence)
    }

    async fn claim_capture_job(
        &self,
        workspace_id: &str,
        job_id: &str,
        worker_id: &str,
        lease_id: &str,
        lease_duration: Duration,
    ) -> Result<CaptureJobLease, StoreError> {
        let lease_seconds = lease_seconds(lease_duration)?;
        let mut transaction = self.pool.begin().await?;
        let row = sqlx::query(
            r#"
            SELECT state, lease_owner, lease_id, lease_epoch, lease_expires_at
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
        let state: BotState = parse_enum(&row.try_get::<String, _>("state")?)?;
        if state.is_terminal() {
            return Err(StoreError::CaptureJobTerminal);
        }

        let current_owner = row.try_get::<Option<String>, _>("lease_owner")?;
        let current_lease_id = row.try_get::<Option<String>, _>("lease_id")?;
        let current_epoch = row.try_get::<i64, _>("lease_epoch")?;
        let current_expires_at = row.try_get::<Option<DateTime<Utc>>, _>("lease_expires_at")?;
        let now = sqlx::query_scalar::<_, DateTime<Utc>>("SELECT clock_timestamp()")
            .fetch_one(&mut *transaction)
            .await?;
        let current_is_active = current_expires_at.is_some_and(|expires_at| expires_at > now);
        if current_is_active
            && (current_owner.as_deref() != Some(worker_id)
                || current_lease_id.as_deref() != Some(lease_id))
        {
            return Err(StoreError::CaptureLeaseUnavailable);
        }

        let epoch = if current_is_active {
            current_epoch
        } else {
            current_epoch
                .checked_add(1)
                .ok_or(StoreError::OutOfRange("capture lease epoch"))?
        };
        let row = sqlx::query(
            r#"
            UPDATE capture_jobs
            SET
                lease_owner = $3,
                lease_id = $4,
                lease_epoch = $5,
                lease_expires_at = clock_timestamp() + $6 * INTERVAL '1 second'
            WHERE workspace_id = $1 AND job_id = $2
            RETURNING lease_expires_at
            "#,
        )
        .bind(workspace_id)
        .bind(job_id)
        .bind(worker_id)
        .bind(lease_id)
        .bind(epoch)
        .bind(lease_seconds)
        .fetch_one(&mut *transaction)
        .await?;
        let lease = CaptureJobLease {
            worker_id: worker_id.to_string(),
            lease_id: lease_id.to_string(),
            epoch: from_i64(epoch, "capture lease epoch")?,
            expires_at: row.try_get("lease_expires_at")?,
        };
        transaction.commit().await?;
        Ok(lease)
    }

    async fn renew_capture_job_lease(
        &self,
        workspace_id: &str,
        job_id: &str,
        lease: &CaptureJobLeaseIdentity,
        lease_duration: Duration,
    ) -> Result<CaptureJobLease, StoreError> {
        let lease_seconds = lease_seconds(lease_duration)?;
        let epoch = to_i64(lease.epoch, "capture lease epoch")?;
        let row = sqlx::query(
            r#"
            UPDATE capture_jobs
            SET lease_expires_at = clock_timestamp() + $6 * INTERVAL '1 second'
            WHERE workspace_id = $1
              AND job_id = $2
              AND lease_owner = $3
              AND lease_id = $4
              AND lease_epoch = $5
              AND lease_expires_at > clock_timestamp()
              AND state NOT IN ('completed', 'failed', 'canceled')
            RETURNING lease_expires_at
            "#,
        )
        .bind(workspace_id)
        .bind(job_id)
        .bind(&lease.worker_id)
        .bind(&lease.lease_id)
        .bind(epoch)
        .bind(lease_seconds)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::CaptureLeaseLost)?;
        Ok(CaptureJobLease {
            worker_id: lease.worker_id.clone(),
            lease_id: lease.lease_id.clone(),
            epoch: lease.epoch,
            expires_at: row.try_get("lease_expires_at")?,
        })
    }

    async fn append_capture_event(
        &self,
        workspace_id: &str,
        job_id: &str,
        lease: &CaptureJobLeaseIdentity,
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
                last_sequence,
                lease_owner,
                lease_id,
                lease_epoch,
                lease_expires_at > clock_timestamp() AS lease_active
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
        let lease_owner = job_row.try_get::<Option<String>, _>("lease_owner")?;
        let lease_id = job_row.try_get::<Option<String>, _>("lease_id")?;
        let lease_epoch = job_row.try_get::<i64, _>("lease_epoch")?;
        let lease_active = job_row.try_get::<Option<bool>, _>("lease_active")? == Some(true);
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

        let active_lease = lease_owner.as_deref() == Some(lease.worker_id.as_str())
            && lease_id.as_deref() == Some(lease.lease_id.as_str())
            && lease_epoch == to_i64(lease.epoch, "capture lease epoch")?
            && lease_active;
        if !active_lease {
            return Err(StoreError::CaptureLeaseLost);
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
                updated_at = $6,
                dispatch_id = CASE
                    WHEN $3 IN ('completed', 'failed', 'canceled') THEN NULL
                    ELSE dispatch_id
                END,
                dispatch_payload = CASE
                    WHEN $3 IN ('completed', 'failed', 'canceled') THEN NULL
                    ELSE dispatch_payload
                END,
                dispatch_accepted_at = CASE
                    WHEN $3 IN ('completed', 'failed', 'canceled') THEN NULL
                    ELSE dispatch_accepted_at
                END
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

    async fn get_capture_policy(&self, workspace_id: &str) -> Result<CapturePolicy, StoreError> {
        let row = sqlx::query(
            r#"
            SELECT workspace_id, capture_enabled, allowed_providers, bot_name,
                   disclosure_text, skip_if_desktop_capture
            FROM capture_policies
            WHERE workspace_id = $1
            "#,
        )
        .bind(workspace_id)
        .fetch_optional(&self.pool)
        .await?;
        match row {
            Some(row) => capture_policy(row),
            None => Ok(CapturePolicy::default_off(workspace_id)),
        }
    }

    async fn upsert_capture_policy(
        &self,
        policy: &CapturePolicy,
    ) -> Result<CapturePolicy, StoreError> {
        validate_policy(policy)?;
        let allowed_providers = serde_json::to_value(&policy.allowed_providers)?;
        sqlx::query(
            r#"
            INSERT INTO capture_policies (
                workspace_id,
                capture_enabled,
                allowed_providers,
                bot_name,
                disclosure_text,
                skip_if_desktop_capture,
                updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, clock_timestamp())
            ON CONFLICT (workspace_id) DO UPDATE SET
                capture_enabled = EXCLUDED.capture_enabled,
                allowed_providers = EXCLUDED.allowed_providers,
                bot_name = EXCLUDED.bot_name,
                disclosure_text = EXCLUDED.disclosure_text,
                skip_if_desktop_capture = EXCLUDED.skip_if_desktop_capture,
                updated_at = clock_timestamp()
            "#,
        )
        .bind(&policy.workspace_id)
        .bind(policy.capture_enabled)
        .bind(&allowed_providers)
        .bind(&policy.bot_name)
        .bind(&policy.disclosure_text)
        .bind(policy.skip_if_desktop_capture)
        .execute(&self.pool)
        .await?;
        self.get_capture_policy(&policy.workspace_id).await
    }

    async fn upsert_calendar_events(
        &self,
        workspace_id: &str,
        events: &[CalendarEventInput],
    ) -> Result<Vec<ScheduledCapture>, StoreError> {
        let policy = self.get_capture_policy(workspace_id).await?;
        let mut transaction = self.pool.begin().await?;
        let mut stored = Vec::with_capacity(events.len());
        for event in events {
            stored.push(upsert_scheduled_capture(&mut transaction, &policy, event).await?);
        }
        transaction.commit().await?;
        Ok(stored)
    }

    async fn list_scheduled_captures(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<ScheduledCapture>, StoreError> {
        let rows = sqlx::query(
            r#"
            SELECT workspace_id, calendar_event_id, job_id, title, starts_at, ends_at,
                   meeting, provider, owner_user_id, status, skip_reason, bot_name,
                   disclosure_text
            FROM scheduled_captures
            WHERE workspace_id = $1
            ORDER BY starts_at ASC, calendar_event_id ASC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(scheduled_capture).collect()
    }

    async fn cancel_scheduled_capture(
        &self,
        workspace_id: &str,
        calendar_event_id: &str,
    ) -> Result<ScheduledCapture, StoreError> {
        let row = sqlx::query(
            r#"
            UPDATE scheduled_captures
            SET
                status = CASE
                    WHEN status = 'dispatched' THEN status
                    ELSE 'canceled'
                END,
                skip_reason = CASE
                    WHEN status = 'dispatched' THEN skip_reason
                    ELSE 'canceled_by_user'
                END,
                updated_at = clock_timestamp()
            WHERE workspace_id = $1 AND calendar_event_id = $2
            RETURNING workspace_id, calendar_event_id, job_id, title, starts_at, ends_at,
                      meeting, provider, owner_user_id, status, skip_reason, bot_name,
                      disclosure_text
            "#,
        )
        .bind(workspace_id)
        .bind(calendar_event_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(scheduled_capture)
            .transpose()?
            .ok_or(StoreError::NotFound)
    }

    async fn dispatch_due_scheduled_captures(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<CaptureJobStatus>, StoreError> {
        let mut transaction = self.pool.begin().await?;
        let rows = sqlx::query(
            r#"
            SELECT workspace_id, calendar_event_id, job_id, title, starts_at, ends_at,
                   meeting, provider, owner_user_id, status, skip_reason, bot_name,
                   disclosure_text
            FROM scheduled_captures
            WHERE status = 'pending' AND starts_at <= $1
            ORDER BY starts_at ASC, workspace_id ASC, calendar_event_id ASC
            FOR UPDATE SKIP LOCKED
            "#,
        )
        .bind(now)
        .fetch_all(&mut *transaction)
        .await?;
        let mut dispatched = Vec::new();
        for row in rows {
            let scheduled = scheduled_capture(row)?;
            let job_id = scheduled
                .job_id
                .clone()
                .unwrap_or_else(|| scheduled_job_id(&scheduled.calendar_event_id));
            let bot_id = format!("bot-{job_id}");
            let created = sqlx::query(
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
                ) VALUES ($1, $2, $3, $4, $4, $2, $5, $6, $7, $8, $8)
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(&scheduled.workspace_id)
            .bind(&job_id)
            .bind(&bot_id)
            .bind(&scheduled.owner_user_id)
            .bind(&scheduled.title)
            .bind(enum_name(scheduled.provider)?)
            .bind(serde_json::to_value(&scheduled.meeting)?)
            .bind(now)
            .execute(&mut *transaction)
            .await?
            .rows_affected()
                == 1;
            sqlx::query(
                r#"
                UPDATE scheduled_captures
                SET
                    job_id = $3,
                    status = 'dispatched',
                    skip_reason = NULL,
                    updated_at = clock_timestamp()
                WHERE workspace_id = $1 AND calendar_event_id = $2
                "#,
            )
            .bind(&scheduled.workspace_id)
            .bind(&scheduled.calendar_event_id)
            .bind(&job_id)
            .execute(&mut *transaction)
            .await?;
            dispatched.push(CaptureJobStatus {
                job_id,
                created,
                state: BotState::Queued,
            });
        }
        transaction.commit().await?;
        Ok(dispatched)
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

fn capture_checkpoint(row: sqlx::postgres::PgRow) -> Result<CaptureJobCheckpoint, StoreError> {
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

fn validate_policy(policy: &CapturePolicy) -> Result<(), StoreError> {
    if policy.bot_name.is_empty()
        || policy.bot_name.len() > 80
        || policy.bot_name.chars().any(char::is_control)
    {
        return Err(StoreError::InvalidCaptureEvent(
            "capture bot name must contain 1-80 non-control characters".into(),
        ));
    }
    if policy.allowed_providers.is_empty() {
        return Err(StoreError::InvalidCaptureEvent(
            "capture policy must allow at least one provider".into(),
        ));
    }
    if let Some(disclosure) = &policy.disclosure_text
        && (disclosure.is_empty() || disclosure.len() > 2048)
    {
        return Err(StoreError::InvalidCaptureEvent(
            "disclosure text must contain 1-2048 bytes".into(),
        ));
    }
    Ok(())
}

fn capture_policy(row: sqlx::postgres::PgRow) -> Result<CapturePolicy, StoreError> {
    Ok(CapturePolicy {
        workspace_id: row.try_get("workspace_id")?,
        capture_enabled: row.try_get("capture_enabled")?,
        allowed_providers: serde_json::from_value(row.try_get("allowed_providers")?)?,
        bot_name: row.try_get("bot_name")?,
        disclosure_text: row.try_get("disclosure_text")?,
        skip_if_desktop_capture: row.try_get("skip_if_desktop_capture")?,
    })
}

fn scheduled_capture(row: sqlx::postgres::PgRow) -> Result<ScheduledCapture, StoreError> {
    Ok(ScheduledCapture {
        workspace_id: row.try_get("workspace_id")?,
        calendar_event_id: row.try_get("calendar_event_id")?,
        job_id: row.try_get("job_id")?,
        title: row.try_get("title")?,
        starts_at: row.try_get("starts_at")?,
        ends_at: row.try_get("ends_at")?,
        meeting: serde_json::from_value(row.try_get("meeting")?)?,
        provider: parse_enum(&row.try_get::<String, _>("provider")?)?,
        owner_user_id: row.try_get("owner_user_id")?,
        status: parse_enum(&row.try_get::<String, _>("status")?)?,
        skip_reason: row.try_get("skip_reason")?,
        bot_name: row.try_get("bot_name")?,
        disclosure_text: row.try_get("disclosure_text")?,
    })
}

async fn upsert_scheduled_capture(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    policy: &CapturePolicy,
    event: &CalendarEventInput,
) -> Result<ScheduledCapture, StoreError> {
    if event.calendar_event_id.is_empty()
        || event.calendar_event_id.len() > 512
        || event.calendar_event_id.chars().any(char::is_control)
    {
        return Err(StoreError::InvalidCaptureEvent(
            "calendar event id must contain 1-512 non-control characters".into(),
        ));
    }
    if event.title.trim().is_empty() || event.title.len() > 1024 {
        return Err(StoreError::InvalidCaptureEvent(
            "calendar event title must contain 1-1024 bytes".into(),
        ));
    }
    let decision = decide_schedule(policy, event);
    let (status, skip_reason, job_id) = match decision {
        ScheduleDecision::Pending => (
            ScheduledCaptureStatus::Pending,
            None,
            Some(scheduled_job_id(&event.calendar_event_id)),
        ),
        ScheduleDecision::Skipped(reason) => (ScheduledCaptureStatus::Skipped, Some(reason), None),
        ScheduleDecision::Canceled(reason) => {
            (ScheduledCaptureStatus::Canceled, Some(reason), None)
        }
    };
    let meeting = serde_json::to_value(&event.meeting)?;
    sqlx::query(
        r#"
        INSERT INTO scheduled_captures (
            workspace_id,
            calendar_event_id,
            job_id,
            title,
            starts_at,
            ends_at,
            meeting,
            provider,
            owner_user_id,
            status,
            skip_reason,
            bot_name,
            disclosure_text,
            updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, clock_timestamp())
        ON CONFLICT (workspace_id, calendar_event_id) DO UPDATE SET
            job_id = CASE
                WHEN scheduled_captures.status = 'dispatched' THEN scheduled_captures.job_id
                ELSE EXCLUDED.job_id
            END,
            title = EXCLUDED.title,
            starts_at = EXCLUDED.starts_at,
            ends_at = EXCLUDED.ends_at,
            meeting = EXCLUDED.meeting,
            provider = EXCLUDED.provider,
            owner_user_id = EXCLUDED.owner_user_id,
            status = CASE
                WHEN scheduled_captures.status = 'dispatched' THEN scheduled_captures.status
                ELSE EXCLUDED.status
            END,
            skip_reason = CASE
                WHEN scheduled_captures.status = 'dispatched' THEN scheduled_captures.skip_reason
                ELSE EXCLUDED.skip_reason
            END,
            bot_name = EXCLUDED.bot_name,
            disclosure_text = EXCLUDED.disclosure_text,
            updated_at = clock_timestamp()
        "#,
    )
    .bind(&policy.workspace_id)
    .bind(&event.calendar_event_id)
    .bind(&job_id)
    .bind(&event.title)
    .bind(event.starts_at)
    .bind(event.ends_at)
    .bind(&meeting)
    .bind(enum_name(event.provider)?)
    .bind(&event.owner_user_id)
    .bind(enum_name(status)?)
    .bind(skip_reason)
    .bind(&policy.bot_name)
    .bind(&policy.disclosure_text)
    .execute(&mut **transaction)
    .await?;
    let row = sqlx::query(
        r#"
        SELECT workspace_id, calendar_event_id, job_id, title, starts_at, ends_at,
               meeting, provider, owner_user_id, status, skip_reason, bot_name,
               disclosure_text
        FROM scheduled_captures
        WHERE workspace_id = $1 AND calendar_event_id = $2
        "#,
    )
    .bind(&policy.workspace_id)
    .bind(&event.calendar_event_id)
    .fetch_one(&mut **transaction)
    .await?;
    scheduled_capture(row)
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

fn lease_seconds(duration: Duration) -> Result<i64, StoreError> {
    i64::try_from(duration.as_secs())
        .ok()
        .filter(|seconds| *seconds > 0)
        .ok_or(StoreError::OutOfRange("capture lease duration"))
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
    #[error("capture provider identity matches more than one dispatchable job")]
    CaptureExternalIdConflict,
    #[error("capture event id or sequence already exists with different content")]
    CaptureEventConflict,
    #[error("capture job already has a different durable dispatch")]
    CaptureDispatchConflict,
    #[error("capture event sequence conflict: expected {expected}, got {actual}")]
    CaptureSequenceConflict { expected: u64, actual: u64 },
    #[error("capture job is already terminal")]
    CaptureJobTerminal,
    #[error("capture job already has an active lease")]
    CaptureLeaseUnavailable,
    #[error("capture job lease is no longer active")]
    CaptureLeaseLost,
    #[error("capture event is invalid: {0}")]
    InvalidCaptureEvent(String),
    #[error("stored capture JSON is invalid")]
    Json(#[from] serde_json::Error),
    #[error("{0} is outside the supported range")]
    OutOfRange(&'static str),
}
