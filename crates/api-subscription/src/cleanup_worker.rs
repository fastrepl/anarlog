use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};
use futures_util::{StreamExt, stream};
use serde::{Deserialize, Serialize};
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use uuid::{Uuid, Version};

use crate::{
    SubscriptionConfig,
    error::{Result, SubscriptionError},
    supabase::SupabaseClient,
};

const ATTACHMENT_BACKUP_BUCKET: &str = "attachment-backups";
const AUDIO_BUCKET: &str = "audio-files";
const ATTACHMENT_BATCH_SIZE: usize = 32;
const ATTACHMENT_CONCURRENCY: usize = 4;
const ATTACHMENT_LEASE_SECONDS: i32 = 300;
const ACCOUNT_BATCH_SIZE: usize = 4;
const ACCOUNT_LEASE_SECONDS: i32 = 900;
const POLL_INTERVAL: Duration = Duration::from_secs(30);
const MAX_ACCOUNT_PREFIX_OBJECTS: usize = 20_000;
const MAX_CIPHERTEXT_SIZE_BYTES: i64 = 545_259_520;

#[derive(Clone)]
pub struct CleanupWorker {
    supabase: SupabaseClient,
    storage: hypr_supabase_storage::SupabaseStorage,
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
struct AccountLeaseRow {
    owner_user_id: String,
    final_sweep_not_before: String,
    prefix_swept: bool,
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
struct AccountLeaseRequest<'a> {
    p_owner_user_id: &'a str,
    p_lease_id: &'a str,
}

impl CleanupWorker {
    pub fn new(config: &SubscriptionConfig) -> Self {
        let supabase = SupabaseClient::new(
            config.supabase.supabase_url.clone(),
            config.supabase.supabase_anon_key.clone(),
            config.supabase.supabase_service_role_key.clone(),
        );
        let storage = supabase.storage();
        Self { supabase, storage }
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
        let account_count = match self.run_account_batch(cancellation).await {
            Ok(count) => count,
            Err(error) => {
                tracing::warn!(error = %error, "account_deletion_batch_failed");
                0
            }
        };
        attachment_count == ATTACHMENT_BATCH_SIZE || account_count == ACCOUNT_BATCH_SIZE
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

    async fn run_account_batch(&self, cancellation: &CancellationToken) -> Result<usize> {
        if cancellation.is_cancelled() {
            return Ok(0);
        }
        let lease_id = Uuid::now_v7().to_string();
        let rows: Vec<AccountLeaseRow> = self
            .supabase
            .admin_rpc(
                "claim_account_deletion_leases",
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
        if !row.prefix_swept {
            let prefix = format!("{}/", row.owner_user_id);
            self.storage
                .clear_prefix_until(
                    ATTACHMENT_BACKUP_BUCKET,
                    &prefix,
                    MAX_ACCOUNT_PREFIX_OBJECTS,
                    || cancellation.is_cancelled(),
                )
                .await
                .map_err(storage_error)?;
            if cancellation.is_cancelled() {
                return Ok(());
            }
            self.storage
                .clear_prefix_until(AUDIO_BUCKET, &prefix, MAX_ACCOUNT_PREFIX_OBJECTS, || {
                    cancellation.is_cancelled()
                })
                .await
                .map_err(storage_error)?;
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
    canonical_uuid(&row.owner_user_id, None)?;
    let lease_id = canonical_uuid(&row.lease_id, Some(Version::SortRand))?;
    let horizon = parse_timestamp(&row.final_sweep_not_before)?;
    let lease_expires_at = validate_lease_expiry(&row.lease_expires_at)?;
    let now = Utc::now();
    if lease_id != expected_lease_id || horizon > now || lease_expires_at <= now {
        return Err(invalid_upstream("account deletion lease"));
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

    fn worker(server: &MockServer) -> CleanupWorker {
        CleanupWorker::new(&SubscriptionConfig::new(
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
        ))
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

    async fn mount_account_claim(server: &MockServer, prefix_swept: bool) {
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/claim_account_deletion_leases"))
            .respond_with(move |request: &Request| {
                let lease_id = request_lease(request);
                ResponseTemplate::new(200).set_body_json(json!([{
                    "owner_user_id": OWNER,
                    "final_sweep_not_before": (Utc::now() - TimeDelta::minutes(1)).to_rfc3339(),
                    "prefix_swept": prefix_swept,
                    "lease_id": lease_id,
                    "lease_expires_at": (Utc::now() + TimeDelta::minutes(15)).to_rfc3339()
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
    async fn sweeps_both_prefixes_before_deleting_the_auth_user() {
        let server = MockServer::start().await;
        mount_account_claim(&server, false).await;
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
            .and(path("/rest/v1/rpc/mark_account_deletion_prefix_swept"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(true)))
            .mount(&server)
            .await;
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
                "/rest/v1/rpc/claim_account_deletion_leases",
                "/storage/v1/object/list/attachment-backups",
                "/storage/v1/object/list/audio-files",
                "/rest/v1/rpc/mark_account_deletion_prefix_swept",
                &format!("/auth/v1/admin/users/{OWNER}"),
                "/rest/v1/rpc/finish_account_deletion",
            ]
        );
    }

    #[tokio::test]
    async fn treats_an_already_deleted_auth_user_as_success() {
        let server = MockServer::start().await;
        mount_account_claim(&server, true).await;
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
        assert_eq!(server.received_requests().await.unwrap().len(), 3);
    }
}
