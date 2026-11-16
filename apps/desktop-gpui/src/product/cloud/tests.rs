use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use desktop_runtime::{Profile, RuntimeHandle, ServiceError, SessionId};
use futures::future::BoxFuture;
use serde_json::{Value, json};
use tokio::sync::Mutex;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_json, body_partial_json, method, path, query_param},
};
use zeroize::Zeroizing;

use super::{
    CloudHost, CloudServices, ConflictDecision,
    auth::{Auth, SecureStore},
    sharing::validate_document,
    transport::{CloudConfig, Transport},
};
use crate::product::services::{Action, Mutation, ProductServices, RequestGate, Scope, Surface};

const ACCOUNT: &str = "180dc97e-633a-4766-9184-799f89db530b";
const SHARE: &str = "73d1e15a-9117-4260-8c78-5081a0c5012e";

#[tokio::test]
async fn shared_note_host_uses_account_rpc_and_anonymous_handoff_contracts() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("shared.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let cloud = fixture_cloud(&runtime, &server, Arc::new(MemoryStore::default())).await;
    let body = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"日本語 🧑🏽‍💻"}]}]});
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/read_my_session_share_snapshot_v2"))
        .and(body_json(json!({"p_share_id":SHARE})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!([{"share_id":SHARE,"title":"Shared","body_json":body}])),
        )
        .expect(1)
        .mount(&server)
        .await;
    cloud
        .read_shared(
            SHARE.into(),
            false,
            desktop_runtime::CancellationToken::new(),
        )
        .await
        .unwrap();
    Mock::given(method("POST"))
        .and(path("/shared-notes/handoffs/claim"))
        .and(body_partial_json(json!({"request_id":SHARE})))
        .respond_with(ResponseTemplate::new(200).set_body_json(snapshot(1, body)))
        .expect(1)
        .mount(&server)
        .await;
    cloud
        .read_shared(
            SHARE.into(),
            true,
            desktop_runtime::CancellationToken::new(),
        )
        .await
        .unwrap();
    let cancelled = desktop_runtime::CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        cloud.read_shared(SHARE.into(), true, cancelled).await,
        Err(ServiceError::Cancelled)
    ));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn calendar_disconnect_requires_confirmed_authenticated_api_response() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("calendar.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let cloud = fixture_cloud(&runtime, &server, Arc::new(MemoryStore::default())).await;
    Mock::given(method("DELETE"))
        .and(path("/nango/connections"))
        .and(body_json(
            json!({"integration_id":"google-calendar","connection_id":"fixture"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"ok"})))
        .expect(1)
        .mount(&server)
        .await;
    crate::product::local::calendar::CalendarAuth::disconnect(
        &cloud,
        anlg_calendar::CalendarProviderType::Google,
        "fixture".into(),
        desktop_runtime::CancellationToken::new(),
    )
    .await
    .unwrap();
    runtime.shutdown().await.unwrap();
}

#[derive(Default)]
struct FixtureHost;

