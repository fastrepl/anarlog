use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};
use futures_util::{StreamExt, stream};
use serde::{Deserialize, Serialize};
use stripe_core::customer::{DeleteCustomer, RetrieveCustomer, RetrieveCustomerReturned};
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use uuid::{Uuid, Version};

use crate::{
    SubscriptionConfig,
    cloudsync_cleanup::CloudsyncCleanupClient,
    error::{Result, SubscriptionError},
    supabase::SupabaseClient,
};

const ATTACHMENT_BACKUP_BUCKET: &str = "attachment-backups";
const SHARED_ATTACHMENT_BUCKET: &str = "shared-note-attachments";
const AUDIO_BUCKET: &str = "audio-files";
const ATTACHMENT_BATCH_SIZE: usize = 32;
const ATTACHMENT_CONCURRENCY: usize = 4;
const ATTACHMENT_LEASE_SECONDS: i32 = 300;
const ACCOUNT_BATCH_SIZE: usize = 4;
const ACCOUNT_LEASE_SECONDS: i32 = 900;
const POLL_INTERVAL: Duration = Duration::from_secs(30);
const MAX_ACCOUNT_PREFIX_OBJECTS: usize = 20_000;
const MAX_CIPHERTEXT_SIZE_BYTES: i64 = 545_259_520;
const STRIPE_DELETE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct CleanupWorker {
    supabase: SupabaseClient,
    storage: hypr_supabase_storage::SupabaseStorage,
    stripe: stripe::Client,
    cloudsync: Option<CloudsyncCleanupClient>,
}

#[derive(Serialize)]
struct ClaimRequest {
    p_lease_id: String,
    p_limit: i32,
    p_lease_seconds: i32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AttachmentLeaseRow {
    object_id: String,
    owner_user_id: String,
    object_key: String,
    ciphertext_size_bytes: i64,
    gc_lease_id: String,
    gc_lease_expires_at: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SharedAttachmentLeaseRow {
    attachment_id: String,
    owner_user_id: String,
    share_id: String,
    object_key: String,
    size_bytes: i64,
    gc_lease_id: String,
    gc_lease_expires_at: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountLeaseRow {
    owner_user_id: String,
    final_sweep_not_before: String,
    stripe_customer_id: Option<String>,
    stripe_deleted: bool,
    cleanup_ready: bool,
    prefix_swept: bool,
    e2ee_workspace_ids: Vec<String>,
    e2ee_purged: bool,
    lease_id: String,
    lease_expires_at: String,
}

#[derive(Serialize)]
struct FinishAttachmentRequest<'a> {
    p_owner_user_id: &'a str,
    p_object_id: &'a str,
    p_object_key: &'a str,
    p_gc_lease_id: &'a str,
}

#[derive(Serialize)]
struct FinishSharedAttachmentRequest<'a> {
    p_attachment_id: &'a str,
    p_object_key: &'a str,
    p_gc_lease_id: &'a str,
}

#[derive(Serialize)]
struct AccountLeaseRequest<'a> {
    p_owner_user_id: &'a str,
    p_lease_id: &'a str,
}

#[derive(Serialize)]
struct StripeDeleteRequest<'a> {
    p_owner_user_id: &'a str,
    p_lease_id: &'a str,
    p_stripe_customer_id: Option<&'a str>,
}

#[derive(Serialize)]
struct E2eePurgeRequest<'a> {
    p_owner_user_id: &'a str,
    p_lease_id: &'a str,
    p_workspace_ids: &'a [String],
}

impl CleanupWorker {
    pub fn new(config: &SubscriptionConfig) -> Self {
        let supabase = SupabaseClient::new(
            config.supabase.supabase_url.clone(),
            config.supabase.supabase_anon_key.clone(),
            config.supabase.supabase_service_role_key.clone(),
        );
        let storage = supabase.storage();
        let stripe = stripe::Client::new(&config.stripe.stripe_secret_key);
        let cloudsync = config
            .cloudsync_cleanup
            .clone()
            .map(CloudsyncCleanupClient::new);
        Self {
            supabase,
            storage,
            stripe,
            cloudsync,
        }
    }

