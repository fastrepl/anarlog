use std::time::{SystemTime, UNIX_EPOCH};

use anarlog_enterprise_control_plane::{api::router, config::Config, configured_state};
use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use serde_json::Value;
use tower::ServiceExt;

const TEST_DATABASE_URL_ENV: &str = "ANARLOG_ENTERPRISE_TEST_DATABASE_URL";
const TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[tokio::test]
async fn migrates_postgres_and_enforces_workspace_delivery() {
    let Ok(database_url) = std::env::var(TEST_DATABASE_URL_ENV) else {
        return;
    };
    let suffix = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let workspace_id = format!("workspace-{suffix}");
    let job_id = format!("job-{suffix}");
    let config = Config::from_values(
        database_url.clone(),
        Some("127.0.0.1:0".into()),
        Some("2".into()),
        format!(r#"{{"{workspace_id}":"{TOKEN}"}}"#),
    )
    .unwrap();
    let state = configured_state(&config).await.unwrap();
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let envelope = serde_json::json!({
        "schema_version": 1,
        "source_id": job_id,
        "revision": 1,
        "finalized": true,
        "workspace_id": workspace_id,
        "owner_user_id": "owner-a",
        "session": {
            "id": format!("session-{suffix}"),
            "title": "Smoke test",
            "status": "completed",
            "created_at": "2026-08-17T00:00:00Z",
            "updated_at": "2026-08-17T00:00:00Z"
        }
    });
    sqlx::query(
        r#"
        INSERT INTO session_envelopes (
            workspace_id,
            job_id,
            revision,
            finalized,
            content_hash,
            envelope
        ) VALUES ($1, $2, 1, TRUE, $3, $4)
        "#,
    )
    .bind(&workspace_id)
    .bind(&job_id)
    .bind("0000000000000000000000000000000000000000000000000000000000000000")
    .bind(envelope)
    .execute(&pool)
    .await
    .unwrap();

    let app = router(state);
    let list_path =
        format!("/v1/workspaces/{workspace_id}/session-envelopes?consumerId=device-a&after=0");
    let listed = app
        .clone()
        .oneshot(authorized_request(Method::GET, &list_path, Body::empty()))
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let listed = response_json(listed).await;
    assert_eq!(listed["items"][0]["jobId"], job_id);
    assert_eq!(listed["items"][0]["acknowledged"], false);

    let acknowledge_path = format!("/v1/workspaces/{workspace_id}/session-envelopes/{job_id}/ack");
    let acknowledged = app
        .clone()
        .oneshot(authorized_request(
            Method::POST,
            &acknowledge_path,
            Body::from(
                serde_json::json!({
                    "consumerId": "device-a",
                    "revision": 1,
                    "contentHash": "0000000000000000000000000000000000000000000000000000000000000000"
                })
                .to_string(),
            ),
        ))
        .await
        .unwrap();
    assert_eq!(acknowledged.status(), StatusCode::OK);

    let wrong_workspace = app
        .oneshot(authorized_request(
            Method::GET,
            "/v1/workspaces/other-workspace/session-envelopes?consumerId=device-a&after=0",
            Body::empty(),
        ))
        .await
        .unwrap();
    assert_eq!(wrong_workspace.status(), StatusCode::FORBIDDEN);
}

fn authorized_request(method: Method, path: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(body)
        .unwrap()
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}