impl CloudHost for FixtureHost {
    fn open_url(&self, _: reqwest::Url) -> BoxFuture<'static, desktop_runtime::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn copy_text(&self, _: Zeroizing<String>) -> BoxFuture<'static, desktop_runtime::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn export_recovery(
        &self,
        _: Zeroizing<String>,
    ) -> BoxFuture<'static, desktop_runtime::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn flush_editor(&self, _: SessionId) -> BoxFuture<'static, desktop_runtime::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

async fn fixture_cloud(
    runtime: &RuntimeHandle,
    server: &MockServer,
    store: Arc<MemoryStore>,
) -> CloudServices {
    store
        .write(
            "fixture:session".into(),
            Zeroizing::new(session(now() + 3600, "fixture-token").to_string()),
        )
        .await
        .unwrap();
    CloudServices::new(
        runtime.clone(),
        config(server, "fixture"),
        store,
        Arc::new(FixtureHost),
    )
    .await
    .unwrap()
}

async fn seed_summary(runtime: &RuntimeHandle, body: Value) -> Scope {
    let note = runtime
        .create_note("日本語 😀".into())
        .unwrap()
        .receive()
        .await
        .unwrap();
    let session = note.summary.id;
    let id = session.clone();
    runtime.service(move |services| async move {
        sqlx::query("INSERT INTO workspaces (id, owner_user_id) VALUES (?, ?)").bind(ACCOUNT).bind(ACCOUNT).execute(services.db.pool()).await.unwrap();
        sqlx::query("UPDATE sessions SET workspace_id = ?, owner_user_id = ? WHERE id = ?").bind(ACCOUNT).bind(ACCOUNT).bind(id.0.as_ref()).execute(services.db.pool()).await.unwrap();
        sqlx::query("INSERT INTO session_documents (id, session_id, workspace_id, kind, body) VALUES (?, ?, ?, 'summary', ?)").bind(uuid::Uuid::new_v4().to_string()).bind(id.0.as_ref()).bind(ACCOUNT).bind(body.to_string()).execute(services.db.pool()).await.unwrap();
        Ok(())
    }).unwrap().receive().await.unwrap();
    Scope {
        account_id: Some(ACCOUNT.into()),
        session_id: Some(session),
        ..Scope::default()
    }
}

fn snapshot(revision: u64, body: Value) -> Value {
    json!({"shareId": SHARE, "schemaVersion": 1, "contentRevision": revision, "accessVersion": 1, "publishedAt": "2026-01-01T00:00:00Z", "title": "Web snapshot", "body": body, "attachments": [], "webEditable": false})
}

async fn create_share_rpc(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/create_session_share"))
        .and(body_partial_json(json!({"p_workspace_id": ACCOUNT})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"share_id": SHARE}])))
        .expect(1)
        .mount(server)
        .await;
}

async fn share_action(
    cloud: &CloudServices,
    scope: &Scope,
    action: Action,
) -> desktop_runtime::Result<crate::product::services::Outcome> {
    cloud
        .perform(
            Surface::Sharing,
            RequestGate::default().next(scope.clone()),
            Mutation {
                operation: super::panels::operation(action, "Fixture operation"),
                fields: Arc::from([]),
            },
        )
        .await
}