    pub async fn run(self, cancellation: CancellationToken) {
        let mut interval = tokio::time::interval(POLL_INTERVAL);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = cancellation.cancelled() => break,
                _ = interval.tick() => {
                    let full_batch = self.run_once(&cancellation).await;
                    if full_batch {
                        interval.reset_immediately();
                    }
                }
            }
        }
        tracing::info!("durable_cleanup_worker_stopped");
    }

    async fn run_once(&self, cancellation: &CancellationToken) -> bool {
        let attachment_count = match self.run_attachment_batch(cancellation).await {
            Ok(count) => count,
            Err(error) => {
                tracing::warn!(error = %error, "attachment_backup_gc_batch_failed");
                0
            }
        };
        let shared_attachment_count = match self.run_shared_attachment_batch(cancellation).await {
            Ok(count) => count,
            Err(error) => {
                tracing::warn!(error = %error, "shared_attachment_gc_batch_failed");
                0
            }
        };
        let account_count = match self.run_account_batch(cancellation).await {
            Ok(count) => count,
            Err(error) => {
                tracing::warn!(error = %error, "account_deletion_batch_failed");
                0
            }
        };
        attachment_count == ATTACHMENT_BATCH_SIZE
            || shared_attachment_count == ATTACHMENT_BATCH_SIZE
            || account_count == ACCOUNT_BATCH_SIZE
    }

    async fn run_attachment_batch(&self, cancellation: &CancellationToken) -> Result<usize> {
        if cancellation.is_cancelled() {
            return Ok(0);
        }
        let lease_id = Uuid::now_v7().to_string();
        let rows: Vec<AttachmentLeaseRow> = self
            .supabase
            .admin_rpc(
                "claim_attachment_backup_gc_leases",
                &ClaimRequest {
                    p_lease_id: lease_id.clone(),
                    p_limit: ATTACHMENT_BATCH_SIZE as i32,
                    p_lease_seconds: ATTACHMENT_LEASE_SECONDS,
                },
            )
            .await?;
        if rows.len() > ATTACHMENT_BATCH_SIZE {
            return Err(invalid_upstream("attachment GC lease count"));
        }
        for row in &rows {
            validate_attachment_lease(row, &lease_id)?;
        }
        let count = rows.len();
        let mut results = stream::iter(rows)
            .map(|row| {
                let worker = self.clone();
                let cancellation = cancellation.clone();
                async move {
                    if cancellation.is_cancelled() {
                        return Ok(());
                    }
                    worker.delete_attachment(row).await
                }
            })
            .buffer_unordered(ATTACHMENT_CONCURRENCY);
        while let Some(result) = results.next().await {
            if let Err(error) = result {
                tracing::warn!(error = %error, "attachment_backup_gc_object_failed");
            }
        }
        Ok(count)
    }

    async fn delete_attachment(&self, row: AttachmentLeaseRow) -> Result<()> {
        self.storage
            .delete_file(ATTACHMENT_BACKUP_BUCKET, &row.object_key)
            .await
            .map_err(storage_error)?;
        let finished: bool = self
            .supabase
            .admin_rpc(
                "finish_attachment_backup_deletion",
                &FinishAttachmentRequest {
                    p_owner_user_id: &row.owner_user_id,
                    p_object_id: &row.object_id,
                    p_object_key: &row.object_key,
                    p_gc_lease_id: &row.gc_lease_id,
                },
            )
            .await?;
        if !finished {
            tracing::info!("attachment_backup_gc_already_finished");
        }
        Ok(())
    }

    async fn run_shared_attachment_batch(&self, cancellation: &CancellationToken) -> Result<usize> {
        if cancellation.is_cancelled() {
            return Ok(0);
        }
        let lease_id = Uuid::now_v7().to_string();
        let rows: Vec<SharedAttachmentLeaseRow> = self
            .supabase
            .admin_rpc(
                "claim_session_share_attachment_gc_leases",
                &ClaimRequest {
                    p_lease_id: lease_id.clone(),
                    p_limit: ATTACHMENT_BATCH_SIZE as i32,
                    p_lease_seconds: ATTACHMENT_LEASE_SECONDS,
                },
            )
            .await?;
        if rows.len() > ATTACHMENT_BATCH_SIZE {
            return Err(invalid_upstream("shared attachment GC lease count"));
        }
        for row in &rows {
            validate_shared_attachment_lease(row, &lease_id)?;
        }
        let count = rows.len();
        let mut results = stream::iter(rows)
            .map(|row| {
                let worker = self.clone();
                let cancellation = cancellation.clone();
                async move {
                    if cancellation.is_cancelled() {
                        return Ok(());
                    }
                    worker.delete_shared_attachment(row).await
                }
            })
            .buffer_unordered(ATTACHMENT_CONCURRENCY);
        while let Some(result) = results.next().await {
            if let Err(error) = result {
                tracing::warn!(error = %error, "shared_attachment_gc_object_failed");
            }
        }
        Ok(count)
    }

    async fn delete_shared_attachment(&self, row: SharedAttachmentLeaseRow) -> Result<()> {
        self.storage
            .delete_file(SHARED_ATTACHMENT_BUCKET, &row.object_key)
            .await
            .map_err(storage_error)?;
        let finished: bool = self
            .supabase
            .admin_rpc(
                "finish_session_share_attachment_deletion",
                &FinishSharedAttachmentRequest {
                    p_attachment_id: &row.attachment_id,
                    p_object_key: &row.object_key,
                    p_gc_lease_id: &row.gc_lease_id,
                },
            )
            .await?;
        if !finished {
            tracing::info!("shared_attachment_gc_already_finished");
        }
        Ok(())
    }

    async fn run_account_batch(&self, cancellation: &CancellationToken) -> Result<usize> {
        if cancellation.is_cancelled() {
            return Ok(0);
        }
        let lease_id = Uuid::now_v7().to_string();
        let rows: Vec<AccountLeaseRow> = self
            .supabase
            .admin_rpc(
                "claim_account_deletion_leases_v2",
                &ClaimRequest {
                    p_lease_id: lease_id.clone(),
                    p_limit: ACCOUNT_BATCH_SIZE as i32,
                    p_lease_seconds: ACCOUNT_LEASE_SECONDS,
                },
            )
            .await?;
        if rows.len() > ACCOUNT_BATCH_SIZE {
            return Err(invalid_upstream("account deletion lease count"));
        }
        for row in &rows {
            validate_account_lease(row, &lease_id)?;
        }
        let count = rows.len();
        for row in rows {
            if cancellation.is_cancelled() {
                break;
            }
            if let Err(error) = self.delete_account(row, cancellation).await {
                tracing::warn!(error = %error, "account_deletion_job_failed");
            }
        }
        Ok(count)
    }

    async fn delete_account(
        &self,
        row: AccountLeaseRow,
        cancellation: &CancellationToken,
    ) -> Result<()> {
        if !row.stripe_deleted {
            if cancellation.is_cancelled() {
                return Ok(());
            }
            if let Some(customer_id) = row.stripe_customer_id.as_deref() {
                tokio::time::timeout(
                    STRIPE_DELETE_TIMEOUT,
                    self.delete_stripe_customer(&row.owner_user_id, customer_id),
                )
                .await
                .map_err(|_| {
                    SubscriptionError::Stripe("Stripe customer deletion timed out".to_string())
                })??;
            }
            if cancellation.is_cancelled() {
                return Ok(());
            }
            let marked: bool = self
                .supabase
                .admin_rpc(
                    "mark_account_deletion_stripe_deleted",
                    &StripeDeleteRequest {
                        p_owner_user_id: &row.owner_user_id,
                        p_lease_id: &row.lease_id,
                        p_stripe_customer_id: row.stripe_customer_id.as_deref(),
                    },
                )
                .await?;
            if !marked {
                return Err(invalid_upstream("account deletion Stripe checkpoint"));
            }
        }

        if cancellation.is_cancelled() || !row.cleanup_ready {
            return Ok(());
        }

        if !row.prefix_swept {
            let prefix = format!("{}/", row.owner_user_id);
            match self
                .storage
                .clear_prefix_until(
                    ATTACHMENT_BACKUP_BUCKET,
                    &prefix,
                    MAX_ACCOUNT_PREFIX_OBJECTS,
                    || cancellation.is_cancelled(),
                )
                .await
            {
                Ok(_) => {}
                Err(hypr_supabase_storage::Error::Cancelled) => return Ok(()),
                Err(error) => return Err(storage_error(error)),
            }
            if cancellation.is_cancelled() {
                return Ok(());
            }
            match self
                .storage
                .clear_prefix_until(AUDIO_BUCKET, &prefix, MAX_ACCOUNT_PREFIX_OBJECTS, || {
                    cancellation.is_cancelled()
                })
                .await
            {
                Ok(_) => {}
                Err(hypr_supabase_storage::Error::Cancelled) => return Ok(()),
                Err(error) => return Err(storage_error(error)),
            }
            if cancellation.is_cancelled() {
                return Ok(());
            }
            match self
                .storage
                .clear_prefix_until(
                    SHARED_ATTACHMENT_BUCKET,
                    &prefix,
                    MAX_ACCOUNT_PREFIX_OBJECTS,
                    || cancellation.is_cancelled(),
                )
                .await
            {
                Ok(_) => {}
                Err(hypr_supabase_storage::Error::Cancelled) => return Ok(()),
                Err(error) => return Err(storage_error(error)),
            }
            if cancellation.is_cancelled() {
                return Ok(());
            }
            let marked: bool = self
                .supabase
                .admin_rpc(
                    "mark_account_deletion_prefix_swept",
                    &AccountLeaseRequest {
                        p_owner_user_id: &row.owner_user_id,
                        p_lease_id: &row.lease_id,
                    },
                )
                .await?;
            if !marked {
                return Err(invalid_upstream("account deletion sweep checkpoint"));
            }
        }

        if !row.e2ee_purged {
            if cancellation.is_cancelled() {
                return Ok(());
            }
            let cloudsync = self.cloudsync.as_ref().ok_or_else(|| {
                SubscriptionError::Internal("CloudSync cleanup is not configured".to_string())
            })?;
            cloudsync.purge_and_confirm(&row.e2ee_workspace_ids).await?;
            if cancellation.is_cancelled() {
                return Ok(());
            }
            let marked: bool = self
                .supabase
                .admin_rpc(
                    "mark_account_deletion_e2ee_purged",
                    &E2eePurgeRequest {
                        p_owner_user_id: &row.owner_user_id,
                        p_lease_id: &row.lease_id,
                        p_workspace_ids: &row.e2ee_workspace_ids,
                    },
                )
                .await?;
            if !marked {
                return Err(invalid_upstream("account deletion E2EE checkpoint"));
            }
        }

        if cancellation.is_cancelled() {
            return Ok(());
        }

        self.supabase.admin_delete_user(&row.owner_user_id).await?;
        let finished: bool = self
            .supabase
            .admin_rpc(
                "finish_account_deletion",
                &AccountLeaseRequest {
                    p_owner_user_id: &row.owner_user_id,
                    p_lease_id: &row.lease_id,
                },
            )
            .await?;
        if !finished {
            tracing::info!("account_deletion_already_finished");
        }
        Ok(())
    }

    async fn delete_stripe_customer(&self, owner_user_id: &str, customer_id: &str) -> Result<()> {
        match RetrieveCustomer::new(customer_id).send(&self.stripe).await {
            Ok(RetrieveCustomerReturned::Customer(customer)) => {
                let metadata_owner_ids = customer
                    .metadata
                    .as_ref()
                    .map(|metadata| {
                        ["userId", "user_id", "userID"]
                            .into_iter()
                            .filter_map(|key| metadata.get(key).map(String::as_str))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let metadata_matches = !metadata_owner_ids.is_empty()
                    && metadata_owner_ids
                        .iter()
                        .all(|metadata_owner_id| *metadata_owner_id == owner_user_id);
                let email_matches = if metadata_owner_ids.is_empty() {
                    match (
                        customer.email.as_deref(),
                        self.supabase.admin_get_user_email(owner_user_id).await?,
                    ) {
                        (Some(customer_email), Some(user_email)) => {
                            customer_email.eq_ignore_ascii_case(&user_email)
                        }
                        _ => false,
                    }
                } else {
                    false
                };
                if !metadata_matches && !email_matches {
                    return Err(SubscriptionError::Internal(
                        "Stripe customer ownership could not be verified".to_string(),
                    ));
                }
            }
            Ok(RetrieveCustomerReturned::DeletedCustomer(_))
            | Err(stripe::StripeError::Stripe(_, 404)) => {
                tracing::info!(
                    enduser.id = %owner_user_id,
                    hyprnote.billing.customer.id = %customer_id,
                    "account_deletion_stripe_customer_already_deleted"
                );
                return Ok(());
            }
            Err(error) => return Err(SubscriptionError::Stripe(error.to_string())),
        }

        match DeleteCustomer::new(customer_id).send(&self.stripe).await {
            Ok(_) => {
                tracing::info!(
                    enduser.id = %owner_user_id,
                    hyprnote.billing.customer.id = %customer_id,
                    "account_deletion_stripe_customer_deleted"
                );
                Ok(())
            }
            Err(stripe::StripeError::Stripe(_, 404)) => {
                tracing::info!(
                    enduser.id = %owner_user_id,
                    hyprnote.billing.customer.id = %customer_id,
                    "account_deletion_stripe_customer_already_deleted"
                );
                Ok(())
            }
            Err(error) => Err(SubscriptionError::Stripe(error.to_string())),
        }
    }
}

fn validate_attachment_lease(row: &AttachmentLeaseRow, expected_lease_id: &str) -> Result<()> {
    let object_id = canonical_uuid(&row.object_id, Some(Version::Random))?;
    let owner_user_id = canonical_uuid(&row.owner_user_id, None)?;
    let object_key = validate_backup_object_key(&row.object_key, &owner_user_id)?;
    let lease_id = canonical_uuid(&row.gc_lease_id, Some(Version::SortRand))?;
    let lease_expires_at = validate_lease_expiry(&row.gc_lease_expires_at)?;
    if object_id != row.object_id
        || object_key != row.object_key
        || lease_id != expected_lease_id
        || !(1..=MAX_CIPHERTEXT_SIZE_BYTES).contains(&row.ciphertext_size_bytes)
        || lease_expires_at <= Utc::now()
    {
        return Err(invalid_upstream("attachment GC lease"));
    }
    Ok(())
}

fn validate_account_lease(row: &AccountLeaseRow, expected_lease_id: &str) -> Result<()> {
    let owner_user_id = canonical_uuid(&row.owner_user_id, None)?;
    let lease_id = canonical_uuid(&row.lease_id, Some(Version::SortRand))?;
    let horizon = parse_timestamp(&row.final_sweep_not_before)?;
    let lease_expires_at = validate_lease_expiry(&row.lease_expires_at)?;
    if row
        .stripe_customer_id
        .as_deref()
        .is_some_and(|customer_id| !is_valid_stripe_customer_id(customer_id))
    {
        return Err(invalid_upstream("account deletion Stripe customer"));
    }
    let mut previous_workspace_id = None;
    for workspace_id in &row.e2ee_workspace_ids {
        let workspace_id = canonical_uuid(workspace_id, None)?;
        if previous_workspace_id
            .as_ref()
            .is_some_and(|previous| previous >= &workspace_id)
        {
            return Err(invalid_upstream("account deletion E2EE workspace scope"));
        }
        previous_workspace_id = Some(workspace_id);
    }
    let now = Utc::now();
    if lease_id != expected_lease_id
        || (row.cleanup_ready && horizon > now)
        || (!row.cleanup_ready && row.stripe_deleted)
        || lease_expires_at <= now
        || row.e2ee_workspace_ids.is_empty()
        || row.e2ee_workspace_ids.len() > 1_000
        || !row.e2ee_workspace_ids.contains(&owner_user_id)
    {
        return Err(invalid_upstream("account deletion lease"));
    }
    Ok(())
}

fn is_valid_stripe_customer_id(value: &str) -> bool {
    value.len() <= 255
        && value.strip_prefix("cus_").is_some_and(|suffix| {
            !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}

fn validate_shared_attachment_lease(
    row: &SharedAttachmentLeaseRow,
    expected_lease_id: &str,
) -> Result<()> {
    let attachment_id = canonical_uuid(&row.attachment_id, Some(Version::Random))?;
    let owner_user_id = canonical_uuid(&row.owner_user_id, None)?;
    let share_id = canonical_uuid(&row.share_id, None)?;
    let object_key = validate_shared_attachment_object_key(
        &row.object_key,
        &owner_user_id,
        &share_id,
        &attachment_id,
    )?;
    let lease_id = canonical_uuid(&row.gc_lease_id, Some(Version::SortRand))?;
    let lease_expires_at = validate_lease_expiry(&row.gc_lease_expires_at)?;
    if object_key != row.object_key
        || lease_id != expected_lease_id
        || !(1..=536_870_912).contains(&row.size_bytes)
        || lease_expires_at <= Utc::now()
    {
        return Err(invalid_upstream("shared attachment GC lease"));
    }
    Ok(())
}

fn validate_lease_expiry(value: &str) -> Result<DateTime<Utc>> {
    let expiry = parse_timestamp(value)?;
    if expiry > Utc::now() + TimeDelta::seconds(3605) {
        return Err(invalid_upstream("cleanup lease expiry"));
    }
    Ok(expiry)
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| invalid_upstream("cleanup timestamp"))
}

fn canonical_uuid(value: &str, version: Option<Version>) -> Result<String> {
    let uuid = Uuid::parse_str(value).map_err(|_| invalid_upstream("cleanup UUID"))?;
    let canonical = uuid.to_string();
    if canonical != value || version.is_some_and(|version| uuid.get_version() != Some(version)) {
        return Err(invalid_upstream("cleanup UUID"));
    }
    Ok(canonical)
}

fn validate_backup_object_key(value: &str, owner_user_id: &str) -> Result<String> {
    let (owner, filename) = value
        .split_once('/')
        .ok_or_else(|| invalid_upstream("attachment object key"))?;
    let object_id = filename
        .strip_suffix(".anb1")
        .ok_or_else(|| invalid_upstream("attachment object key"))?;
    let object_uuid =
        Uuid::parse_str(object_id).map_err(|_| invalid_upstream("attachment object key"))?;
    if owner != owner_user_id
        || filename.contains('/')
        || object_uuid.to_string() != object_id
        || !matches!(
            object_uuid.get_version(),
            Some(Version::Random | Version::SortRand)
        )
    {
        return Err(invalid_upstream("attachment object key"));
    }
    Ok(value.to_string())
}

fn validate_shared_attachment_object_key(
    value: &str,
    owner_user_id: &str,
    share_id: &str,
    attachment_id: &str,
) -> Result<String> {
    let expected = format!("{owner_user_id}/{share_id}/{attachment_id}.sna1");
    if value != expected {
        return Err(invalid_upstream("shared attachment object key"));
    }
    Ok(value.to_string())
}

fn storage_error(error: hypr_supabase_storage::Error) -> SubscriptionError {
    SubscriptionError::Internal(format!("Storage cleanup failed: {error}"))
}

fn invalid_upstream(context: &str) -> SubscriptionError {
    SubscriptionError::Internal(format!("Invalid {context} response"))
}

#[cfg(test)]
mod tests {
    use hypr_api_env::{LoopsEnv, StripeEnv, SupabaseEnv};
    use serde_json::{Value, json};
    use wiremock::{
        Mock, MockServer, Request, ResponseTemplate,
        matchers::{method, path},
    };

    use super::*;

    const OWNER: &str = "00000000-0000-4000-8000-000000000501";
    const OBJECT: &str = "00000000-0000-4000-8000-000000000502";
    const SHARE: &str = "00000000-0000-4000-8000-000000000503";
    const WORKSPACE: &str = "00000000-0000-4000-8000-000000000504";
    const CUSTOMER: &str = "cus_cleanup501";

    fn worker(server: &MockServer) -> CleanupWorker {
        let mut worker = CleanupWorker::new(
            &SubscriptionConfig::new(
                &SupabaseEnv {
                    supabase_url: server.uri(),
                    supabase_anon_key: "anon-key".to_string(),
                    supabase_service_role_key: "service-role-key".to_string(),
                },
                &StripeEnv {
                    stripe_secret_key: "sk_test_fake".to_string(),
                    stripe_monthly_price_id: "price_monthly".to_string(),
                    stripe_yearly_price_id: "price_yearly".to_string(),
                },
                &LoopsEnv {
                    loops_key: "loops-key".to_string(),
                },
            )
            .with_cloudsync_cleanup(crate::CloudsyncCleanupConfig::for_test(&server.uri())),
        );
        worker.stripe = stripe::ClientBuilder::new("sk_test_fake")
            .url(format!("{}/", server.uri()))
            .build()
            .unwrap();
        worker
    }

    fn request_lease(request: &Request) -> String {
        serde_json::from_slice::<Value>(&request.body).unwrap()["p_lease_id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    async fn mount_attachment_claim(server: &MockServer, invalid_key: bool) {
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/claim_attachment_backup_gc_leases"))
            .respond_with(move |request: &Request| {
                let lease_id = request_lease(request);
                let object_key = if invalid_key {
                    format!("{OWNER}/../{OBJECT}.anb1")
                } else {
                    format!("{OWNER}/{OBJECT}.anb1")
                };
                ResponseTemplate::new(200).set_body_json(json!([{
                    "object_id": OBJECT,
                    "owner_user_id": OWNER,
                    "object_key": object_key,
                    "ciphertext_size_bytes": 1024,
                    "gc_lease_id": lease_id,
                    "gc_lease_expires_at": (Utc::now() + TimeDelta::minutes(5)).to_rfc3339()
                }]))
            })
            .mount(server)
            .await;
    }

    async fn mount_account_claim(server: &MockServer, prefix_swept: bool, e2ee_purged: bool) {
        mount_account_claim_with_stripe(server, prefix_swept, e2ee_purged, None, true, false).await;
    }

    async fn mount_account_claim_with_stripe(
        server: &MockServer,
        prefix_swept: bool,
        e2ee_purged: bool,
        stripe_customer_id: Option<&str>,
        stripe_deleted: bool,
        before_horizon: bool,
    ) {
        let stripe_customer_id = stripe_customer_id.map(ToString::to_string);
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/claim_account_deletion_leases_v2"))
            .respond_with(move |request: &Request| {
                let lease_id = request_lease(request);
                ResponseTemplate::new(200).set_body_json(json!([{
                    "owner_user_id": OWNER,
                    "final_sweep_not_before": if before_horizon {
                        (Utc::now() + TimeDelta::hours(24)).to_rfc3339()
                    } else {
                        (Utc::now() - TimeDelta::minutes(1)).to_rfc3339()
                    },
                    "stripe_customer_id": stripe_customer_id,
                    "stripe_deleted": stripe_deleted,
                    "cleanup_ready": !before_horizon,
                    "prefix_swept": prefix_swept,
                    "e2ee_workspace_ids": [OWNER, WORKSPACE],
                    "e2ee_purged": e2ee_purged,
                    "lease_id": lease_id,
                    "lease_expires_at": (Utc::now() + TimeDelta::minutes(15)).to_rfc3339()
                }]))
            })
            .mount(server)
            .await;
    }

    async fn mount_cloudsync_cleanup(server: &MockServer, remaining: u64) {
        Mock::given(method("GET"))
            .and(path("/v1/databases/managed-e2ee"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "managedDatabaseId": "managed-e2ee",
                    "databaseName": "e2ee.sqlite",
                    "projectId": "test-project"
                }
            })))
            .mount(server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v2/weblite/sql"))
            .respond_with(move |request: &Request| {
                let body: Value = serde_json::from_slice(&request.body).unwrap();
                let sql = body["sql"].as_str().unwrap();
                if sql.starts_with("DELETE FROM e2ee_records") {
                    ResponseTemplate::new(200).set_body_json(json!({ "data": [] }))
                } else {
                    ResponseTemplate::new(200)
                        .set_body_json(json!({ "data": [{ "remaining": remaining }] }))
                }
            })
            .mount(server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/mark_account_deletion_e2ee_purged"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(true)))
            .mount(server)
            .await;
    }

    async fn mount_stripe_customer(server: &MockServer, metadata: Value, email: Option<&str>) {
        Mock::given(method("GET"))
            .and(path(format!("/v1/customers/{CUSTOMER}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": CUSTOMER,
                "object": "customer",
                "created": 1_700_000_000,
                "email": email,
                "livemode": false,
                "metadata": metadata
            })))
            .mount(server)
            .await;
    }

    async fn mount_shared_attachment_claim(server: &MockServer) {
        Mock::given(method("POST"))
            .and(path(
                "/rest/v1/rpc/claim_session_share_attachment_gc_leases",
            ))
            .respond_with(move |request: &Request| {
                let lease_id = request_lease(request);
                ResponseTemplate::new(200).set_body_json(json!([{
                    "attachment_id": OBJECT,
                    "owner_user_id": OWNER,
                    "share_id": SHARE,
                    "object_key": format!("{OWNER}/{SHARE}/{OBJECT}.sna1"),
                    "size_bytes": 1024,
                    "gc_lease_id": lease_id,
                    "gc_lease_expires_at": (Utc::now() + TimeDelta::minutes(5)).to_rfc3339()
                }]))
            })
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn deletes_storage_before_finishing_an_attachment_lease() {
        let server = MockServer::start().await;
        mount_attachment_claim(&server, false).await;
        Mock::given(method("DELETE"))
            .and(path(format!(
                "/storage/v1/object/{ATTACHMENT_BACKUP_BUCKET}/{OWNER}/{OBJECT}.anb1"
            )))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/finish_attachment_backup_deletion"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(true)))
            .mount(&server)
            .await;

        let count = worker(&server)
            .run_attachment_batch(&CancellationToken::new())
            .await
            .unwrap();

        assert_eq!(count, 1);
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 3);
        assert_eq!(
            requests[1].url.path(),
            format!("/storage/v1/object/{ATTACHMENT_BACKUP_BUCKET}/{OWNER}/{OBJECT}.anb1")
        );
        assert_eq!(
            requests[2].url.path(),
            "/rest/v1/rpc/finish_attachment_backup_deletion"
        );
    }

    #[tokio::test]
    async fn leaves_the_ledger_lease_when_storage_deletion_fails() {
        let server = MockServer::start().await;
        mount_attachment_claim(&server, false).await;
        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let count = worker(&server)
            .run_attachment_batch(&CancellationToken::new())
            .await
            .unwrap();

        assert_eq!(count, 1);
        assert!(
            server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .all(|request| {
                    request.url.path() != "/rest/v1/rpc/finish_attachment_backup_deletion"
                })
        );
    }

    #[tokio::test]
    async fn rejects_invalid_attachment_leases_before_touching_storage() {
        let server = MockServer::start().await;
        mount_attachment_claim(&server, true).await;

        let error = worker(&server)
            .run_attachment_batch(&CancellationToken::new())
            .await
            .unwrap_err();

        assert!(error.to_string().contains("Invalid attachment object key"));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn deletes_shared_storage_before_finishing_its_lease() {
        let server = MockServer::start().await;
        mount_shared_attachment_claim(&server).await;
        Mock::given(method("DELETE"))
            .and(path(format!(
                "/storage/v1/object/{SHARED_ATTACHMENT_BUCKET}/{OWNER}/{SHARE}/{OBJECT}.sna1"
            )))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path(
                "/rest/v1/rpc/finish_session_share_attachment_deletion",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(true)))
            .mount(&server)
            .await;

        let count = worker(&server)
            .run_shared_attachment_batch(&CancellationToken::new())
            .await
            .unwrap();

        assert_eq!(count, 1);
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 3);
        assert_eq!(
            requests[2].url.path(),
            "/rest/v1/rpc/finish_session_share_attachment_deletion"
        );
    }

    #[tokio::test]
    async fn accepts_the_maximum_bounded_account_claim_response() {
        let server = MockServer::start().await;
        let cancellation = CancellationToken::new();
        let cancel_after_claim = cancellation.clone();
        let workspace_ids = (1..=1_000)
            .map(|index| format!("00000000-0000-4000-8000-{index:012}"))
            .collect::<Vec<_>>();
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/claim_account_deletion_leases_v2"))
            .respond_with(move |request: &Request| {
                cancel_after_claim.cancel();
                let lease_id = request_lease(request);
                let row = json!({
                    "owner_user_id": OWNER,
                    "final_sweep_not_before":
                        (Utc::now() - TimeDelta::minutes(1)).to_rfc3339(),
                    "stripe_customer_id": null,
                    "stripe_deleted": true,
                    "cleanup_ready": true,
                    "prefix_swept": true,
                    "e2ee_workspace_ids": workspace_ids.clone(),
                    "e2ee_purged": true,
                    "lease_id": lease_id,
                    "lease_expires_at":
                        (Utc::now() + TimeDelta::minutes(15)).to_rfc3339()
                });
                ResponseTemplate::new(200).set_body_json(vec![
                    row.clone(),
                    row.clone(),
                    row.clone(),
                    row,
                ])
            })
            .mount(&server)
            .await;

        let count = worker(&server)
            .run_account_batch(&cancellation)
            .await
            .unwrap();

        assert_eq!(count, ACCOUNT_BATCH_SIZE);
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn sweeps_all_attachment_prefixes_before_deleting_the_auth_user() {
        let server = MockServer::start().await;
        mount_account_claim_with_stripe(&server, false, false, Some(CUSTOMER), false, false).await;
        mount_stripe_customer(&server, json!({ "userId": OWNER }), None).await;
        Mock::given(method("DELETE"))
            .and(path(format!("/v1/customers/{CUSTOMER}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": CUSTOMER,
                "object": "customer",
                "deleted": true
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/mark_account_deletion_stripe_deleted"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(true)))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path(format!(
                "/storage/v1/object/list/{ATTACHMENT_BACKUP_BUCKET}"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path(format!("/storage/v1/object/list/{AUDIO_BUCKET}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path(format!(
                "/storage/v1/object/list/{SHARED_ATTACHMENT_BUCKET}"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/mark_account_deletion_prefix_swept"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(true)))
            .mount(&server)
            .await;
        mount_cloudsync_cleanup(&server, 0).await;
        Mock::given(method("DELETE"))
            .and(path(format!("/auth/v1/admin/users/{OWNER}")))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/finish_account_deletion"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(true)))
            .mount(&server)
            .await;

        let count = worker(&server)
            .run_account_batch(&CancellationToken::new())
            .await
            .unwrap();

        assert_eq!(count, 1);
        let paths = server
            .received_requests()
            .await
            .unwrap()
            .into_iter()
            .map(|request| request.url.path().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                "/rest/v1/rpc/claim_account_deletion_leases_v2",
                &format!("/v1/customers/{CUSTOMER}"),
                &format!("/v1/customers/{CUSTOMER}"),
                "/rest/v1/rpc/mark_account_deletion_stripe_deleted",
                "/storage/v1/object/list/attachment-backups",
                "/storage/v1/object/list/audio-files",
                "/storage/v1/object/list/shared-note-attachments",
                "/rest/v1/rpc/mark_account_deletion_prefix_swept",
                "/v1/databases/managed-e2ee",
                "/v2/weblite/sql",
                "/v2/weblite/sql",
                "/rest/v1/rpc/mark_account_deletion_e2ee_purged",
                &format!("/auth/v1/admin/users/{OWNER}"),
                "/rest/v1/rpc/finish_account_deletion",
            ]
        );
        let sql = server
            .received_requests()
            .await
            .unwrap()
            .into_iter()
            .filter(|request| request.url.path() == "/v2/weblite/sql")
            .map(|request| {
                serde_json::from_slice::<Value>(&request.body).unwrap()["sql"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(sql.len(), 2);
        assert!(sql[0].starts_with("DELETE FROM e2ee_records"));
        assert!(sql[0].contains(OWNER));
        assert!(sql[0].contains(WORKSPACE));
        assert!(sql[1].starts_with("SELECT COUNT(*) AS remaining"));
    }

    #[tokio::test]
    async fn checkpoints_an_already_deleted_stripe_customer_before_the_cleanup_horizon() {
        let server = MockServer::start().await;
        mount_account_claim_with_stripe(&server, false, false, Some(CUSTOMER), false, true).await;
        mount_stripe_customer(&server, json!({ "user_id": OWNER }), None).await;
        Mock::given(method("DELETE"))
            .and(path(format!("/v1/customers/{CUSTOMER}")))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({
                "error": {
                    "message": "No such customer",
                    "type": "invalid_request_error"
                }
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/mark_account_deletion_stripe_deleted"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(true)))
            .mount(&server)
            .await;

        let count = worker(&server)
            .run_account_batch(&CancellationToken::new())
            .await
            .unwrap();

        assert_eq!(count, 1);
        let paths = server
            .received_requests()
            .await
            .unwrap()
            .into_iter()
            .map(|request| request.url.path().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                "/rest/v1/rpc/claim_account_deletion_leases_v2",
                &format!("/v1/customers/{CUSTOMER}"),
                &format!("/v1/customers/{CUSTOMER}"),
                "/rest/v1/rpc/mark_account_deletion_stripe_deleted",
            ]
        );
    }

    #[tokio::test]
    async fn checkpoints_a_missing_stripe_customer_without_an_external_request() {
        let server = MockServer::start().await;
        mount_account_claim_with_stripe(&server, false, false, None, false, true).await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/mark_account_deletion_stripe_deleted"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(true)))
            .mount(&server)
            .await;

        let count = worker(&server)
            .run_account_batch(&CancellationToken::new())
            .await
            .unwrap();

        assert_eq!(count, 1);
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests[1].url.path(),
            "/rest/v1/rpc/mark_account_deletion_stripe_deleted"
        );
        let checkpoint: Value = serde_json::from_slice(&requests[1].body).unwrap();
        assert!(checkpoint["p_stripe_customer_id"].is_null());
    }

    #[tokio::test]
    async fn leaves_stripe_and_auth_checkpoints_pending_when_stripe_fails() {
        let server = MockServer::start().await;
        mount_account_claim_with_stripe(&server, false, false, Some(CUSTOMER), false, true).await;
        mount_stripe_customer(&server, json!({ "userId": OWNER }), None).await;
        Mock::given(method("DELETE"))
            .and(path(format!("/v1/customers/{CUSTOMER}")))
            .respond_with(ResponseTemplate::new(503).set_body_json(json!({
                "error": {
                    "message": "Unavailable",
                    "type": "api_error"
                }
            })))
            .mount(&server)
            .await;

        let count = worker(&server)
            .run_account_batch(&CancellationToken::new())
            .await
            .unwrap();

        assert_eq!(count, 1);
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 3);
        assert!(requests.iter().all(|request| {
            request.url.path() != "/rest/v1/rpc/mark_account_deletion_stripe_deleted"
                && request.url.path() != format!("/auth/v1/admin/users/{OWNER}")
        }));
    }

    #[tokio::test]
    async fn rejects_a_stripe_customer_owned_by_another_user() {
        let server = MockServer::start().await;
        mount_account_claim_with_stripe(&server, false, false, Some(CUSTOMER), false, true).await;
        mount_stripe_customer(
            &server,
            json!({ "userId": "00000000-0000-4000-8000-000000000599" }),
            None,
        )
        .await;

        let count = worker(&server)
            .run_account_batch(&CancellationToken::new())
            .await
            .unwrap();

        assert_eq!(count, 1);
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests.iter().all(|request| {
            request.method.as_str() != "DELETE"
                && request.url.path() != "/rest/v1/rpc/mark_account_deletion_stripe_deleted"
        }));
    }

    #[tokio::test]
    async fn verifies_a_legacy_stripe_customer_by_the_auth_email() {
        let server = MockServer::start().await;
        mount_account_claim_with_stripe(&server, false, false, Some(CUSTOMER), false, true).await;
        mount_stripe_customer(&server, json!({}), Some("owner@example.com")).await;
        Mock::given(method("GET"))
            .and(path(format!("/auth/v1/admin/users/{OWNER}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": OWNER,
                "email": "OWNER@example.com"
            })))
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .and(path(format!("/v1/customers/{CUSTOMER}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": CUSTOMER,
                "object": "customer",
                "deleted": true
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/mark_account_deletion_stripe_deleted"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(true)))
            .mount(&server)
            .await;

        let count = worker(&server)
            .run_account_batch(&CancellationToken::new())
            .await
            .unwrap();

        assert_eq!(count, 1);
        let paths = server
            .received_requests()
            .await
            .unwrap()
            .into_iter()
            .map(|request| request.url.path().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                "/rest/v1/rpc/claim_account_deletion_leases_v2",
                &format!("/v1/customers/{CUSTOMER}"),
                &format!("/auth/v1/admin/users/{OWNER}"),
                &format!("/v1/customers/{CUSTOMER}"),
                "/rest/v1/rpc/mark_account_deletion_stripe_deleted",
            ]
        );
    }

    #[tokio::test]
    async fn treats_storage_sweep_cancellation_as_a_graceful_stop() {
        let server = MockServer::start().await;
        let cancellation = CancellationToken::new();
        let cancel_during_list = cancellation.clone();
        Mock::given(method("POST"))
            .and(path(format!(
                "/storage/v1/object/list/{ATTACHMENT_BACKUP_BUCKET}"
            )))
            .respond_with(move |_: &Request| {
                cancel_during_list.cancel();
                ResponseTemplate::new(200).set_body_json(json!([{
                    "name": format!("{OBJECT}.anb1"),
                    "id": OBJECT,
                    "updated_at": "2026-07-18T00:00:00Z",
                    "created_at": "2026-07-18T00:00:00Z",
                    "last_accessed_at": "2026-07-18T00:00:00Z",
                    "metadata": { "size": 1024 }
                }]))
            })
            .mount(&server)
            .await;

        let result = worker(&server)
            .delete_account(
                AccountLeaseRow {
                    owner_user_id: OWNER.to_string(),
                    final_sweep_not_before: (Utc::now() - TimeDelta::minutes(1)).to_rfc3339(),
                    stripe_customer_id: None,
                    stripe_deleted: true,
                    cleanup_ready: true,
                    prefix_swept: false,
                    e2ee_workspace_ids: vec![OWNER.to_string(), WORKSPACE.to_string()],
                    e2ee_purged: false,
                    lease_id: Uuid::now_v7().to_string(),
                    lease_expires_at: (Utc::now() + TimeDelta::minutes(15)).to_rfc3339(),
                },
                &cancellation,
            )
            .await;

        assert!(result.is_ok());
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].url.path(),
            "/storage/v1/object/list/attachment-backups"
        );
    }

    #[tokio::test]
    async fn leaves_auth_intact_when_cloudsync_purge_fails() {
        let server = MockServer::start().await;
        mount_account_claim(&server, true, false).await;
        Mock::given(method("GET"))
            .and(path("/v1/databases/managed-e2ee"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "managedDatabaseId": "managed-e2ee",
                    "databaseName": "e2ee.sqlite",
                    "projectId": "test-project"
                }
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v2/weblite/sql"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let count = worker(&server)
            .run_account_batch(&CancellationToken::new())
            .await
            .unwrap();

        assert_eq!(count, 1);
        let requests = server.received_requests().await.unwrap();
        assert!(requests.iter().all(|request| {
            request.url.path() != format!("/auth/v1/admin/users/{OWNER}")
                && request.url.path() != "/rest/v1/rpc/mark_account_deletion_e2ee_purged"
                && request.url.path() != "/rest/v1/rpc/finish_account_deletion"
        }));
    }

    #[tokio::test]
    async fn treats_an_already_deleted_auth_user_as_success() {
        let server = MockServer::start().await;
        mount_account_claim(&server, true, true).await;
        Mock::given(method("DELETE"))
            .and(path(format!("/auth/v1/admin/users/{OWNER}")))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/finish_account_deletion"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(true)))
            .mount(&server)
            .await;

        let count = worker(&server)
            .run_account_batch(&CancellationToken::new())
            .await
            .unwrap();

        assert_eq!(count, 1);
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 3);
        assert!(requests.iter().all(|request| {
            request.url.path() != "/v2/weblite/sql"
                && request.url.path() != "/rest/v1/rpc/mark_account_deletion_e2ee_purged"
        }));
    }
}
