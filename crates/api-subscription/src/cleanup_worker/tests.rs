use anlg_api_env::{LoopsEnv, StripeEnv, SupabaseEnv};
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

#[test]
fn full_batches_back_off_without_changing_idle_polling() {
    assert_eq!(full_batch_poll_delay(true), Some(Duration::from_secs(5)));
    assert_eq!(full_batch_poll_delay(false), None);
    assert_eq!(POLL_INTERVAL, Duration::from_secs(30));
}

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
        json!({
            "userId": OWNER,
            "user_id": "00000000-0000-4000-8000-000000000599"
        }),
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
    mount_stripe_customer(
        &server,
        json!({ "userId": "", "user_id": "" }),
        Some("owner@example.com"),
    )
    .await;
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
