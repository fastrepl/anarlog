//! Live checks against a running sync API and its Supabase replica. Deployment
//! verification runs them with dedicated accounts; they are ignored otherwise.

use std::collections::HashMap;

use db_sync::{E2eeWitnessCancellation, E2eeWitnessClient, E2eeWitnessConfig};

const KEY_ID_HEADER: &str = "x-anarlog-e2ee-key-id";
const TRANSPORTS_HEADER: &str = "x-anarlog-cloudsync-transports";

fn required_env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} must be set"))
}

async fn fresh_device() -> anlg_db_core::Db {
    let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    db
}

async fn status_of(request: reqwest::RequestBuilder) -> u16 {
    request.send().await.unwrap().status().as_u16()
}

#[tokio::test]
#[ignore = "live replica verification run by deploys (ANLG-554); requires ANARLOG_REPLICA_API_URL, ANARLOG_REPLICA_ACCESS_TOKEN_A/B, and ANARLOG_REPLICA_RECOVERY_KEY_A"]
async fn fresh_devices_converge_through_the_replica_and_other_accounts_stay_out() {
    let api = required_env("ANARLOG_REPLICA_API_URL");
    let api = api.trim_end_matches('/');
    let token_a = required_env("ANARLOG_REPLICA_ACCESS_TOKEN_A");
    let token_b = required_env("ANARLOG_REPLICA_ACCESS_TOKEN_B");
    let recovery =
        anlg_e2ee::RecoveryKey::parse(&required_env("ANARLOG_REPLICA_RECOVERY_KEY_A")).unwrap();
    let http = reqwest::Client::new();

    let credentials: serde_json::Value = http
        .post(format!("{api}/sync/replica/credentials"))
        .bearer_auth(&token_a)
        .header(KEY_ID_HEADER, recovery.key_id())
        .header(TRANSPORTS_HEADER, "replica, accepted_replica")
        .send()
        .await
        .unwrap()
        .error_for_status()
        .expect("replica credentials were refused")
        .json()
        .await
        .unwrap();
    assert_eq!(credentials["transport"], "replica");
    let workspace = credentials["personalWorkspaceId"]
        .as_str()
        .expect("replica credentials name the personal workspace")
        .to_string();

    // The compatibility endpoint must issue only replica credentials and turn
    // away clients that do not advertise `accepted_replica`.
    let token_request = || {
        http.post(format!("{api}/sync/token"))
            .bearer_auth(&token_a)
            .header(KEY_ID_HEADER, recovery.key_id())
    };
    assert_eq!(status_of(token_request()).await, 426);
    let upgraded: serde_json::Value = token_request()
        .header(TRANSPORTS_HEADER, "replica, accepted_replica")
        .send()
        .await
        .unwrap()
        .error_for_status()
        .expect("an accepted-replica client was refused credentials")
        .json()
        .await
        .unwrap();
    assert_eq!(upgraded["transport"], "replica");
    assert!(upgraded.get("token").is_none());

    // Builds that predate the accepted replica are refused on the replica route
    // too, so none keeps syncing after SQLite Cloud's retirement.
    let old_client = http
        .post(format!("{api}/sync/replica/credentials"))
        .bearer_auth(&token_a)
        .header(KEY_ID_HEADER, recovery.key_id())
        .header(TRANSPORTS_HEADER, "replica");
    assert_eq!(status_of(old_client).await, 426);

    let key = recovery.workspace_key(&workspace).unwrap();
    let keys = HashMap::from([(workspace.clone(), key.clone().into())]);
    let endpoint = format!("{api}/sync/e2ee/witness/{workspace}");
    let client = |token: &str| {
        E2eeWitnessClient::new(
            E2eeWitnessConfig {
                endpoint: endpoint.clone(),
                access_token: token.to_string(),
            },
            &workspace,
        )
        .unwrap()
    };

    let marker = format!(
        "deploy-verification-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let writer = fresh_device().await;
    sqlx::query("INSERT INTO sessions(id, workspace_id, owner_user_id, title) VALUES(?, ?, ?, ?)")
        .bind(&marker)
        .bind(&workspace)
        .bind(&workspace)
        .bind(&marker)
        .execute(writer.pool())
        .await
        .unwrap();
    anlg_db_app::encrypt_e2ee_replica_changes(writer.pool(), &keys)
        .await
        .unwrap();
    client(&token_a)
        .initialize_cancellable(writer.pool(), &key, &E2eeWitnessCancellation::default())
        .await
        .expect("the writing device could not publish through the replica");
    assert!(
        anlg_db_app::pending_e2ee_cloud_batch(writer.pool(), &workspace, false)
            .await
            .unwrap()
            .is_none(),
        "the accepted edit is still pending on the writing device"
    );

    let reader = fresh_device().await;
    client(&token_a)
        .initialize_cancellable(reader.pool(), &key, &E2eeWitnessCancellation::default())
        .await
        .expect("a fresh device could not recover from the replica");
    // Each pass applies a bounded batch; the workspace keeps every earlier run.
    while anlg_db_app::apply_received_e2ee_replica_changes_with_witness(reader.pool(), &keys, true)
        .await
        .unwrap()
        .remaining_replica_changes
    {}
    let recovered: Option<String> = sqlx::query_scalar("SELECT title FROM sessions WHERE id = ?")
        .bind(&marker)
        .fetch_optional(reader.pool())
        .await
        .unwrap();
    assert_eq!(recovered.as_deref(), Some(marker.as_str()));

    for path in ["", "/accepted"] {
        let status = status_of(http.get(format!("{endpoint}{path}")).bearer_auth(&token_b)).await;
        assert!(
            matches!(status, 403 | 404),
            "another account read the workspace log{path}: {status}"
        );
    }
    let outsider = fresh_device().await;
    assert!(
        client(&token_b)
            .initialize_cancellable(outsider.pool(), &key, &E2eeWitnessCancellation::default())
            .await
            .is_err(),
        "another account synchronized with the workspace"
    );
}
