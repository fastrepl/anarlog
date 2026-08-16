use std::sync::Arc;

use anarlog_enterprise_control_plane::{
    api::{AppState, router},
    auth::StaticTokenAuthenticator,
    serve,
    store::{DeliveryStore, StoreError},
};
use anlg_session_ingest::{AcknowledgeRequest, DeliveryPage, SessionRead};
use async_trait::async_trait;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use serde_json::Value;
use tokio::{net::TcpListener, sync::oneshot, time::Duration};
use tower::ServiceExt;

const TOKEN_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const TOKEN_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

struct MemoryStore {
    ready: bool,
}

#[async_trait]
impl DeliveryStore for MemoryStore {
    async fn readiness(&self) -> Result<(), StoreError> {
        if self.ready {
            Ok(())
        } else {
            Err(StoreError::CorruptEnvelope)
        }
    }

    async fn list_deliveries(
        &self,
        _workspace_id: &str,
        _consumer_id: &str,
        after: u64,
        _limit: u16,
    ) -> Result<DeliveryPage, StoreError> {
        Ok(DeliveryPage {
            items: vec![],
            next_cursor: after,
            has_more: false,
        })
    }

    async fn acknowledge(
        &self,
        _workspace_id: &str,
        _job_id: &str,
        _request: &AcknowledgeRequest,
    ) -> Result<(), StoreError> {
        Ok(())
    }

    async fn read_session(
        &self,
        _workspace_id: &str,
        _job_id: &str,
    ) -> Result<SessionRead, StoreError> {
        Err(StoreError::NotFound)
    }
}

fn state(ready: bool) -> AppState {
    let authenticator = StaticTokenAuthenticator::new([
        ("workspace-a".into(), TOKEN_A.into()),
        ("workspace-b".into(), TOKEN_B.into()),
    ])
    .unwrap();
    AppState::new(Arc::new(MemoryStore { ready }), Arc::new(authenticator))
}

#[tokio::test]
async fn exposes_database_independent_liveness_and_fail_closed_readiness() {
    let app = router(state(false));

    let live = app
        .clone()
        .oneshot(Request::get("/health/live").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let ready = app
        .oneshot(Request::get("/health/ready").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(live.status(), StatusCode::OK);
    assert_eq!(ready.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn requires_workspace_scoped_credentials() {
    let app = router(state(true));
    let path = "/v1/workspaces/workspace-a/session-envelopes?consumerId=device-a&after=0";

    let missing = app
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let wrong_workspace = app
        .clone()
        .oneshot(authorized_request(path, TOKEN_B))
        .await
        .unwrap();
    let authorized = app
        .oneshot(authorized_request(path, TOKEN_A))
        .await
        .unwrap();

    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(missing.headers()[header::WWW_AUTHENTICATE], "Bearer");
    assert_eq!(wrong_workspace.status(), StatusCode::FORBIDDEN);
    assert_eq!(authorized.status(), StatusCode::OK);
    assert_eq!(
        response_json(authorized).await,
        serde_json::json!({
            "items": [],
            "nextCursor": 0,
            "hasMore": false,
        })
    );
}

#[tokio::test]
async fn boots_on_a_real_listener_and_shuts_down_gracefully() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server = tokio::spawn(serve(listener, state(true), async move {
        shutdown_rx.await.ok();
    }));

    let response = reqwest::get(format!("http://{address}/health/live"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    shutdown_tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .expect("server did not stop")
        .unwrap()
        .unwrap();
}

fn authorized_request(path: &str, token: &str) -> Request<Body> {
    Request::get(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}