#[tokio::test]
async fn publication_conflicts_preserve_drafts_and_require_explicit_review_after_reopen() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let profile = Profile {
        database: directory.path().join("cloud.sqlite"),
    };
    let (runtime, ready) = RuntimeHandle::start(profile.clone()).unwrap();
    ready.receive().await.unwrap();
    let store = Arc::new(MemoryStore::default());
    let cloud = fixture_cloud(&runtime, &server, store.clone()).await;
    let document =
        json!({"type":"doc","content":[{"type":"futureWidget","attrs":{"opaque":"秘密 🧑🏽‍💻"}}]});
    let scope = seed_summary(&runtime, document.clone()).await;
    create_share_rpc(&server).await;
    Mock::given(method("PUT")).and(path(format!("/sync/shares/{SHARE}/snapshot")))
        .and(body_partial_json(json!({"baseRevision":0,"body":document,"attachmentIds":[]})))
        .respond_with(ResponseTemplate::new(409).set_body_json(json!({"code":"snapshot_conflict","snapshot":snapshot(5,json!({"type":"doc","content":[]}))})))
        .expect(1).mount(&server).await;
    assert!(matches!(
        share_action(&cloud, &scope, Action::CreateShare).await,
        Err(ServiceError::Conflict)
    ));
    runtime.shutdown().await.unwrap();
    let (runtime, ready) = RuntimeHandle::start(profile).unwrap();
    ready.receive().await.unwrap();
    let restored = fixture_cloud(&runtime, &server, store).await;
    let review = restored
        .conflict_review(scope.clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(review.remote_revision, 5);
    assert!(
        review
            .local_preview
            .iter()
            .any(|line| line.contains("秘密"))
    );
    assert!(matches!(
        restored
            .resolve_conflict(scope.clone(), 4, ConflictDecision::KeepRemote)
            .await,
        Err(ServiceError::Conflict)
    ));
    restored
        .resolve_conflict(scope.clone(), 5, ConflictDecision::KeepRemote)
        .await
        .unwrap();
    assert!(
        restored
            .conflict_review(scope.clone())
            .await
            .unwrap()
            .is_none()
    );
    let id = scope.session_id.clone().unwrap();
    runtime.service(move |services| async move {
        let raw: String = sqlx::query_scalar("SELECT body FROM session_documents WHERE session_id = ? AND kind = 'summary'").bind(id.0.as_ref()).fetch_one(services.db.pool()).await.unwrap();
        assert_eq!(serde_json::from_str::<Value>(&raw).unwrap(), document);
        let revision: i64 = sqlx::query_scalar("SELECT content_revision FROM shared_session_cache WHERE viewer_user_id = ? AND share_id = ?").bind(ACCOUNT).bind(SHARE).fetch_one(services.db.pool()).await.unwrap();
        assert_eq!(revision, 5);
        Ok(())
    }).unwrap().receive().await.unwrap();
    let mut wrong = scope;
    wrong.account_id = Some("different-account".into());
    assert!(matches!(
        restored.conflict_review(wrong).await,
        Err(ServiceError::Conflict)
    ));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn failed_publication_retries_the_identical_durable_mutation() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let profile = Profile {
        database: directory.path().join("cloud.sqlite"),
    };
    let (runtime, ready) = RuntimeHandle::start(profile.clone()).unwrap();
    ready.receive().await.unwrap();
    let store = Arc::new(MemoryStore::default());
    let cloud = fixture_cloud(&runtime, &server, store.clone()).await;
    let document = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Keep my draft"}]}]});
    let scope = seed_summary(&runtime, document.clone()).await;
    create_share_rpc(&server).await;
    let failed = Mock::given(method("PUT"))
        .and(path(format!("/sync/shares/{SHARE}/snapshot")))
        .respond_with(ResponseTemplate::new(503).set_body_json(json!({})))
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    assert!(
        share_action(&cloud, &scope, Action::CreateShare)
            .await
            .is_err()
    );
    drop(failed);
    let requests = server.received_requests().await.unwrap();
    let pending: Value = requests
        .iter()
        .find(|request| request.method == "PUT")
        .unwrap()
        .body_json()
        .unwrap();
    runtime.shutdown().await.unwrap();
    let (runtime, ready) = RuntimeHandle::start(profile).unwrap();
    ready.receive().await.unwrap();
    let restored = fixture_cloud(&runtime, &server, store).await;
    Mock::given(method("PUT"))
        .and(path(format!("/sync/shares/{SHARE}/snapshot")))
        .and(body_json(pending))
        .respond_with(ResponseTemplate::new(200).set_body_json(snapshot(1, document)))
        .expect(1)
        .mount(&server)
        .await;
    share_action(&restored, &scope, Action::PublishShare)
        .await
        .unwrap();
    runtime.shutdown().await.unwrap();
}

#[derive(Clone, Default)]
struct MemoryStore(Arc<Mutex<HashMap<String, Zeroizing<String>>>>);

impl SecureStore for MemoryStore {
    fn read(
        &self,
        key: String,
    ) -> BoxFuture<'static, desktop_runtime::Result<Option<Zeroizing<String>>>> {
        let this = self.clone();
        Box::pin(async move { Ok(this.0.lock().await.get(&key).cloned()) })
    }
    fn write(
        &self,
        key: String,
        value: Zeroizing<String>,
    ) -> BoxFuture<'static, desktop_runtime::Result<()>> {
        let this = self.clone();
        Box::pin(async move {
            this.0.lock().await.insert(key, value);
            Ok(())
        })
    }
    fn remove(&self, key: String) -> BoxFuture<'static, desktop_runtime::Result<()>> {
        let this = self.clone();
        Box::pin(async move {
            this.0.lock().await.remove(&key);
            Ok(())
        })
    }
}

fn config(server: &MockServer, namespace: &str) -> CloudConfig {
    CloudConfig {
        supabase: server.uri().parse().unwrap(),
        api: server.uri().parse().unwrap(),
        web: server.uri().parse().unwrap(),
        anon_key: "fixture-anon".into(),
        callback: "anarlog-dev://auth/callback".parse().unwrap(),
        credential_namespace: namespace.into(),
        device_fingerprint: "fixture-device".into(),
        device_name: "Fixture device".into(),
    }
}

