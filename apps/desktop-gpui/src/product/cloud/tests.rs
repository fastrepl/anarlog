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
async fn workspace_members_can_share_their_own_notes_but_not_someone_elses() {
    const TEAM: &str = "5b0f2a3c-9d7e-4a51-8f6b-2c1d0e9a8b7c";
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("cloud.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let cloud = fixture_cloud(&runtime, &server, Arc::new(MemoryStore::default())).await;
    let document = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Plan"}]}]});
    let scope = seed_summary(&runtime, document.clone()).await;
    let set_owner = |owner: &'static str| {
        let session = scope.session_id.clone().unwrap();
        runtime
            .service(move |services| async move {
                sqlx::query("UPDATE sessions SET workspace_id = ?, owner_user_id = ? WHERE id = ?")
                    .bind(TEAM)
                    .bind(owner)
                    .bind(session.0.as_ref())
                    .execute(services.db.pool())
                    .await
                    .unwrap();
                Ok(())
            })
            .unwrap()
            .receive()
    };
    runtime
        .service(|services| async move {
            sqlx::query("INSERT INTO workspaces (id, owner_user_id, kind) VALUES (?, 'someone-else', 'shared')")
                .bind(TEAM).execute(services.db.pool()).await.unwrap();
            sqlx::query("INSERT INTO workspace_memberships (id, workspace_id, user_id, role) VALUES ('m', ?, ?, 'member')")
                .bind(TEAM).bind(ACCOUNT).execute(services.db.pool()).await.unwrap();
            Ok(())
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    set_owner("someone-else").await.unwrap();
    let error = share_action(&cloud, &scope, Action::CreateShare)
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "This account does not own this meeting");
    set_owner(ACCOUNT).await.unwrap();
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/create_session_share"))
        .and(body_partial_json(json!({"p_workspace_id": TEAM})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"share_id": SHARE}])))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path(format!("/sync/shares/{SHARE}/snapshot")))
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
async fn paused_shares_still_open_the_web_copy_and_narrow_access_but_not_widen_it() {
    use crate::product::sharing::Capability;
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("cloud.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let cloud = fixture_cloud(&runtime, &server, Arc::new(MemoryStore::default())).await;
    let document = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Desktop edit"}]}]});
    let scope = seed_summary(&runtime, document.clone()).await;
    create_share_rpc(&server).await;
    Mock::given(method("PUT"))
        .and(path(format!("/sync/shares/{SHARE}/snapshot")))
        .respond_with(
            ResponseTemplate::new(409)
                .set_body_json(json!({"code":"snapshot_conflict","snapshot":snapshot(2,document)})),
        )
        .expect(1)
        .mount(&server)
        .await;
    assert!(matches!(
        share_action(&cloud, &scope, Action::CreateShare).await,
        Err(ServiceError::Conflict)
    ));
    let grant = uuid::Uuid::new_v4().to_string();
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
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"entry_type":"grant",
            "entry_id":grant,"user_id":uuid::Uuid::new_v4().to_string(),"user_email":"maya@northwind.example","capability":"editor"}])))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/get_session_share_workspace_slug"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!([{"workspace_share_slug":null}])),
        )
        .mount(&server)
        .await;
    for (capability, calls) in [("viewer", 1), ("editor", 0)] {
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/update_session_access_grant"))
            .and(body_json(
                json!({"p_grant_id":grant,"p_capability":capability}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .expect(calls)
            .mount(&server)
            .await;
    }

    let share = cloud
        .share_command(scope.clone(), ShareCommand::OpenWebCopy)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(share.conflict);
    let editor = share.access[0].clone();
    cloud
        .share_command(
            scope.clone(),
            ShareCommand::SetCapability(editor.clone(), Capability::Viewer),
        )
        .await
        .unwrap()
        .unwrap();
    let viewer = crate::product::sharing::AccessEntry {
        capability: Capability::Viewer,
        ..editor
    };
    assert!(
        cloud
            .share_command(
                scope,
                ShareCommand::SetCapability(viewer, Capability::Editor)
            )
            .await
            .is_err()
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn failed_publication_retries_the_identical_mutation_then_publishes_later_edits() {
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
    // An edit saved after the failure is published next, so the recap carries it.
    let edited = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Revised draft"}]}]});
    let session = scope.session_id.clone().unwrap();
    let body = edited.to_string();
    runtime
        .service(move |services| async move {
            sqlx::query("UPDATE session_documents SET body = ? WHERE session_id = ?")
                .bind(body)
                .bind(session.0.as_ref())
                .execute(services.db.pool())
                .await
                .unwrap();
            Ok(())
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    Mock::given(method("PUT"))
        .and(path(format!("/sync/shares/{SHARE}/snapshot")))
        .and(body_partial_json(json!({"baseRevision":1,"body":edited})))
        .respond_with(ResponseTemplate::new(200).set_body_json(snapshot(2, edited)))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/shared-notes/{SHARE}/recap/email")))
        .and(body_partial_json(json!({"noteBody": "Revised draft"})))
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
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;
    restored
        .share_command(
            scope,
            ShareCommand::EmailRecap(vec!["lee@northwind.example".into()]),
        )
        .await
        .unwrap()
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

#[tokio::test]
async fn shared_note_reconcile_evicts_notes_whose_access_was_revoked() {
    const REVOKED: &str = "0b8f4a52-2c4e-4d55-9a37-5f3e1c6d7a90";
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("shared.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let cloud = fixture_cloud(&runtime, &server, Arc::new(MemoryStore::default())).await;
    let row = |share: &str, title: &str, revision: u64| {
        json!({"share_id":share,"workspace_id":ACCOUNT,"session_id":format!("remote-{share}"),
            "schema_version":1,"content_revision":revision,"title":title,
            "body_json":{"type":"doc","content":[{"type":"paragraph"}]},"attachments_json":[],
            "capability":"viewer","manage_access":false,"access_version":1,"web_editable":false,
            "published_at":"2026-01-01T00:00:00Z"})
    };
    for (share, title) in [(SHARE, "Kept"), (REVOKED, "Revoked")] {
        let read = Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/read_my_session_share_snapshot_v2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([row(share, title, 1)])))
            .mount_as_scoped(&server)
            .await;
        cloud
            .refresh_shared(share.into(), desktop_runtime::CancellationToken::new())
            .await
            .unwrap();
        drop(read);
    }
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/list_my_session_share_snapshot_page_v2"))
        .and(body_json(json!({"p_after_share_id":null,"p_limit":8})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([row(
            SHARE,
            "Kept, edited",
            2
        )])))
        .mount(&server)
        .await;
    cloud
        .reconcile_shared(desktop_runtime::CancellationToken::new())
        .await
        .unwrap();
    let cached = runtime
        .service(|services| async move {
            Ok(sqlx::query_scalar::<_, String>(
                "SELECT title FROM shared_session_cache WHERE viewer_user_id = ? ORDER BY title",
            )
            .bind(ACCOUNT)
            .fetch_all(services.db.pool())
            .await
            .unwrap())
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(cached, ["Kept, edited"]);
    runtime.shutdown().await.unwrap();
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
async fn deleting_a_shared_note_revokes_its_share_and_clears_the_cache() {
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
    create_share_rpc(&server).await;
    Mock::given(method("PUT"))
        .and(path(format!("/sync/shares/{SHARE}/snapshot")))
        .respond_with(ResponseTemplate::new(200).set_body_json(snapshot(1, document)))
        .mount(&server)
        .await;
    share_action(&cloud, &scope, Action::CreateShare)
        .await
        .unwrap();
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/delete_session_share_by_session"))
        .and(body_partial_json(json!({"p_session_id": session.0.as_ref()})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"share_id": SHARE, "access_version": 2, "deleted_at": "2026-01-01T00:00:00Z", "was_deleted": true}])))
        .expect(1)
        .mount(&server)
        .await;
    let local_copies = || {
        runtime
            .service(|services| async move {
                let pool = services.db.pool();
                let count = |sql| async move {
                    sqlx::query_scalar::<_, i64>(sql)
                        .bind(SHARE)
                        .fetch_one(pool)
                        .await
                        .unwrap()
                };
                let journal = count("SELECT COUNT(*) FROM app_settings WHERE id LIKE 'gpui.share.%' AND json_extract(value_json, '$.share_id') = ?").await;
                let cache = count("SELECT COUNT(*) FROM shared_session_cache WHERE share_id = ?").await;
                Ok((journal, cache))
            })
            .unwrap()
            .receive()
    };
    // A live note keeps its share.
    cloud
        .revoke_deleted_shares(vec![session.clone()])
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(local_copies().await.unwrap(), (1, 1));
    let id = session.clone();
    runtime
        .service(move |services| async move {
            sqlx::query("UPDATE sessions SET deleted_at = '2026-01-01T00:00:00.000Z' WHERE id = ?")
                .bind(id.0.as_ref())
                .execute(services.db.pool())
                .await
                .unwrap();
            Ok(())
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    cloud
        .revoke_deleted_shares(vec![session])
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(local_copies().await.unwrap(), (0, 0));
    runtime.shutdown().await.unwrap();
}

/// Only the publishing device keeps a share's journal, so deleting the note on another device
/// once left its share live.
#[tokio::test]
async fn deleting_a_note_shared_from_another_device_revokes_its_share() {
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("cloud.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let cloud = fixture_cloud(&runtime, &server, Arc::new(MemoryStore::default())).await;
    let scope = seed_summary(
        &runtime,
        json!({"type":"doc","content":[{"type":"paragraph"}]}),
    )
    .await;
    let session = scope.session_id.clone().unwrap();
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/delete_session_share_by_session"))
        .and(body_json(json!({"p_workspace_id": ACCOUNT, "p_session_id": session.0.as_ref()})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"share_id": SHARE, "access_version": 2, "deleted_at": "2026-01-01T00:00:00Z", "was_deleted": true}])))
        .expect(1)
        .mount(&server)
        .await;
    let id = session.clone();
    let other = uuid::Uuid::new_v4().to_string();
    let other_share = other.clone();
    runtime
        .service(move |services| async move {
            // The share's cached copy, as opening its link here leaves it, beside another note's.
            for (share, session) in [(SHARE, id.0.as_ref()), (&*other_share, "another-note")] {
                sqlx::query("INSERT INTO shared_session_cache (share_id, viewer_user_id, workspace_id, session_id, content_revision, access_version, published_at) VALUES (?, ?, ?, ?, 1, 1, '2026-01-01T00:00:00Z')")
                    .bind(share).bind(ACCOUNT).bind(ACCOUNT).bind(session)
                    .execute(services.db.pool()).await.unwrap();
            }
            sqlx::query("UPDATE sessions SET deleted_at = '2026-01-01T00:00:00.000Z' WHERE id = ?")
                .bind(id.0.as_ref())
                .execute(services.db.pool())
                .await
                .unwrap();
            Ok(())
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    cloud
        .revoke_deleted_shares(vec![session])
        .unwrap()
        .receive()
        .await
        .unwrap();
    let cached = runtime
        .service(|services| async move {
            Ok(
                sqlx::query_scalar::<_, String>("SELECT share_id FROM shared_session_cache")
                    .fetch_all(services.db.pool())
                    .await
                    .unwrap(),
            )
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(cached, [other]);
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
async fn returning_from_checkout_renews_the_session_so_the_new_plan_shows() {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("account.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let cloud = fixture_cloud(&runtime, &server, Arc::new(MemoryStore::default())).await;
    let pro = format!(
        "header.{}.signature",
        URL_SAFE_NO_PAD.encode(json!({"entitlements": ["hyprnote_pro"]}).to_string())
    );
    Mock::given(method("POST"))
        .and(path("/auth/v1/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(session(now() + 3600, &pro)))
        .expect(1)
        .mount(&server)
        .await;
    cloud.refresh(true).await.unwrap();
    let account = cloud.account().await.unwrap();
    assert!(account.is_pro(crate::product::account::now()));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn share_popover_publishes_later_edits_before_sharing_or_emailing_them() {
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
        .share_command(scope.clone(), ShareCommand::Remove(entry))
        .await
        .unwrap()
        .unwrap();
    // An edit made after sharing is published before the link is copied, once.
    let edited = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Revised decisions"}]}]});
    let session = scope.session_id.clone().unwrap();
    let body = edited.to_string();
    runtime
        .service(move |services| async move {
            sqlx::query("UPDATE session_documents SET body = ? WHERE session_id = ?")
                .bind(body)
                .bind(session.0.as_ref())
                .execute(services.db.pool())
                .await
                .unwrap();
            Ok(())
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    Mock::given(method("PUT"))
        .and(path(format!("/sync/shares/{SHARE}/snapshot")))
        .and(body_partial_json(json!({"baseRevision":1,"body":edited})))
        .respond_with(ResponseTemplate::new(200).set_body_json(snapshot(2, edited.clone())))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/get_session_share_workspace_slug"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!([{"workspace_share_slug":null}])),
        )
        .mount(&server)
        .await;
    for _ in 0..2 {
        cloud
            .share_command(scope.clone(), ShareCommand::CopyLink)
            .await
            .unwrap()
            .unwrap();
    }
    // The recap email carries the published note as Markdown. It was sent once, so a failed
    // read-back of the share afterwards doesn't report it as failed and invite a second send.
    Mock::given(method("POST"))
        .and(path(format!("/shared-notes/{SHARE}/recap/email")))
        .and(body_partial_json(
            json!({"recipients": ["lee@northwind.example"],
            "noteTitle": "Web snapshot", "noteBody": "Revised decisions"}),
        ))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/get_session_share_management"))
        .respond_with(ResponseTemplate::new(503))
        .with_priority(1)
        .mount(&server)
        .await;
    let refreshed = cloud
        .share_command(
            scope,
            ShareCommand::EmailRecap(vec!["lee@northwind.example".into()]),
        )
        .await
        .unwrap();
    assert!(refreshed.is_err());
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
async fn folder_shares_treat_underscore_and_percent_in_folder_names_literally() {
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("cloud.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let titles = |folder: &'static str| {
        runtime
            .service(move |services| async move {
                let payload = super::resources::folder_payload(&services, folder).await?;
                Ok(payload["notes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|note| note["title"].as_str().unwrap().to_owned())
                    .collect::<Vec<_>>())
            })
            .unwrap()
            .receive()
    };
    runtime
        .service(|services| async move {
            sqlx::query(
                "INSERT INTO sessions (id, title, folder_path, created_at) VALUES
            ('1', 'Shared', 'Client_', '2026-01-01'), ('2', 'Nested', 'Client_/Sub', '2026-01-02'),
            ('3', 'Private', 'ClientA/private', '2026-01-03'), ('4', 'Quota', '100%', '2026-01-04'),
            ('5', 'Hidden', '100 days/plan', '2026-01-05')",
            )
            .execute(services.db.pool())
            .await
            .unwrap();
            Ok(())
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(titles("Client_").await.unwrap(), ["Shared", "Nested"]);
    assert_eq!(titles("100%").await.unwrap(), ["Quota"]);
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
async fn cloud_api_uploads_an_edit_saved_while_the_previous_upload_was_in_flight() {
    use super::cloud_api::{CloudApi, Revision};
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("cloud-api.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let note = runtime
        .create_note("Draft".into())
        .unwrap()
        .receive()
        .await
        .unwrap();
    let id = note.summary.id.0.to_string();
    let cloud = fixture_cloud(&runtime, &server, Arc::new(MemoryStore::default())).await;
    let api = CloudApi::new(cloud, directory.path().join("cloud-api.json"));
    Mock::given(method("GET"))
        .and(path("/v1/cloud-api/settings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "enabled": true })))
        .mount(&server)
        .await;
    let snapshot = format!("/v1/sync-snapshots/{id}");
    Mock::given(method("PUT"))
        .and(path(snapshot.as_str()))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({}))
                .set_delay(std::time::Duration::from_millis(300)),
        )
        .mount(&server)
        .await;
    let revision = |revision: &str| {
        [Revision {
            id: id.clone(),
            deleted_at: None,
            revision: revision.into(),
        }]
    };
    let uploaded_titles = async || {
        let requests = server.received_requests().await.unwrap();
        requests
            .iter()
            .filter(|request| request.method.as_str() == "PUT" && request.url.path() == snapshot)
            .map(|request| request.body_json::<Value>().unwrap()["title"].clone())
            .collect::<Vec<_>>()
    };
    api.observe(ACCOUNT, &revision("1"));
    let first = tokio::spawn(api.sync(ACCOUNT.into(), false));
    while uploaded_titles().await.is_empty() {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let renamed = id.clone();
    runtime
        .service(move |services| async move {
            sqlx::query("UPDATE sessions SET title = 'Final' WHERE id = ?")
                .bind(renamed)
                .execute(services.db.pool())
                .await
                .unwrap();
            Ok(())
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    api.observe(ACCOUNT, &revision("2"));
    first.await.unwrap().unwrap();
    api.sync(ACCOUNT.into(), false).await.unwrap();
    assert_eq!(uploaded_titles().await, [json!("Draft"), json!("Final")]);
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
async fn workspace_keys_are_granted_to_members_with_an_identity_without_waiting_for_the_rest() {
    const WORKSPACE: &str = "5b0f2a3c-9d7e-4a51-8f6b-2c1d0e9a8b7c";
    const PENDING: &str = "9c3e7d1a-4b2f-4e8a-b6c5-0d1f2e3a4b5c";
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("cloud.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let cloud = fixture_cloud(&runtime, &server, Arc::new(MemoryStore::default())).await;
    let recovery = anlg_e2ee::RecoveryKey::generate().unwrap();
    let identity = recovery.member_identity_key().unwrap();
    Mock::given(method("GET"))
        .and(path(format!(
            "/sync/e2ee/workspaces/{WORKSPACE}/recipients"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"userId": ACCOUNT, "publicKey": identity.public_key(), "grantedKeyIds": []},
            {"userId": PENDING, "publicKey": null, "grantedKeyIds": []},
        ])))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path(format!("/sync/e2ee/workspaces/{WORKSPACE}/key")))
        .respond_with(|request: &wiremock::Request| {
            let body = request.body_json::<Value>().unwrap();
            ResponseTemplate::new(200).set_body_json(json!({"keyId": body["keyId"]}))
        })
        .mount(&server)
        .await;
    let credentials = json!({"workspaces": [{"kind": "shared", "role": "owner", "id": WORKSPACE}],
        "workspaceKeyGrants": []});
    let lease = cloud.core.auth.lease(ACCOUNT, false).await.unwrap();
    let published = cloud
        .core
        .sync
        .provision_keys(&cloud.core, &lease, &recovery, &credentials)
        .await
        .unwrap();
    assert!(published);
    let requests = server.received_requests().await.unwrap();
    let key = requests
        .iter()
        .find(|request| request.method.as_str() == "PUT")
        .unwrap()
        .body_json::<Value>()
        .unwrap();
    let granted = key["grants"]
        .as_array()
        .unwrap()
        .iter()
        .map(|grant| grant["userId"].clone())
        .collect::<Vec<_>>();
    assert_eq!(granted, [json!(ACCOUNT)]);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn devices_waiting_for_approval_keep_waiting_and_syncing_devices_approve_them() {
    use super::calls::{CloudCalls, SyncBlock};
    let server = MockServer::start().await;
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("cloud.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let store = Arc::new(MemoryStore::default());
    let cloud = fixture_cloud(&runtime, &server, store.clone()).await;
    let slot = |name: &str| format!("fixture:{ACCOUNT}:{name}");
    let enrollment = anlg_e2ee::DeviceEnrollmentKey::generate().unwrap();
    store
        .write(slot("sync-enabled"), Zeroizing::new("true".into()))
        .await
        .unwrap();
    store
        .write(slot("enrollment"), enrollment.expose_code())
        .await
        .unwrap();
    // This device asked for approval earlier: each refresh checks again and keeps waiting.
    let expires = (chrono::Utc::now() + chrono::Duration::minutes(10)).to_rfc3339();
    Mock::given(method("POST"))
        .and(path("/sync/e2ee/device-enrollments"))
        .and(body_partial_json(json!({"publicKey": enrollment.public_key()})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "requestId": uuid::Uuid::new_v4().to_string(), "status": "pending", "expiresAt": expires,
        })))
        .expect(1)
        .mount(&server)
        .await;
    assert!(cloud.refresh(false).await.is_err());
    assert_eq!(
        cloud.sync_state().await.unwrap().block,
        Some(SyncBlock::ApprovalPending)
    );
    // A syncing device approves the account's other open requests.
    let recovery = anlg_e2ee::RecoveryKey::generate().unwrap();
    store
        .write(slot("recovery"), recovery.expose_code())
        .await
        .unwrap();
    let request = |status: &str, expires: String| {
        json!({"requestId": uuid::Uuid::new_v4().to_string(), "deviceFingerprint": "device-fingerprint",
            "publicKey": anlg_e2ee::DeviceEnrollmentKey::generate().unwrap().public_key(),
            "status": status, "expiresAt": expires})
    };
    let open = request("pending", expires.clone());
    Mock::given(method("GET"))
        .and(path("/sync/devices"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"devices": [], "maxDevices": 3,
            "pendingDevices": [open, request("sealed", expires),
                request("pending", "2026-01-01T00:00:00Z".into())]}),
        ))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/sync/e2ee/device-enrollments/{}/seal",
            open["requestId"].as_str().unwrap()
        )))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    let lease = cloud.core.auth.lease(ACCOUNT, false).await.unwrap();
    cloud
        .core
        .sync
        .seal_open_requests(&cloud.core, &lease)
        .await
        .unwrap();
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
