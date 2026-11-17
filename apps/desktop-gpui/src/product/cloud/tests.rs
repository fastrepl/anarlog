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
    CloudHost, CloudServices, ConflictDecision, ShareCommand,
    auth::{Auth, SecureStore},
    sharing::validate_document,
    transport::{CloudConfig, Transport},
};
use crate::product::{
    services::{Action, Mutation, ProductServices, RequestGate, Scope, Surface},
    sharing::GeneralAccess,
};

const ACCOUNT: &str = "180dc97e-633a-4766-9184-799f89db530b";
const SHARE: &str = "73d1e15a-9117-4260-8c78-5081a0c5012e";

#[tokio::test]
async fn shared_note_links_refresh_the_cache_or_claim_an_anonymous_preview() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("shared.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let cloud = fixture_cloud(&runtime, &server, Arc::new(MemoryStore::default())).await;
    let body = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"日本語 🧑🏽‍💻"}]}]});
    let row = json!({"share_id":SHARE,"workspace_id":ACCOUNT,"session_id":"remote-session","schema_version":1,
        "content_revision":2,"title":"Shared","body_json":body,"attachments_json":[],"capability":"commenter",
        "manage_access":false,"access_version":1,"web_editable":false,"published_at":"2026-01-01T00:00:00Z"});
    let read = Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/read_my_session_share_snapshot_v2"))
        .and(body_json(json!({"p_share_id":SHARE})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([row])))
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    cloud
        .refresh_shared(SHARE.into(), desktop_runtime::CancellationToken::new())
        .await
        .unwrap();
    drop(read);
    let cached = || {
        runtime
            .service(|services| async move {
                Ok(sqlx::query_scalar::<_, String>("SELECT title || ':' || capability FROM shared_session_cache WHERE viewer_user_id = ? AND share_id = ?")
                    .bind(ACCOUNT).bind(SHARE).fetch_optional(services.db.pool()).await.unwrap())
            })
            .unwrap()
    };
    assert_eq!(
        cached().receive().await.unwrap().as_deref(),
        Some("Shared:commenter")
    );
    // Revoked access removes the cached copy, so the view says access is gone.
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/read_my_session_share_snapshot_v2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(1)
        .mount(&server)
        .await;
    cloud
        .refresh_shared(SHARE.into(), desktop_runtime::CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(cached().receive().await.unwrap(), None);
    Mock::given(method("POST"))
        .and(path("/shared-notes/handoffs/claim"))
        .and(body_partial_json(json!({"request_id":SHARE})))
        .respond_with(ResponseTemplate::new(200).set_body_json(snapshot(1, body.clone())))
        .expect(1)
        .mount(&server)
        .await;
    let preview = cloud
        .claim_preview(SHARE.into(), desktop_runtime::CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(
        (preview.title.as_str(), preview.body),
        ("Web snapshot", body)
    );
    let cancelled = desktop_runtime::CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        cloud.claim_preview(SHARE.into(), cancelled).await,
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
async fn sign_in_handoff_checks_state_before_exchange_and_rejects_replayed_callback() {
    let server = MockServer::start().await;
    let auth = Auth::new(
        Transport::new(config(&server, "handoff")).unwrap(),
        Arc::new(MemoryStore::default()),
    )
    .await
    .unwrap();
    let login = auth.begin_sign_in().await.unwrap();
    assert_eq!(login.path(), "/auth");
    let pairs = login.query_pairs().collect::<HashMap<_, _>>();
    assert_eq!(pairs["flow"], "desktop");
    assert_eq!(pairs["scheme"], "anarlog-dev");
    let state = pairs["desktop_state"].to_string();
    let callback = |state: &str| -> reqwest::Url {
        let mut url: reqwest::Url = "anarlog-dev://auth/callback".parse().unwrap();
        url.query_pairs_mut()
            .append_pair("access_token", "handed-off")
            .append_pair("refresh_token", "fixture-refresh")
            .append_pair("state", state);
        url
    };
    assert!(auth.deep_link(callback("wrong")).await.is_err());
    assert!(server.received_requests().await.unwrap().is_empty());
    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .and(query_param("grant_type", "refresh_token"))
        .and(body_json(json!({"refresh_token": "fixture-refresh"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(session(now() + 3600, "fixture-token")),
        )
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(
        auth.deep_link(callback(&state))
            .await
            .unwrap()
            .account_id
            .as_deref(),
        Some(ACCOUNT)
    );
    assert!(auth.deep_link(callback(&state)).await.is_err());
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

#[tokio::test]
async fn account_counts_a_paid_workspace_seat_as_pro_without_offering_a_trial() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("account.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let cloud = fixture_cloud(&runtime, &server, Arc::new(MemoryStore::default())).await;
    const WORKSPACE: &str = "5b0f2a3c-9d7e-4a51-8f6b-2c1d0e9a8b7c";
    Mock::given(method("GET"))
        .and(path("/rest/v1/workspace_memberships"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"workspace_id": WORKSPACE, "role": "member",
             "workspaces": {"id": WORKSPACE, "name": "Acme", "kind": "shared"}},
            {"workspace_id": ACCOUNT, "role": "owner",
             "workspaces": {"id": ACCOUNT, "name": "Personal", "kind": "personal"}}
        ])))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/get_workspace_access"))
        .and(body_json(json!({"p_workspace_id": WORKSPACE})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"workspace_role": "member", "workspace_tier": "team", "capabilities": [],
             "seat_limit": 10, "used_seats": 4}
        ])))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/subscription/can-start-trial"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"canStartTrial": true})))
        .expect(0)
        .mount(&server)
        .await;

    let account = cloud.account().await.unwrap();
    let now = crate::product::account::now();
    assert!(account.is_pro(now));
    assert!(!account.can_start_trial(now));
    let seats = account.workspaces.as_deref().unwrap();
    assert_eq!(seats.len(), 1);
    assert_eq!((seats[0].used_seats, seats[0].seat_limit), (4, Some(10)));
    assert_eq!(
        account.user.unwrap().email.as_deref(),
        Some("fixture@example.invalid")
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn share_popover_publishes_before_its_first_invite_and_reads_back_access() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("cloud.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let cloud = fixture_cloud(&runtime, &server, Arc::new(MemoryStore::default())).await;
    let document = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Decisions"}]}]});
    let scope = seed_summary(&runtime, document.clone()).await;
    create_share_rpc(&server).await;
    Mock::given(method("PUT"))
        .and(path(format!("/sync/shares/{SHARE}/snapshot")))
        .and(body_partial_json(json!({"baseRevision":0,"body":document})))
        .respond_with(ResponseTemplate::new(200).set_body_json(snapshot(1, document.clone())))
        .expect(1)
        .mount(&server)
        .await;
    let invitation = uuid::Uuid::new_v4().to_string();
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/create_session_access_invitation"))
        .and(body_json(json!({"p_share_id":SHARE,"p_invitee_email":"maya@northwind.example","p_capability":"viewer"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!([{"invitation_id":invitation,"invite_token":"a".repeat(43)}]),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/shared-notes/invitations/{invitation}/email"
        )))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/get_session_share_management"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!([{"share_id":SHARE,
            "general_scope":"restricted","general_workspace_id":null,"has_active_link":false}])),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/list_session_share_access"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"entry_type":"invitation",
            "entry_id":invitation,"user_id":null,"user_email":"maya@northwind.example","capability":"viewer"}])))
        .mount(&server)
        .await;
    let share = cloud
        .share_command(
            scope.clone(),
            ShareCommand::Invite(vec!["Maya@Northwind.example".into()]),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(share.general, GeneralAccess::Restricted);
    let entry = share.access[0].clone();
    assert_eq!(entry.kind, crate::product::sharing::EntryKind::Invitation);
    // The share exists now, so later changes manage it without publishing again.
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/revoke_session_access_invitation"))
        .and(body_json(json!({"p_invitation_id":invitation})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(1)
        .mount(&server)
        .await;
    cloud
        .share_command(scope, ShareCommand::Remove(entry))
        .await
        .unwrap();
    runtime.shutdown().await.unwrap();
}

#[test]
fn account_share_urls_follow_the_workspace_subdomain_and_desktop_scheme() {
    use super::sharing::account_share_url;
    let production: reqwest::Url = "https://anarlog.so".parse().unwrap();
    assert_eq!(
        account_share_url(&production, SHARE, None, "anarlog").unwrap(),
        format!("https://anarlog.so/share/{SHARE}/")
    );
    assert_eq!(
        account_share_url(&production, SHARE, Some("lumen"), "anarlog-dev").unwrap(),
        format!("https://lumen.anarlog.so/share/{SHARE}/?scheme=anarlog-dev")
    );
    let local: reqwest::Url = "http://localhost:3000".parse().unwrap();
    assert_eq!(
        account_share_url(&local, SHARE, Some("lumen"), "anarlog").unwrap(),
        format!("http://localhost:3000/share/{SHARE}/")
    );
    for slug in ["www", "-lumen", "Lumen", "ab"] {
        assert!(account_share_url(&production, SHARE, Some(slug), "anarlog").is_err());
    }
    assert!(account_share_url(&production, "not-a-share", None, "anarlog").is_err());
}

#[tokio::test]
async fn folder_shares_publish_the_folder_payload_then_grant_a_guest() {
    use super::resources::{ResourceCommand, ResourceKind, is_team_upsell};
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("cloud.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let cloud = fixture_cloud(&runtime, &server, Arc::new(MemoryStore::default())).await;
    let summary = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Findings"}]}]});
    let scope = seed_summary(&runtime, summary.clone()).await;
    let id = scope.session_id.unwrap();
    runtime.service(move |services| async move {
        sqlx::query("INSERT INTO folders (id, path, instructions) VALUES ('f-research', 'Research', 'Customer interviews')").execute(services.db.pool()).await.unwrap();
        sqlx::query("UPDATE sessions SET folder_path = 'Research/Interviews', title = 'Interview' WHERE id = ?").bind(id.0.as_ref()).execute(services.db.pool()).await.unwrap();
        Ok(())
    }).unwrap().receive().await.unwrap();
    let share = uuid::Uuid::new_v4().to_string();
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/upsert_shared_resource"))
        .and(body_json(
            json!({"p_resource_type":"folder","p_source_id":"Research","p_title":"Research",
            "p_payload":{"version":1,"path":"Research","instructions":"Customer interviews",
                "notes":[{"title":"Interview","relativeFolderPath":"Interviews","body":summary}]},
            "p_general_workspace_id":null}),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!([{"share_id":share,"general_workspace_id":null}])),
        )
        .expect(2)
        .mount(&server)
        .await;
    let grant = Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/grant_shared_resource_access"))
        .and(body_json(
            json!({"p_share_id":share,"p_invitee_email":"guest@example.com"}),
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!([{"guest_id":"g","invitee_email":"guest@example.com"}])),
        )
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    let invite = || ResourceCommand::Invite {
        workspace: None,
        email: " Guest@Example.com ".into(),
    };
    cloud
        .resource_command(
            ResourceKind::Folder,
            "Research".into(),
            "Research".into(),
            invite(),
        )
        .await
        .unwrap();
    drop(grant);
    // A guest who already has another item needs Team access; the dialog shows the upsell.
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/grant_shared_resource_access"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_json(json!({"message":"multi-resource guest requires Team membership"})),
        )
        .mount(&server)
        .await;
    let error = cloud
        .resource_command(
            ResourceKind::Folder,
            "Research".into(),
            "Research".into(),
            invite(),
        )
        .await
        .unwrap_err();
    assert!(is_team_upsell(&error));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn template_shares_publish_the_editable_template_fields() {
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("cloud.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let payload = runtime
        .service(|services| async move {
            sqlx::query("INSERT INTO templates (id, title, description, category, targets_json, sections_json) VALUES ('t', 'Interview', 'Hiring loop', 'Hiring', '[\"Manager\"]', '[{\"title\":\"Signals\"}]')")
                .execute(services.db.pool()).await.unwrap();
            super::resources::template_payload(&services, "t").await
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(
        payload["template"],
        json!({"title":"Interview","description":"Hiring loop","category":"Hiring",
            "icon":{"type":"icon","value":"notebook-tabs","color":"#9ca3af"},
            "targets":["Manager"],"sections":[{"title":"Signals"}]})
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn enterprise_capture_applies_acknowledges_and_queues_finished_meetings() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("capture.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let cloud = fixture_cloud(&runtime, &server, Arc::new(MemoryStore::default())).await;
    const WORKSPACE: &str = "5b0f2a3c-9d7e-4a51-8f6b-2c1d0e9a8b7c";
    Mock::given(method("GET"))
        .and(path("/rest/v1/workspace_memberships"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"workspace_id": WORKSPACE, "role": "member",
             "workspaces": {"id": WORKSPACE, "name": "Acme", "kind": "shared"}}
        ])))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/get_workspace_access"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"workspace_role": "member", "workspace_tier": "enterprise",
             "capabilities": ["enterprise.capture"]}
        ])))
        .mount(&server)
        .await;
    let envelope = json!({
        "schema_version": 1, "source_id": "capture-job-1", "revision": 1, "finalized": true,
        "workspace_id": WORKSPACE, "owner_user_id": ACCOUNT,
        "session": {"id": "captured-session", "title": "Board sync", "status": "completed",
            "created_at": "2026-11-17T08:00:00.000Z", "updated_at": "2026-11-17T09:00:00.000Z",
            "started_at": "2026-11-17T08:00:00.000Z", "ended_at": "2026-11-17T09:00:00.000Z"},
        "documents": [], "transcripts": [], "participants": [], "attachments": []
    });
    let envelopes = format!("/v1/workspaces/{WORKSPACE}/session-envelopes");
    let first = Mock::given(method("GET"))
        .and(path(envelopes.clone()))
        .and(query_param("after", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{"cursor": 1, "jobId": "job-1", "revision": 1, "finalized": true,
                "contentHash": "a".repeat(64), "acknowledged": false,
                "createdAt": "2026-11-17T09:00:00.000Z", "envelope": envelope}],
            "nextCursor": 1, "hasMore": false
        })))
        .expect(1)
        .mount_as_scoped(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("{envelopes}/job-1/ack")))
        .and(body_json(
            json!({"consumerId": "fixture-device", "revision": 1, "contentHash": "a".repeat(64)}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"acknowledged": true})))
        .expect(1)
        .mount(&server)
        .await;
    let url: reqwest::Url = server.uri().parse().unwrap();
    let delivery = cloud.deliver_captures(url.clone()).await.unwrap();
    assert!(delivery.error.is_none(), "{:?}", delivery.error);
    assert_eq!(delivery.completions.len(), 1);
    assert_eq!(
        delivery.completions[0].session.0.as_ref(),
        "captured-session"
    );
    drop(first);
    let title = runtime
        .service(|services| async move {
            Ok(sqlx::query_scalar::<_, String>(
                "SELECT title FROM sessions WHERE id = 'captured-session'",
            )
            .fetch_one(services.db.pool())
            .await
            .unwrap())
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(title, "Board sync");
    // The next poll resumes after the stored cursor; the meeting stays queued until its
    // automations were started.
    Mock::given(method("GET"))
        .and(path(envelopes))
        .and(query_param("after", "1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"items": [], "nextCursor": 1, "hasMore": false})),
        )
        .expect(2)
        .mount(&server)
        .await;
    assert_eq!(
        cloud
            .deliver_captures(url.clone())
            .await
            .unwrap()
            .completions
            .len(),
        1
    );
    cloud
        .complete_capture("capture-job-1".into())
        .await
        .unwrap();
    assert!(
        cloud
            .deliver_captures(url)
            .await
            .unwrap()
            .completions
            .is_empty()
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn cloud_api_enable_uploads_existing_meetings_and_surfaces_api_errors() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("cloud-api.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let note = runtime
        .create_note("Roadmap".into())
        .unwrap()
        .receive()
        .await
        .unwrap();
    let cloud = fixture_cloud(&runtime, &server, Arc::new(MemoryStore::default())).await;
    let api = super::cloud_api::CloudApi::new(cloud, directory.path().join("cloud-api.json"));
    for enabled in [true, false] {
        Mock::given(method("PUT"))
            .and(path("/v1/cloud-api/settings"))
            .and(body_json(json!({ "enabled": enabled })))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({ "enabled": enabled, "updated_at": null })),
            )
            .expect(1)
            .mount(&server)
            .await;
    }
    Mock::given(method("PUT"))
        .and(path(format!("/v1/sync-snapshots/{}", note.summary.id.0)))
        .and(body_partial_json(json!({ "title": "Roadmap" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/cloud-api/keys"))
        .respond_with(ResponseTemplate::new(403).set_body_json(
            json!({ "error": { "code": "subscription_required", "message": "Cloud API requires Pro." } }),
        ))
        .mount(&server)
        .await;

    let enabled = api.set_enabled(true).await.unwrap();
    assert_eq!(
        (enabled.enabled, enabled.uploaded, enabled.backfill_failed),
        (true, 1, false)
    );
    let error = api.create_key("Claude Code".into()).await.unwrap_err();
    assert_eq!(error.to_string(), "Cloud API requires Pro.");
    assert!(!api.set_enabled(false).await.unwrap().enabled);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn team_calls_show_the_servers_seat_limit_reason() {
    use super::calls::CloudCalls;
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("cloud.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let cloud = fixture_cloud(&runtime, &server, Arc::new(MemoryStore::default())).await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/create_workspace_invitation"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "code": "22023", "details": null, "hint": null,
            "message": "workspace seat limit reached",
        })))
        .mount(&server)
        .await;
    let error = cloud
        .rpc("create_workspace_invitation", json!({}))
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "workspace seat limit reached");
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn sync_setup_claims_the_recovery_key_it_showed_and_enabling_without_one_asks_for_setup() {
    use super::calls::{CloudCalls, SyncBlock, SyncCommand};
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("cloud.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let store = Arc::new(MemoryStore::default());
    let cloud = fixture_cloud(&runtime, &server, store.clone()).await;
    // Without a recovery key the switch opens setup instead of enabling or failing.
    assert!(
        cloud
            .sync(SyncCommand::SetEnabled(true))
            .await
            .unwrap()
            .is_none()
    );
    let state = cloud.sync_state().await.unwrap();
    assert_eq!(
        (state.enabled, state.block),
        (false, Some(SyncBlock::SetupRequired))
    );
    // Reopening setup shows the same unsaved key, and "I saved it" claims exactly that key.
    let shown = cloud
        .sync(SyncCommand::CreateRecoveryKey)
        .await
        .unwrap()
        .unwrap();
    let again = cloud
        .sync(SyncCommand::CreateRecoveryKey)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(again.as_str(), shown.as_str());
    let key = anlg_e2ee::RecoveryKey::parse(&shown).unwrap();
    Mock::given(method("PUT"))
        .and(path("/sync/e2ee/identity"))
        .and(body_json(json!({"keyId": key.key_id()})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"keyId": key.key_id()})))
        .expect(1)
        .mount(&server)
        .await;
    cloud
        .sync(SyncCommand::UseRecoveryKey(shown.clone()))
        .await
        .unwrap();
    let stored = |slot: &str| store.read(format!("fixture:{ACCOUNT}:{slot}"));
    assert_eq!(
        stored("recovery").await.unwrap().unwrap().as_str(),
        shown.as_str()
    );
    assert!(stored("candidate-recovery").await.unwrap().is_none());
    assert_eq!(cloud.sync_state().await.unwrap().block, None);
    runtime.shutdown().await.unwrap();
}