fn session(expiry: u64, token: &str) -> Value {
    json!({"access_token": token, "refresh_token": "fixture-refresh", "token_type": "bearer",
        "expires_at": expiry, "expires_in": 3600,
        "user": {"id": ACCOUNT, "email": "fixture@example.invalid", "is_anonymous": false,
            "aud": "authenticated", "role": "authenticated", "app_metadata": {}, "user_metadata": {},
            "created_at": "2026-01-01T00:00:00Z"}})
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[tokio::test]
async fn persisted_sessions_are_isolated_and_logout_invalidates_leases() {
    let server = MockServer::start().await;
    let store = Arc::new(MemoryStore::default());
    store
        .write(
            "first:session".into(),
            Zeroizing::new(session(now() + 3600, "fixture-token").to_string()),
        )
        .await
        .unwrap();
    let first = Auth::new(
        Transport::new(config(&server, "first")).unwrap(),
        store.clone(),
    )
    .await
    .unwrap();
    let other = Auth::new(
        Transport::new(config(&server, "other")).unwrap(),
        store.clone(),
    )
    .await
    .unwrap();
    assert!(other.identity().account_id.is_none());
    assert!(matches!(
        first.lease("different-account", false).await,
        Err(ServiceError::Conflict)
    ));
    let lease = first.lease(ACCOUNT, false).await.unwrap();
    assert!(!format!("{lease:?}").contains("fixture-token"));
    let mut identities = first.identities();
    Mock::given(method("POST"))
        .and(path("/auth/v1/logout"))
        .and(query_param("scope", "local"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount(&server)
        .await;
    first.sign_out().await.unwrap();
    identities.changed().await.unwrap();
    assert!(identities.borrow().account_id.is_none());
    assert!(matches!(lease.check(), Err(ServiceError::Cancelled)));
    assert!(store.read("first:session".into()).await.unwrap().is_none());
}

#[tokio::test]
async fn concurrent_expired_leases_refresh_once_and_persist_rotation() {
    let server = MockServer::start().await;
    let store = Arc::new(MemoryStore::default());
    store
        .write(
            "test:session".into(),
            Zeroizing::new(session(1, "expired-token").to_string()),
        )
        .await
        .unwrap();
    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .and(body_json(json!({"refresh_token": "fixture-refresh"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(session(now() + 3600, "renewed-token")),
        )
        .expect(1)
        .mount(&server)
        .await;
    let auth = Auth::new(
        Transport::new(config(&server, "test")).unwrap(),
        store.clone(),
    )
    .await
    .unwrap();
    let (a, b) = tokio::join!(auth.lease(ACCOUNT, false), auth.lease(ACCOUNT, false));
    assert_eq!(a.unwrap().access_token().unwrap(), "renewed-token");
    assert_eq!(b.unwrap().access_token().unwrap(), "renewed-token");
    let restored = Auth::new(Transport::new(config(&server, "test")).unwrap(), store)
        .await
        .unwrap();
    assert_eq!(
        restored
            .lease(ACCOUNT, false)
            .await
            .unwrap()
            .access_token()
            .unwrap(),
        "renewed-token"
    );
}

#[tokio::test]
async fn pkce_checks_state_before_exchange_and_rejects_replayed_callback() {
    let server = MockServer::start().await;
    let auth = Auth::new(
        Transport::new(config(&server, "pkce")).unwrap(),
        Arc::new(MemoryStore::default()),
    )
    .await
    .unwrap();
    let login = auth.begin_login("google").await.unwrap();
    let redirect = login
        .query_pairs()
        .find(|(key, _)| key == "redirect_to")
        .unwrap()
        .1
        .into_owned();
    let mut bad: reqwest::Url = "anarlog-dev://auth/callback?state=wrong&code=fixture-code"
        .parse()
        .unwrap();
    assert!(auth.deep_link(bad.clone()).await.is_err());
    assert!(server.received_requests().await.unwrap().is_empty());
    bad = redirect.parse().unwrap();
    bad.query_pairs_mut().append_pair("code", "fixture-code");
    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "pkce"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(session(now() + 3600, "fixture-token")),
        )
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(
        auth.deep_link(bad.clone())
            .await
            .unwrap()
            .account_id
            .as_deref(),
        Some(ACCOUNT)
    );
    assert!(auth.deep_link(bad).await.is_err());
}

#[tokio::test]
async fn sync_preferences_and_recovery_keys_survive_reconstruction() {
    let server = MockServer::start().await;
    let store = Arc::new(MemoryStore::default());
    let auth = Auth::new(
        Transport::new(config(&server, "sync")).unwrap(),
        store.clone(),
    )
    .await
    .unwrap();
    let sync = super::sync::Sync::default();
    assert!(!sync.enabled(&auth, ACCOUNT).await.unwrap());
    store
        .write(
            auth.key(&format!("{ACCOUNT}:sync-enabled")),
            Zeroizing::new("true".into()),
        )
        .await
        .unwrap();
    let recovery = anlg_e2ee::RecoveryKey::generate().unwrap();
    store
        .write(
            auth.key(&format!("{ACCOUNT}:recovery")),
            recovery.expose_code(),
        )
        .await
        .unwrap();
    let restored = Auth::new(Transport::new(config(&server, "sync")).unwrap(), store)
        .await
        .unwrap();
    assert!(sync.enabled(&restored, ACCOUNT).await.unwrap());
    assert_eq!(
        sync.recovery(&restored, ACCOUNT).await.unwrap().key_id(),
        recovery.key_id()
    );
    assert!(sync.recovery(&restored, "another-account").await.is_err());
}

#[test]
fn publication_validation_preserves_unicode_and_unknown_nodes() {
    let document = json!({"type": "doc", "content": [
        {"type": "futureWidget", "attrs": {"opaque": "秘密"}},
        {"type": "paragraph", "content": [{"type": "text", "text": "こんにちは 🧑🏽‍💻"}]}
    ]});
    let original = document.clone();
    validate_document(&document).unwrap();
    assert_eq!(document, original);
    assert!(validate_document(&json!({"type": "doc", "content": "invalid"})).is_err());
    let mut nested = json!({"type": "paragraph"});
    for _ in 0..65 {
        nested = json!({"type": "doc", "content": [nested]});
    }
    assert!(validate_document(&nested).is_err());
}

#[tokio::test]
async fn transport_does_not_follow_redirects_with_credentials() {
    let first = MockServer::start().await;
    let second = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(302).insert_header("Location", second.uri()))
        .mount(&first)
        .await;
    let transport = Transport::new(config(&first, "transport")).unwrap();
    let response = transport
        .send(
            reqwest::Method::GET,
            first.uri().parse().unwrap(),
            Some("fixture-token"),
            None,
            &[],
        )
        .await
        .unwrap();
    assert!(Transport::checked(response).is_err());
    assert!(second.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn workspace_directory_pages_and_cancelled_mutations_obey_scope() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("cloud.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let cloud = fixture_cloud(&runtime, &server, Arc::new(MemoryStore::default())).await;
    let rows = (0..129).map(|index| json!({"workspace_id":uuid::Uuid::new_v4().to_string(),"role":"member","workspaces":{"name":format!("Workspace {index}")}})).collect::<Vec<_>>();
    Mock::given(method("GET"))
        .and(path("/rest/v1/workspace_memberships"))
        .and(query_param("offset", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&rows[..128]))
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/v1/workspace_memberships"))
        .and(query_param("offset", "128"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&rows[128..]))
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/list_my_workspace_invitations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(2)
        .mount(&server)
        .await;
    let scope = cloud.scope();
    let first = cloud
        .load_page(
            Surface::Teams,
            RequestGate::default().next(scope.clone()),
            0,
        )
        .await
        .unwrap();
    let second = cloud
        .load_page(
            Surface::Teams,
            RequestGate::default().next(scope.clone()),
            1,
        )
        .await
        .unwrap();
    first.validate(&scope).unwrap();
    second.validate(&scope).unwrap();
    assert_eq!(first.rows.len(), 128);
    assert_eq!(second.rows.len(), 1);
    assert_ne!(first.rows[0].id, second.rows[0].id);
    let request = RequestGate::default().next(scope);
    request.cancel.cancel();
    assert!(matches!(
        cloud
            .perform(
                Surface::Teams,
                request,
                Mutation {
                    operation: super::panels::operation(Action::CreateWorkspace, "Create"),
                    fields: Arc::from([(Arc::from("name"), Arc::from("Must not create"))])
                }
            )
            .await,
        Err(ServiceError::Cancelled)
    ));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn publication_projects_participant_names_and_meeting_time() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("cloud.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let cloud = fixture_cloud(&runtime, &server, Arc::new(MemoryStore::default())).await;
    let document = json!({"type":"doc","content":[{"type":"paragraph"}]});
    let scope = seed_summary(&runtime, document.clone()).await;
    let session = scope.session_id.clone().unwrap();
    runtime.service(move |services| async move {
        sqlx::query("UPDATE sessions SET started_at = '2026-09-18T10:00:00Z' WHERE id = ?")
            .bind(session.0.as_ref()).execute(services.db.pool()).await.unwrap();
        for (id, name, source) in [("one", "  Ada   李 ", "calendar"), ("two", "Ada 李", "calendar"), ("three", "Excluded", "excluded")] {
            sqlx::query("INSERT INTO session_participants (id, session_id, workspace_id, human_id, display_name, source) VALUES (?, ?, ?, ?, ?, ?)")
                .bind(id).bind(session.0.as_ref()).bind(ACCOUNT).bind(id).bind(name).bind(source)
                .execute(services.db.pool()).await.unwrap();
        }
        Ok(())
    }).unwrap().receive().await.unwrap();
    create_share_rpc(&server).await;
    Mock::given(method("PUT"))
        .and(path(format!("/sync/shares/{SHARE}/snapshot")))
        .and(body_partial_json(
            json!({"participants":["Ada 李"], "meetingAt":"2026-09-18T10:00:00Z"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(snapshot(1, document)))
        .expect(1)
        .mount(&server)
        .await;
    share_action(&cloud, &scope, Action::CreateShare)
        .await
        .unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn publication_preserves_existing_attachment_ids_and_cache_metadata() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("cloud.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let cloud = fixture_cloud(&runtime, &server, Arc::new(MemoryStore::default())).await;
    let document = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Draft"}]}]});
    let scope = seed_summary(&runtime, document.clone()).await;
    let attachment = uuid::Uuid::new_v4().to_string();
    let mut first = snapshot(1, document.clone());
    first["attachments"] = json!([{"id":attachment,"filename":"diagram.png","contentType":"image/png","sizeBytes":16,"sha256":"fixture-digest"}]);
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/create_session_share"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"share_id":SHARE}])))
        .expect(1)
        .mount(&server)
        .await;
    let initial = Mock::given(method("PUT"))
        .and(path(format!("/sync/shares/{SHARE}/snapshot")))
        .respond_with(ResponseTemplate::new(200).set_body_json(first.clone()))
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    share_action(&cloud, &scope, Action::CreateShare)
        .await
        .unwrap();
    drop(initial);
    first["contentRevision"] = json!(2);
    Mock::given(method("PUT"))
        .and(path(format!("/sync/shares/{SHARE}/snapshot")))
        .and(body_partial_json(
            json!({"baseRevision":1,"attachmentIds":[attachment]}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(first))
        .expect(1)
        .mount(&server)
        .await;
    share_action(&cloud, &scope, Action::PublishShare)
        .await
        .unwrap();
    runtime
        .service(move |services| async move {
            let value: String = sqlx::query_scalar(
                "SELECT attachments_json FROM shared_session_cache WHERE share_id = ?",
            )
            .bind(SHARE)
            .fetch_one(services.db.pool())
            .await
            .unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(&value).unwrap()[0]["id"],
                attachment
            );
            Ok(())
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn invitations_use_current_email_contract_and_report_clipboard_fallback() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("cloud.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let cloud = fixture_cloud(&runtime, &server, Arc::new(MemoryStore::default())).await;
    let invitation = uuid::Uuid::new_v4().to_string();
    let token = "a".repeat(43);
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/create_workspace_invitation"))
        .and(body_json(
            json!({"p_workspace_id":ACCOUNT,"p_invitee_email":"member@example.invalid"}),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!([{"invitation_id":invitation,"invite_token":token}])),
        )
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/v1/workspace_memberships"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(
                json!([{"workspace_id":ACCOUNT,"workspaces":{"name":"Fixture team"}}]),
            ),
        )
        .expect(2)
        .mount(&server)
        .await;
    let email = Mock::given(method("POST")).and(path(format!("/workspaces/invitations/{invitation}/email"))).and(body_json(json!({"workspaceId":ACCOUNT,"inviteToken":token,"workspaceName":"Fixture team","fromName":"fixture@example.invalid"}))).respond_with(ResponseTemplate::new(204)).expect(1).mount_as_scoped(&server).await;
    let scope = Scope {
        workspace_id: Some(ACCOUNT.into()),
        ..cloud.scope()
    };
    let mutation = || Mutation {
        operation: super::panels::operation(Action::InviteMember, "Invite"),
        fields: Arc::from([(Arc::from("email"), Arc::from("member@example.invalid"))]),
    };
    cloud
        .perform(
            Surface::Teams,
            RequestGate::default().next(scope.clone()),
            mutation(),
        )
        .await
        .unwrap();
    assert!(cloud.core.notice.lock().await.is_none());
    drop(email);
    Mock::given(method("POST"))
        .and(path(format!("/workspaces/invitations/{invitation}/email")))
        .respond_with(ResponseTemplate::new(502))
        .expect(1)
        .mount(&server)
        .await;
    cloud
        .perform(
            Surface::Teams,
            RequestGate::default().next(scope),
            mutation(),
        )
        .await
        .unwrap();
    assert_eq!(
        cloud.core.notice.lock().await.as_deref(),
        Some("Email unavailable. Invitation link copied instead")
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn encryption_setup_never_replaces_an_existing_server_identity_and_pause_is_durable() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("cloud.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let store = Arc::new(MemoryStore::default());
    let cloud = fixture_cloud(&runtime, &server, store.clone()).await;
    let mut gate = RequestGate::default();
    let mut request = || gate.next(cloud.scope());
    let mutation = |action| Mutation {
        operation: super::panels::operation(action, "Encryption fixture"),
        fields: Arc::from([]),
    };
    let conflicting = Mock::given(method("PUT"))
        .and(path("/sync/e2ee/identity"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"keyId":"ExistingAccountKey"})),
        )
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    let rejected = cloud
        .perform(
            Surface::CloudSync,
            request(),
            mutation(Action::SetupEncryption),
        )
        .await
        .unwrap_err();
    assert!(
        rejected
            .to_string()
            .contains("Account already has encryption"),
        "{rejected}"
    );
    assert!(
        store
            .read(format!("fixture:{ACCOUNT}:recovery"))
            .await
            .unwrap()
            .is_none()
    );
    let candidate = store
        .read(format!("fixture:{ACCOUNT}:candidate-recovery"))
        .await
        .unwrap()
        .unwrap();
    let key = anlg_e2ee::RecoveryKey::parse(&candidate).unwrap();
    drop(conflicting);
    Mock::given(method("PUT"))
        .and(path("/sync/e2ee/identity"))
        .and(body_json(json!({"keyId":key.key_id()})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"keyId":key.key_id()})))
        .expect(1)
        .mount(&server)
        .await;
    cloud
        .perform(
            Surface::CloudSync,
            request(),
            mutation(Action::SetupEncryption),
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .read(format!("fixture:{ACCOUNT}:recovery"))
            .await
            .unwrap()
            .unwrap()
            .as_str(),
        candidate.as_str()
    );
    assert!(
        cloud
            .perform(
                Surface::CloudSync,
                request(),
                mutation(Action::SetupEncryption)
            )
            .await
            .is_err()
    );
    cloud
        .perform(Surface::CloudSync, request(), mutation(Action::PauseSync))
        .await
        .unwrap();
    assert_eq!(
        store
            .read(format!("fixture:{ACCOUNT}:sync-enabled"))
            .await
            .unwrap()
            .unwrap()
            .as_str(),
        "false"
    );
    runtime.shutdown().await.unwrap();
}
