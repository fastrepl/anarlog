use std::sync::{Arc, Mutex as StdMutex};

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use wiremock::{Request as MockRequest, Respond};
use yrs::sync::{Message, SyncMessage};
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{Doc, GetString, ReadTxn, StateVector, Text, Transact, Update};

use super::*;
use crate::live_docs::{decode_hex, encode_hex};

const SHARE_ID: &str = "11111111-1111-4111-8111-111111111111";

fn live_test_state(server: &MockServer) -> AppState {
    AppState::new(SyncConfig {
        project_url: server.uri(),
        token_issuer_api_key: "issuer-key".to_string(),
        database_id: "database-id".to_string(),
        legacy_database_id: None,
        protocol_mode: CloudsyncProtocolMode::E2eeEnforced,
        desktop_transport: CloudsyncTransport::SqliteSync,
        token_ttl_seconds: 60,
        supabase_url: server.uri(),
        supabase_anon_key: "anon-key".to_string(),
        supabase_service_role_key: "service-role-key".to_string(),
    })
}

async fn mock_access(server: &MockServer, user_id: &str, capability: &str) {
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/resolve_session_share_live_access"))
        .and(header("apikey", "service-role-key"))
        .and(header("authorization", "Bearer service-role-key"))
        .and(body_partial_json(json!({
            "p_share_id": SHARE_ID,
            "p_actor_user_id": user_id,
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "share_id": SHARE_ID,
            "capability": capability,
            "access_version": 2,
            "content_revision": 5
        }])))
        .mount(server)
        .await;
}

/// In-memory stand-in for the Supabase live-document log, shared by every
/// "instance" (AppState) pointed at the same mock server.
#[derive(Default)]
struct FakeLog {
    updates: Vec<(i64, Vec<u8>)>,
    state: Option<Vec<u8>>,
    compacted_through: i64,
}

#[derive(Clone, Default)]
struct FakeStore(Arc<StdMutex<FakeLog>>);

impl FakeStore {
    fn seq(&self) -> i64 {
        let log = self.0.lock().unwrap();
        log.updates
            .last()
            .map(|(seq, _)| *seq)
            .unwrap_or(log.compacted_through)
    }

