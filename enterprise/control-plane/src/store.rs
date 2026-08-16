use std::{sync::Arc, time::Duration};

use anlg_session_ingest::{
    AcknowledgeRequest, DeliveryItem, DeliveryPage, SessionIngestEnvelope, SessionRead,
};
use async_trait::async_trait;
use sqlx::{PgPool, Row, postgres::PgPoolOptions};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

#[async_trait]
pub trait DeliveryStore: Send + Sync {
    async fn readiness(&self) -> Result<(), StoreError>;

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

    pub fn into_shared(self) -> Arc<dyn DeliveryStore> {
        Arc::new(self)
    }
}

#[async_trait]
impl DeliveryStore for PostgresStore {
    async fn readiness(&self) -> Result<(), StoreError> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
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
    #[error("{0} is outside the supported range")]
    OutOfRange(&'static str),
}