    async fn mount(&self, server: &MockServer) {
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/read_session_share_live_document"))
            .respond_with(ReadDocument(self.clone()))
            .mount(server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/read_session_share_live_updates"))
            .respond_with(ReadUpdates(self.clone()))
            .mount(server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/append_session_share_live_update"))
            .respond_with(AppendUpdate(self.clone()))
            .mount(server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/compact_session_share_live_document"))
            .respond_with(Compact(self.clone()))
            .mount(server)
            .await;
    }
}

struct ReadDocument(FakeStore);
impl Respond for ReadDocument {
    fn respond(&self, _: &MockRequest) -> ResponseTemplate {
        let log = self.0.0.lock().unwrap();
        ResponseTemplate::new(200).set_body_json(json!([{
            "share_id": SHARE_ID,
            "state_hex": log.state.as_deref().map(encode_hex),
            "compacted_through_seq": log.compacted_through,
            "updates": log.updates.iter().take(256).map(|(seq, update)| json!({
                "seq": seq,
                "update_hex": encode_hex(update)
            })).collect::<Vec<_>>()
        }]))
    }
}

struct ReadUpdates(FakeStore);
impl Respond for ReadUpdates {
    fn respond(&self, request: &MockRequest) -> ResponseTemplate {
        let body: Value = serde_json::from_slice(&request.body).unwrap();
        let after = body["p_after_seq"].as_i64().unwrap();
        let limit = body["p_limit"].as_u64().unwrap() as usize;
        let log = self.0.0.lock().unwrap();
        ResponseTemplate::new(200).set_body_json(
            log.updates
                .iter()
                .filter(|(seq, _)| *seq > after)
                .take(limit)
                .map(|(seq, update)| json!({ "seq": seq, "update_hex": encode_hex(update) }))
                .collect::<Vec<_>>(),
        )
    }
}

struct AppendUpdate(FakeStore);
impl Respond for AppendUpdate {
    fn respond(&self, request: &MockRequest) -> ResponseTemplate {
        let body: Value = serde_json::from_slice(&request.body).unwrap();
        assert_eq!(body["p_share_id"], SHARE_ID);
        let update = decode_hex(body["p_update_hex"].as_str().unwrap()).unwrap();
        let mut log = self.0.0.lock().unwrap();
        let has_content = !log.updates.is_empty() || log.state.is_some();
        if body["p_require_empty"] == true && has_content {
            return ResponseTemplate::new(200)
                .set_body_json(json!([{ "outcome": "not_empty", "seq": null }]));
        }
        let seq = log
            .updates
            .last()
            .map(|(seq, _)| *seq)
            .unwrap_or(log.compacted_through)
            + 1;
        log.updates.push((seq, update));
        ResponseTemplate::new(200).set_body_json(json!([{ "outcome": "appended", "seq": seq }]))
    }
}

struct Compact(FakeStore);
impl Respond for Compact {
    fn respond(&self, request: &MockRequest) -> ResponseTemplate {
        let body: Value = serde_json::from_slice(&request.body).unwrap();
        let through = body["p_through_seq"].as_i64().unwrap();
        let mut log = self.0.0.lock().unwrap();
        if through > log.compacted_through {
            log.state = decode_hex(body["p_state_hex"].as_str().unwrap());
            log.compacted_through = through;
            log.updates.retain(|(seq, _)| *seq > through);
        }
        ResponseTemplate::new(200).set_body_json(json!([{
            "share_id": SHARE_ID,
            "compacted_through_seq": log.compacted_through
        }]))
    }
}

fn authed_router(state: AppState, user_id: &str) -> Router {
    web_edit_router(state.clone())
        .merge(live_socket_router(state))
        .layer(Extension(AuthContext {
            token: "supabase-token".to_string(),
            claims: Claims {
                sub: user_id.to_string(),
                email: None,
                entitlements: Vec::new(),
                subscription_status: None,
                trial_end: None,
                has_payment_method: None,
                referral_extension: None,
            },
        }))
}

async fn issue_ticket(router: Router) -> Value {
    let response = router
        .oneshot(
            Request::post(format!("/shares/{SHARE_ID}/live/ticket"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response_json(response).await
}

async fn serve(router: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("ws://{address}")
}

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect(base: &str, ticket: &str) -> Socket {
    let (socket, _) =
        tokio_tungstenite::connect_async(format!("{base}/shares/{SHARE_ID}/live?ticket={ticket}"))
            .await
            .unwrap();
    socket
}

async fn next_binary(socket: &mut Socket) -> Vec<u8> {
    loop {
        match tokio::time::timeout(std::time::Duration::from_secs(5), socket.next())
            .await
            .expect("frame before timeout")
            .expect("open socket")
            .unwrap()
        {
            WsMessage::Binary(bytes) => return bytes.to_vec(),
            WsMessage::Text(_) | WsMessage::Ping(_) | WsMessage::Pong(_) => continue,
            other => panic!("unexpected frame {other:?}"),
        }
    }
}

async fn next_text(socket: &mut Socket) -> Value {
    match tokio::time::timeout(std::time::Duration::from_secs(5), socket.next())
        .await
        .expect("frame before timeout")
        .expect("open socket")
        .unwrap()
    {
        WsMessage::Text(text) => serde_json::from_str(&text).unwrap(),
        other => panic!("unexpected frame {other:?}"),
    }
}

/// Performs the handshake as a client would: read `ready`, step1, step2.
async fn handshake(socket: &mut Socket, doc: &Doc) -> Value {
    let ready = next_text(socket).await;
    let step1 = next_binary(socket).await;
    let step2 = next_binary(socket).await;
    assert!(matches!(
        Message::decode_v1(&step1).unwrap(),
        Message::Sync(SyncMessage::SyncStep1(_))
    ));
    match Message::decode_v1(&step2).unwrap() {
        Message::Sync(SyncMessage::SyncStep2(update)) => {
            doc.transact_mut()
                .apply_update(Update::decode_v1(&update).unwrap())
                .unwrap();
        }
        other => panic!("unexpected message {other:?}"),
    }
    ready
}

fn text_update(doc: &Doc, text: &str) -> Vec<u8> {
    let field = doc.get_or_insert_text("body");
    let mut txn = doc.transact_mut();
    let before = txn.before_state().clone();
    field.push(&mut txn, text);
    drop(txn);
    doc.transact().encode_diff_v1(&before)
}

fn update_frame(update: Vec<u8>) -> WsMessage {
    WsMessage::Binary(
        Message::Sync(SyncMessage::Update(update))
            .encode_v1()
            .into(),
    )
}

fn body_text(doc: &Doc) -> String {
    doc.get_or_insert_text("body").get_string(&doc.transact())
}

#[tokio::test]
async fn issues_tickets_that_carry_the_resolved_capability() {
    let server = MockServer::start().await;
    mock_access(&server, "user-123", "viewer").await;
    let response = issue_ticket(authed_router(live_test_state(&server), "user-123")).await;
    assert_eq!(response["capability"], "viewer");
    assert_eq!(response["contentRevision"], 5);
    assert_eq!(response["expiresInSeconds"], 60);
    assert!(response["ticket"].as_str().unwrap().contains('.'));
}

#[tokio::test]
async fn rejects_tickets_for_users_without_access() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/resolve_session_share_live_access"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;
    let response = authed_router(live_test_state(&server), "user-123")
        .oneshot(
            Request::post(format!("/shares/{SHARE_ID}/live/ticket"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn rejects_socket_upgrades_without_a_valid_ticket() {
    let server = MockServer::start().await;
    let base = serve(authed_router(live_test_state(&server), "user-123")).await;
    let error = tokio_tungstenite::connect_async(format!(
        "{base}/shares/{SHARE_ID}/live?ticket=forged.ticket"
    ))
    .await
    .expect_err("upgrade must fail");
    match error {
        tokio_tungstenite::tungstenite::Error::Http(response) => {
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
        other => panic!("unexpected error {other:?}"),
    }
}

#[tokio::test]
async fn relays_updates_between_editors_and_persists_them() {
    let server = MockServer::start().await;
    let store = FakeStore::default();
    store.mount(&server).await;
    mock_access(&server, "editor-a", "editor").await;
    mock_access(&server, "editor-b", "editor").await;
    let state = live_test_state(&server);
    let base = serve(authed_router(state.clone(), "editor-a")).await;
    let ticket_a = issue_ticket(authed_router(state.clone(), "editor-a")).await;
    let ticket_b = issue_ticket(authed_router(state.clone(), "editor-b")).await;

    let doc_a = Doc::new();
    let mut socket_a = connect(&base, ticket_a["ticket"].as_str().unwrap()).await;
    let ready = handshake(&mut socket_a, &doc_a).await;
    assert_eq!(ready["type"], "ready");
    assert_eq!(ready["capability"], "editor");
    assert_eq!(ready["seedRequired"], true);

    socket_a
        .send(update_frame(text_update(&doc_a, "hello")))
        .await
        .unwrap();

    let doc_b = Doc::new();
    let mut socket_b = connect(&base, ticket_b["ticket"].as_str().unwrap()).await;
    // Second joiner may race the append; poll until the seed is visible.
    let ready_b = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let ready = handshake(&mut socket_b, &doc_b).await;
            if body_text(&doc_b) == "hello" {
                return ready;
            }
            socket_b.close(None).await.ok();
            socket_b = connect(&base, ticket_b["ticket"].as_str().unwrap()).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(ready_b["seedRequired"], false);

    socket_b
        .send(update_frame(text_update(&doc_b, " world")))
        .await
        .unwrap();
    let relayed = next_binary(&mut socket_a).await;
    match Message::decode_v1(&relayed).unwrap() {
        Message::Sync(SyncMessage::Update(update)) => doc_a
            .transact_mut()
            .apply_update(Update::decode_v1(&update).unwrap())
            .unwrap(),
        other => panic!("unexpected message {other:?}"),
    }
    assert_eq!(body_text(&doc_a), "hello world");
    assert_eq!(store.seq(), 2);
}

#[tokio::test]
async fn viewers_receive_state_but_cannot_publish() {
    let server = MockServer::start().await;
    let store = FakeStore::default();
    {
        let seed = Doc::new();
        seed.get_or_insert_text("body")
            .push(&mut seed.transact_mut(), "seeded");
        let update = seed
            .transact()
            .encode_state_as_update_v1(&StateVector::default());
        store.0.lock().unwrap().updates.push((1, update));
    }
    store.mount(&server).await;
    mock_access(&server, "viewer-1", "viewer").await;
    let state = live_test_state(&server);
    let base = serve(authed_router(state.clone(), "viewer-1")).await;
    let ticket = issue_ticket(authed_router(state, "viewer-1")).await;

    let doc = Doc::new();
    let mut socket = connect(&base, ticket["ticket"].as_str().unwrap()).await;
    let ready = handshake(&mut socket, &doc).await;
    assert_eq!(ready["capability"], "viewer");
    assert_eq!(ready["seedRequired"], false);
    assert_eq!(body_text(&doc), "seeded");

    // Answering the relay's sync step 1 is part of the protocol, not an edit.
    let step2 = Message::Sync(SyncMessage::SyncStep2(
        doc.transact()
            .encode_state_as_update_v1(&StateVector::default()),
    ))
    .encode_v1();
    socket.send(WsMessage::Binary(step2.into())).await.unwrap();
    socket
        .send(
            Message::Sync(SyncMessage::SyncStep1(StateVector::default()))
                .encode_v1()
                .into(),
        )
        .await
        .unwrap();
    match Message::decode_v1(&next_binary(&mut socket).await).unwrap() {
        Message::Sync(SyncMessage::SyncStep2(_)) => {}
        other => panic!("unexpected message {other:?}"),
    }

    socket
        .send(update_frame(text_update(&doc, "!")))
        .await
        .unwrap();
    let closed = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            match socket.next().await {
                Some(Ok(WsMessage::Close(_))) | None | Some(Err(_)) => return true,
                Some(Ok(_)) => continue,
            }
        }
    })
    .await
    .unwrap();
    assert!(closed);
    assert_eq!(store.seq(), 1);
}

#[tokio::test]
async fn bootstrap_pages_through_long_update_logs() {
    let server = MockServer::start().await;
    let store = FakeStore::default();
    let source = Doc::new();
    for index in 0..300 {
        let update = text_update(&source, &format!("{index},"));
        let seq = index + 1;
        store.0.lock().unwrap().updates.push((seq, update));
    }
    store.mount(&server).await;
    mock_access(&server, "editor-1", "editor").await;
    let state = live_test_state(&server);
    let base = serve(authed_router(state.clone(), "editor-1")).await;
    let ticket = issue_ticket(authed_router(state, "editor-1")).await;

    let doc = Doc::new();
    let mut socket = connect(&base, ticket["ticket"].as_str().unwrap()).await;
    let ready = handshake(&mut socket, &doc).await;
    assert_eq!(ready["seedRequired"], false);
    assert_eq!(body_text(&doc), body_text(&source));
}

#[tokio::test]
async fn compaction_by_another_instance_does_not_lose_updates() {
    let server = MockServer::start().await;
    let store = FakeStore::default();
    let source = Doc::new();
    store
        .0
        .lock()
        .unwrap()
        .updates
        .push((1, text_update(&source, "one ")));
    store.mount(&server).await;
    mock_access(&server, "editor-1", "editor").await;
    let state = live_test_state(&server);
    let base = serve(authed_router(state.clone(), "editor-1")).await;
    let ticket = issue_ticket(authed_router(state, "editor-1")).await;

    let doc = Doc::new();
    let mut socket = connect(&base, ticket["ticket"].as_str().unwrap()).await;
    handshake(&mut socket, &doc).await;
    assert_eq!(body_text(&doc), "one ");

    // Another instance appends, compacts everything so far, then appends again
    // before this instance polls: seq 2 only survives inside the compacted state.
    let _ = text_update(&source, "two ");
    let three = text_update(&source, "three");
    {
        let mut log = store.0.lock().unwrap();
        log.state = Some(
            source
                .transact()
                .encode_state_as_update_v1(&StateVector::default()),
        );
        log.compacted_through = 2;
        log.updates.clear();
        log.updates.push((3, three));
    }

    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while body_text(&doc) != "one two three" {
            match Message::decode_v1(&next_binary(&mut socket).await).unwrap() {
                Message::Sync(SyncMessage::Update(update)) => doc
                    .transact_mut()
                    .apply_update(Update::decode_v1(&update).unwrap())
                    .unwrap(),
                other => panic!("unexpected message {other:?}"),
            }
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn peers_on_another_instance_converge_through_the_durable_log() {
    let server = MockServer::start().await;
    let store = FakeStore::default();
    store.mount(&server).await;
    mock_access(&server, "editor-a", "editor").await;
    mock_access(&server, "editor-b", "editor").await;
    let instance_a = live_test_state(&server);
    let instance_b = live_test_state(&server);
    let base_a = serve(authed_router(instance_a.clone(), "editor-a")).await;
    let base_b = serve(authed_router(instance_b.clone(), "editor-b")).await;
    let ticket_a = issue_ticket(authed_router(instance_a, "editor-a")).await;
    let ticket_b = issue_ticket(authed_router(instance_b, "editor-b")).await;

    let doc_a = Doc::new();
    let mut socket_a = connect(&base_a, ticket_a["ticket"].as_str().unwrap()).await;
    handshake(&mut socket_a, &doc_a).await;
    socket_a
        .send(update_frame(text_update(&doc_a, "from a")))
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while store.seq() < 1 {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();

    let doc_b = Doc::new();
    let mut socket_b = connect(&base_b, ticket_b["ticket"].as_str().unwrap()).await;
    let ready_b = handshake(&mut socket_b, &doc_b).await;
    assert_eq!(ready_b["seedRequired"], false);
    assert_eq!(body_text(&doc_b), "from a");

    socket_b
        .send(update_frame(text_update(&doc_b, " and b")))
        .await
        .unwrap();
    let relayed = next_binary(&mut socket_a).await;
    match Message::decode_v1(&relayed).unwrap() {
        Message::Sync(SyncMessage::Update(update)) => doc_a
            .transact_mut()
            .apply_update(Update::decode_v1(&update).unwrap())
            .unwrap(),
        other => panic!("unexpected message {other:?}"),
    }
    assert_eq!(body_text(&doc_a), "from a and b");
}

#[tokio::test]
async fn racing_seeds_reset_the_losing_peer() {
    let server = MockServer::start().await;
    let store = FakeStore::default();
    store.mount(&server).await;
    mock_access(&server, "editor-a", "editor").await;
    let state = live_test_state(&server);
    let base = serve(authed_router(state.clone(), "editor-a")).await;
    let ticket = issue_ticket(authed_router(state, "editor-a")).await;

    let doc = Doc::new();
    let mut socket = connect(&base, ticket["ticket"].as_str().unwrap()).await;
    let ready = handshake(&mut socket, &doc).await;
    assert_eq!(ready["seedRequired"], true);

    // Another instance seeds the log before this peer's seed lands.
    {
        let winner = Doc::new();
        winner
            .get_or_insert_text("body")
            .push(&mut winner.transact_mut(), "winner");
        let update = winner
            .transact()
            .encode_state_as_update_v1(&StateVector::default());
        store.0.lock().unwrap().updates.push((1, update));
    }

    socket
        .send(update_frame(text_update(&doc, "loser")))
        .await
        .unwrap();
    // The relay may have already polled the winning seed and relayed it as an
    // update; skip any binary frames until the reset control message.
    let reset = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            match socket.next().await.unwrap().unwrap() {
                WsMessage::Text(text) => return serde_json::from_str::<Value>(&text).unwrap(),
                WsMessage::Binary(_) | WsMessage::Ping(_) | WsMessage::Pong(_) => continue,
                other => panic!("unexpected frame {other:?}"),
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(reset["type"], "reset");
    let fresh = Doc::new();
    match Message::decode_v1(&next_binary(&mut socket).await).unwrap() {
        Message::Sync(SyncMessage::SyncStep2(update)) => fresh
            .transact_mut()
            .apply_update(Update::decode_v1(&update).unwrap())
            .unwrap(),
        other => panic!("unexpected message {other:?}"),
    }
    assert_eq!(body_text(&fresh), "winner");
    assert_eq!(store.seq(), 1);
}
