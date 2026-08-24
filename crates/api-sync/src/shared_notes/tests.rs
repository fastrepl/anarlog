use anlg_api_auth::Claims;
use axum::{body::Body, body::to_bytes, http::Request, http::StatusCode};
use serde_json::{Value, json};
use tower::ServiceExt;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_json, body_partial_json, header, method, path},
};

use super::*;

const SHARE_ID: &str = "11111111-1111-4111-8111-111111111111";
const PUBLIC_SLUG: &str = "s_0123456789abcdef0123456789abcdef";
const LINK_TOKEN: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const LINK_PREVIEW_TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const LINK_ID: &str = "77777777-7777-4777-8777-777777777777";
const HANDOFF_ID: &str = "22222222-2222-4222-8222-222222222222";
const LEASE_ID: &str = "55555555-5555-4555-8555-555555555555";
const ATTACHMENT_ID: &str = "33333333-3333-4333-8333-333333333333";
const OWNER_ID: &str = "44444444-4444-4444-8444-444444444444";
const INVITATION_ID: &str = "66666666-6666-4666-8666-666666666666";

fn test_router(server: &MockServer) -> Router {
    router(SharedNotesState::new(
        SharedNotesConfig::new(server.uri(), "service-role-key").unwrap(),
    ))
}

fn test_source_hash(client_ip: Option<&str>) -> String {
    let state = SharedNotesState::new(
        SharedNotesConfig::new("https://project.supabase.co", "service-role-key").unwrap(),
    );
    let mut headers = HeaderMap::new();
    if let Some(client_ip) = client_ip {
        headers.insert(FLY_CLIENT_IP_HEADER, client_ip.parse().unwrap());
    }
    state.handoff_source_hash(&headers)
}

fn snapshot_row(title: &str) -> Value {
    json!({
        "share_id": SHARE_ID,
        "schema_version": 1,
        "content_revision": 2,
        "title": title,
        "body_json": {
            "type": "doc",
            "content": [{ "type": "paragraph" }]
        },
        "attachments_json": [],
        "published_at": "2026-07-16T10:00:00Z"
    })
}

async fn response_json(response: Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn mount_rpc(server: &MockServer, function: &str, request: Value, response: Value) {
    Mock::given(method("POST"))
        .and(path(format!("/rest/v1/rpc/{function}")))
        .and(header("apikey", "service-role-key"))
        .and(header("authorization", "Bearer service-role-key"))
        .and(body_partial_json(request))
        .respond_with(ResponseTemplate::new(200).set_body_json(response))
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test]
async fn verifies_and_sends_a_resend_shared_note_invitation_email() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/list_session_share_access"))
        .and(header("apikey", "service-role-key"))
        .and(header("authorization", "Bearer user-token"))
        .and(body_partial_json(json!({ "p_share_id": SHARE_ID })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "entry_type": "invitation",
            "entry_id": INVITATION_ID,
            "user_email": "invitee@example.com",
            "status": "pending"
        }])))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/get_session_share_workspace_slug"))
        .and(header("apikey", "service-role-key"))
        .and(header("authorization", "Bearer user-token"))
        .and(body_partial_json(json!({ "p_share_id": SHARE_ID })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!([{ "workspace_share_slug": "fastrepl" }])),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/emails"))
        .and(header("authorization", "Bearer resend-key"))
        .and(header("idempotency-key", INVITATION_ID))
        .and(body_partial_json(json!({
            "from": "Owner via Anarlog <notes@send.anarlog.so>",
            "to": "invitee@example.com",
            "reply_to": "owner@example.com",
            "subject": "Owner invited you to Planning",
            "text": "Owner invited you to view \"Planning\" in Anarlog.\n\nOpen the meeting notes:\nhttps://fastrepl.anarlog.so/share/invite/66666666-6666-4666-8666-666666666666/#token=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n\nReply to this email to contact Owner."
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "email-id" })))
        .expect(1)
        .mount(&server)
        .await;
    let config = SharedNotesConfig::new(server.uri(), "service-role-key")
        .unwrap()
        .with_resend_email("resend-key", "notes@send.anarlog.so")
        .unwrap()
        .with_resend_api_base(reqwest::Url::parse(&format!("{}/", server.uri())).unwrap());
    let app = authenticated_router(SharedNotesState::new(config)).layer(Extension(AuthContext {
        token: "user-token".to_string(),
        claims: Claims {
            sub: OWNER_ID.to_string(),
            email: Some("owner@example.com".to_string()),
            entitlements: vec![],
            subscription_status: None,
            trial_end: None,
            has_payment_method: None,
        },
    }));

    let response = app
        .oneshot(
            Request::post(format!("/shared-notes/invitations/{INVITATION_ID}/email"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "shareId": SHARE_ID,
                        "inviteToken": LINK_TOKEN,
                        "noteTitle": "Planning",
                        "fromName": "Owner"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn authorizes_and_sends_a_meeting_recap_to_each_recipient() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/list_session_share_access"))
        .and(header("apikey", "service-role-key"))
        .and(header("authorization", "Bearer user-token"))
        .and(body_partial_json(json!({ "p_share_id": SHARE_ID })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/emails/batch"))
        .and(header("authorization", "Bearer resend-key"))
        .and(header("idempotency-key", INVITATION_ID))
        .and(body_json(json!([
            {
                "from": "Owner via Anarlog <notes@send.anarlog.so>",
                "to": "one@example.com",
                "reply_to": "owner@example.com",
                "subject": "Meeting notes: Planning",
                "text": "Planning\n\n## Decisions\n\nShip it.\n\nSent by Owner via Anarlog. Reply to this email to contact them."
            },
            {
                "from": "Owner via Anarlog <notes@send.anarlog.so>",
                "to": "two@example.com",
                "reply_to": "owner@example.com",
                "subject": "Meeting notes: Planning",
                "text": "Planning\n\n## Decisions\n\nShip it.\n\nSent by Owner via Anarlog. Reply to this email to contact them."
            }
        ])))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": [] })))
        .expect(1)
        .mount(&server)
        .await;
    let config = SharedNotesConfig::new(server.uri(), "service-role-key")
        .unwrap()
        .with_resend_email("resend-key", "notes@send.anarlog.so")
        .unwrap()
        .with_resend_api_base(reqwest::Url::parse(&format!("{}/", server.uri())).unwrap());
    let app = authenticated_router(SharedNotesState::new(config)).layer(Extension(AuthContext {
        token: "user-token".to_string(),
        claims: Claims {
            sub: OWNER_ID.to_string(),
            email: Some("owner@example.com".to_string()),
            entitlements: vec![],
            subscription_status: None,
            trial_end: None,
            has_payment_method: None,
        },
    }));

    let response = app
        .oneshot(
            Request::post(format!("/shared-notes/{SHARE_ID}/recap/email"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "recipients": ["one@example.com", "two@example.com"],
                        "senderName": "Owner",
                        "noteTitle": "Planning",
                        "noteBody": "## Decisions\n\nShip it.",
                        "deliveryId": INVITATION_ID
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn reads_public_snapshot_through_the_service_gateway() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "gateway_read_public_session_share_snapshot_v2",
        json!({ "p_public_slug": PUBLIC_SLUG }),
        json!([snapshot_row("Public note")]),
    )
    .await;

    let response = test_router(&server)
        .oneshot(
            Request::get(format!("/shared-notes/public/{PUBLIC_SLUG}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let body = response_json(response).await;
    assert_eq!(body["shareId"], SHARE_ID);
    assert_eq!(body["schemaVersion"], 1);
    assert_eq!(body["contentRevision"], 2);
    assert_eq!(body["title"], "Public note");
    assert!(body.get("accessVersion").is_none());
    assert!(body.get("workspaceId").is_none());
    assert!(body.get("sessionId").is_none());
    assert_eq!(body["attachments"], json!([]));
}

#[tokio::test]
async fn reads_a_stable_link_without_a_rotating_bearer_token() {
    let server = MockServer::start().await;
    let mut row = snapshot_row("Stable link note");
    row["general_scope"] = json!("link");
    mount_rpc(
        &server,
        "gateway_read_stable_session_share_snapshot_v2",
        json!({ "p_share_id": SHARE_ID }),
        json!([row]),
    )
    .await;

    let response = test_router(&server)
        .oneshot(
            Request::get(format!("/shared-notes/share/{SHARE_ID}"))
                .header(header::COOKIE, "session=private")
                .header(header::AUTHORIZATION, "Bearer user-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["accessScope"], "link");
    assert_eq!(body["snapshot"]["shareId"], SHARE_ID);
    assert_eq!(body["snapshot"]["title"], "Stable link note");
    let upstream = server.received_requests().await.unwrap().pop().unwrap();
    assert!(upstream.headers.get("cookie").is_none());
    assert_eq!(
        upstream.headers["authorization"].to_str().unwrap(),
        "Bearer service-role-key"
    );
    assert!(
        !String::from_utf8(upstream.body)
            .unwrap()
            .contains("user-token")
    );
}

#[tokio::test]
async fn authorizes_before_signing_a_public_attachment_download() {
    let server = MockServer::start().await;
    let object_key = format!("{OWNER_ID}/{SHARE_ID}/{ATTACHMENT_ID}.sna1");
    Mock::given(method("POST"))
        .and(path(
            "/rest/v1/rpc/gateway_prepare_public_session_share_attachment_download",
        ))
        .and(body_partial_json(json!({
            "p_public_slug": PUBLIC_SLUG,
            "p_attachment_id": ATTACHMENT_ID
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "share_id": SHARE_ID,
            "attachment_id": ATTACHMENT_ID,
            "object_key": object_key,
            "filename": "diagram.png",
            "content_type": "image/png",
            "size_bytes": 1024,
            "sha256": "a".repeat(64),
            "access_version": 3,
            "cleanup_not_before": "2026-07-16T10:10:00Z"
        }])))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/storage/v1/object/sign/{SHARED_ATTACHMENT_BUCKET}/{object_key}"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "signedURL": format!(
                "/object/sign/{SHARED_ATTACHMENT_BUCKET}/{object_key}?token=download-token"
            )
        })))
        .expect(1)
        .mount(&server)
        .await;

    let response = test_router(&server)
        .oneshot(
            Request::post(format!(
                "/shared-notes/public/{PUBLIC_SLUG}/attachments/{ATTACHMENT_ID}/download"
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let body = response_json(response).await;
    assert_eq!(body["id"], ATTACHMENT_ID);
    assert_eq!(body["filename"], "diagram.png");
    assert_eq!(body["contentType"], "image/png");
    assert!(
        body["signedUrl"]
            .as_str()
            .unwrap()
            .contains("download-token")
    );
}

#[tokio::test]
async fn accepts_a_maximum_size_document_with_bounded_envelope_overhead() {
    let server = MockServer::start().await;
    let mut body = json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [{ "type": "text", "text": "" }]
        }]
    });
    let empty_body_size = serde_json::to_vec(&body).unwrap().len();
    body["content"][0]["content"][0]["text"] =
        Value::String("x".repeat(MAX_SNAPSHOT_BODY_BYTES - empty_body_size));
    assert_eq!(
        serde_json::to_vec(&body).unwrap().len(),
        MAX_SNAPSHOT_BODY_BYTES
    );

    let mut row = snapshot_row("Maximum note");
    row["body_json"] = body;
    let gateway_response = json!([row]);
    let gateway_response_size = serde_json::to_vec(&gateway_response).unwrap().len();
    assert!(gateway_response_size > MAX_SNAPSHOT_BODY_BYTES);
    assert!(gateway_response_size <= MAX_SNAPSHOT_RESPONSE_BYTES);
    mount_rpc(
        &server,
        "gateway_read_public_session_share_snapshot_v2",
        json!({ "p_public_slug": PUBLIC_SLUG }),
        gateway_response,
    )
    .await;

    let response = test_router(&server)
        .oneshot(
            Request::get(format!("/shared-notes/public/{PUBLIC_SLUG}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await["title"], "Maximum note");
}

#[tokio::test]
async fn reads_link_snapshot_without_forwarding_client_headers() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "gateway_read_session_share_link_snapshot_v2",
        json!({ "p_share_id": SHARE_ID, "p_link_token": LINK_TOKEN }),
        json!([snapshot_row("Link note")]),
    )
    .await;

    let response = test_router(&server)
        .oneshot(
            Request::post(format!("/shared-notes/link/{SHARE_ID}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, "session=private")
                .header(header::AUTHORIZATION, "Bearer user-token")
                .body(Body::from(json!({ "token": LINK_TOKEN }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await["title"], "Link note");
    let upstream = server.received_requests().await.unwrap().pop().unwrap();
    assert!(upstream.headers.get("cookie").is_none());
    assert_eq!(
        upstream.headers["authorization"].to_str().unwrap(),
        "Bearer service-role-key"
    );
    assert!(
        !String::from_utf8(upstream.body)
            .unwrap()
            .contains("user-token")
    );
}

#[tokio::test]
async fn returns_only_link_preview_metadata() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "gateway_read_session_share_link_preview",
        json!({
            "p_share_id": SHARE_ID,
            "p_preview_token": LINK_PREVIEW_TOKEN
        }),
        json!([{
            "title": "Planning & decisions",
            "body_json": {
                "type": "doc",
                "content": [
                    { "type": "heading", "content": [{ "type": "text", "text": "Overview" }] },
                    {
                        "type": "paragraph",
                        "content": [
                            { "type": "text", "text": "The team aligned" },
                            { "type": "hardBreak" },
                            { "type": "text", "text": "on launch scope and remaining blockers." }
                        ]
                    }
                ]
            },
            "participants": ["John Jeong", "Sungbin Jo"],
            "meeting_at": "2026-08-06T01:30:00Z"
        }]),
    )
    .await;

    let response = test_router(&server)
        .oneshot(
            Request::post(format!("/shared-notes/link/{SHARE_ID}/preview"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "previewToken": LINK_PREVIEW_TOKEN }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response_json(response).await,
        json!({
            "title": "Planning & decisions",
            "summary": "The team aligned on launch scope and remaining blockers.",
            "participants": ["John Jeong", "Sungbin Jo"],
            "meetingAt": "2026-08-06T01:30:00Z"
        })
    );
}

#[tokio::test]
async fn resolves_short_link_preview_metadata() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "gateway_read_session_share_link_preview_by_id",
        json!({ "p_link_id": LINK_ID }),
        json!([{
            "share_id": SHARE_ID,
            "title": "Planning & decisions",
            "body_json": {
                "type": "doc",
                "content": [{
                    "type": "paragraph",
                    "content": [{ "type": "text", "text": "The team aligned." }]
                }]
            },
            "participants": ["John Jeong", "Sungbin Jo"],
            "meeting_at": "2026-08-12T01:30:00Z"
        }]),
    )
    .await;

    let response = test_router(&server)
        .oneshot(
            Request::get(format!("/shared-notes/links/{LINK_ID}/preview"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response_json(response).await,
        json!({
            "shareId": SHARE_ID,
            "title": "Planning & decisions",
            "summary": "The team aligned.",
            "participants": ["John Jeong", "Sungbin Jo"],
            "meetingAt": "2026-08-12T01:30:00Z"
        })
    );
}

#[tokio::test]
async fn returns_public_preview_metadata() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "gateway_read_public_session_share_preview",
        json!({ "p_public_slug": PUBLIC_SLUG }),
        json!([{
            "title": "Customer review",
            "body_json": {
                "type": "doc",
                "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Customer feedback shaped the next product iteration." }] }]
            },
            "participants": ["John Jeong", "Artem"],
            "meeting_at": "2026-08-05T09:00:00Z"
        }]),
    )
    .await;

    let response = test_router(&server)
        .oneshot(
            Request::get(format!("/shared-notes/public/{PUBLIC_SLUG}/preview"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response_json(response).await,
        json!({
            "title": "Customer review",
            "summary": "Customer feedback shaped the next product iteration.",
            "participants": ["John Jeong", "Artem"],
            "meetingAt": "2026-08-05T09:00:00Z"
        })
    );
}

#[tokio::test]
async fn creates_and_leases_canonical_handoffs() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "gateway_create_public_session_share_handoff",
        json!({
            "p_public_slug": PUBLIC_SLUG,
            "p_source_hash": test_source_hash(Some("203.0.113.8"))
        }),
        json!([{
            "request_id": HANDOFF_ID,
            "expires_at": "2026-07-16T10:01:00Z"
        }]),
    )
    .await;
    let mut claimed_row = snapshot_row("Claimed note");
    claimed_row["lease_expires_at"] = json!("2099-07-16T10:20:00Z");
    mount_rpc(
        &server,
        "gateway_lease_session_share_handoff",
        json!({ "p_request_id": HANDOFF_ID, "p_lease_id": LEASE_ID }),
        json!([claimed_row]),
    )
    .await;

    let create_response = test_router(&server)
        .oneshot(
            Request::post(format!("/shared-notes/public/{PUBLIC_SLUG}/handoff"))
                .header(FLY_CLIENT_IP_HEADER, "203.0.113.8")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::OK);
    let handoff = response_json(create_response).await;
    assert_eq!(handoff["requestId"], HANDOFF_ID);
    assert_eq!(handoff["expiresAt"], "2026-07-16T10:01:00Z");

    let claim_response = test_router(&server)
        .oneshot(
            Request::post("/shared-notes/handoffs/claim")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "requestId": HANDOFF_ID, "leaseId": LEASE_ID }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(claim_response.status(), StatusCode::OK);
    let claimed = response_json(claim_response).await;
    assert_eq!(claimed["title"], "Claimed note");
    assert_eq!(claimed["leaseExpiresAt"], "2099-07-16T10:20:00Z");
}

#[tokio::test]
async fn claims_manifest_without_signing_and_grants_one_attachment_just_in_time() {
    let server = MockServer::start().await;
    let object_key = format!("{OWNER_ID}/{SHARE_ID}/{ATTACHMENT_ID}.sna1");
    let attachment = json!({
        "id": ATTACHMENT_ID,
        "filename": "diagram.png",
        "contentType": "image/png",
        "sizeBytes": 1024,
        "sha256": "a".repeat(64)
    });
    let mut claimed_row = snapshot_row("Claimed attachment");
    claimed_row["attachments_json"] = json!([attachment]);
    claimed_row["lease_expires_at"] = json!("2099-07-16T10:20:00Z");
    mount_rpc(
        &server,
        "gateway_lease_session_share_handoff",
        json!({ "p_request_id": HANDOFF_ID, "p_lease_id": LEASE_ID }),
        json!([claimed_row]),
    )
    .await;
    Mock::given(method("POST"))
        .and(path(
            "/rest/v1/rpc/gateway_prepare_session_share_handoff_attachment_download",
        ))
        .and(body_partial_json(json!({
            "p_lease_id": LEASE_ID,
            "p_attachment_id": ATTACHMENT_ID
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "share_id": SHARE_ID,
            "attachment_id": ATTACHMENT_ID,
            "object_key": object_key,
            "filename": "diagram.png",
            "content_type": "image/png",
            "size_bytes": 1024,
            "sha256": "a".repeat(64),
            "access_version": 3,
            "cleanup_not_before": "2026-07-16T10:10:00Z"
        }])))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/storage/v1/object/sign/{SHARED_ATTACHMENT_BUCKET}/{object_key}"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "signedURL": format!(
                "/object/sign/{SHARED_ATTACHMENT_BUCKET}/{object_key}?token=handoff-download"
            )
        })))
        .expect(1)
        .mount(&server)
        .await;

    let response = test_router(&server)
        .oneshot(
            Request::post("/shared-notes/handoffs/claim")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "requestId": HANDOFF_ID, "leaseId": LEASE_ID }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["attachments"], json!([attachment]));
    assert!(body.get("attachmentDownloads").is_none());

    let response = test_router(&server)
        .oneshot(
            Request::post(format!(
                "/shared-notes/handoffs/attachments/{ATTACHMENT_ID}/download"
            ))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "leaseId": LEASE_ID }).to_string()))
            .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["id"], ATTACHMENT_ID);
    assert_eq!(body["filename"], "diagram.png");
    assert!(
        body["signedUrl"]
            .as_str()
            .unwrap()
            .contains("handoff-download")
    );
    assert!(body["expiresAt"].is_string());
    assert!(body.get("objectKey").is_none());
}

#[tokio::test]
async fn creates_link_handoff_without_returning_the_bearer_token() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "gateway_create_session_share_link_handoff",
        json!({
            "p_share_id": SHARE_ID,
            "p_link_token": LINK_TOKEN,
            "p_source_hash": test_source_hash(None)
        }),
        json!([{
            "request_id": HANDOFF_ID,
            "expires_at": "2026-07-16T10:01:00Z"
        }]),
    )
    .await;

    let response = test_router(&server)
        .oneshot(
            Request::post(format!("/shared-notes/link/{SHARE_ID}/handoff"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "token": LINK_TOKEN }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["requestId"], HANDOFF_ID);
    assert!(!body.to_string().contains(LINK_TOKEN));
}

#[tokio::test]
async fn creates_a_stable_link_handoff_without_a_bearer_token() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "gateway_create_stable_session_share_handoff",
        json!({
            "p_share_id": SHARE_ID,
            "p_source_hash": test_source_hash(None)
        }),
        json!([{
            "request_id": HANDOFF_ID,
            "expires_at": "2026-07-16T10:01:00Z"
        }]),
    )
    .await;

    let response = test_router(&server)
        .oneshot(
            Request::post(format!("/shared-notes/share/{SHARE_ID}/handoff"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await["requestId"], HANDOFF_ID);
}

#[test]
fn handoff_source_hashes_are_canonical_and_do_not_expose_client_ips() {
    let canonical = test_source_hash(Some("2001:db8::1"));
    let expanded = test_source_hash(Some("2001:0db8:0000:0000:0000:0000:0000:0001"));
    let other = test_source_hash(Some("2001:db8::2"));

    assert_eq!(canonical, expanded);
    assert_ne!(canonical, other);
    assert_eq!(canonical.len(), 64);
    assert!(canonical.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert!(!canonical.contains("2001"));
}

#[tokio::test]
async fn rejects_invalid_capabilities_before_the_gateway() {
    let server = MockServer::start().await;
    let requests = [
        Request::get("/shared-notes/public/not-a-slug")
            .body(Body::empty())
            .unwrap(),
        Request::post("/shared-notes/link/not-a-uuid")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "token": LINK_TOKEN }).to_string()))
            .unwrap(),
        Request::post(format!("/shared-notes/link/{SHARE_ID}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({ "token": "too-short" }).to_string()))
            .unwrap(),
        Request::post("/shared-notes/handoffs/claim")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({
                    "requestId": "22222222-2222-3222-8222-222222222222",
                    "leaseId": LEASE_ID
                })
                .to_string(),
            ))
            .unwrap(),
    ];

    for request in requests {
        let response = test_router(&server).oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(
            response_json(response).await["error"]["code"],
            "shared_note_not_found"
        );
    }
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn bounds_link_requests_and_redacts_gateway_failures() {
    let server = MockServer::start().await;
    let oversized = test_router(&server)
        .oneshot(
            Request::post(format!("/shared-notes/link/{SHARE_ID}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "token": "x".repeat(2000) }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(oversized.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(oversized.headers()[header::CACHE_CONTROL], "no-store");

    let oversized_claim = test_router(&server)
        .oneshot(
            Request::post("/shared-notes/handoffs/claim")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "requestId": "x".repeat(300) }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(oversized_claim.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(oversized_claim.headers()[header::CACHE_CONTROL], "no-store");

    Mock::given(method("POST"))
        .and(path(
            "/rest/v1/rpc/gateway_read_public_session_share_snapshot_v2",
        ))
        .respond_with(ResponseTemplate::new(403).set_body_string("secret database detail"))
        .mount(&server)
        .await;
    let failed = test_router(&server)
        .oneshot(
            Request::get(format!("/shared-notes/public/{PUBLIC_SLUG}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(failed.status(), StatusCode::BAD_GATEWAY);
    let body = response_json(failed).await.to_string();
    assert!(!body.contains("database detail"));
    assert!(!body.contains("service-role-key"));
}

#[tokio::test]
async fn maps_empty_or_invalid_gateway_rows_without_leaking_details() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "gateway_read_public_session_share_snapshot_v2",
        json!({ "p_public_slug": PUBLIC_SLUG }),
        json!([]),
    )
    .await;
    let missing = test_router(&server)
        .oneshot(
            Request::get(format!("/shared-notes/public/{PUBLIC_SLUG}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);

    let invalid_server = MockServer::start().await;
    mount_rpc(
        &invalid_server,
        "gateway_read_public_session_share_snapshot_v2",
        json!({ "p_public_slug": PUBLIC_SLUG }),
        json!([{
            "share_id": SHARE_ID,
            "schema_version": 1,
            "content_revision": 1,
            "title": "Private metadata",
            "body_json": { "type": "paragraph" },
            "attachments_json": [],
            "published_at": "2026-07-16T10:00:00Z"
        }]),
    )
    .await;
    let invalid = test_router(&invalid_server)
        .oneshot(
            Request::get(format!("/shared-notes/public/{PUBLIC_SLUG}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(
        response_json(invalid).await["error"]["code"],
        "shared_note_service_unavailable"
    );
}

#[tokio::test]
async fn does_not_follow_gateway_redirects_with_service_credentials() {
    let redirect_target = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([snapshot_row("Leak")])))
        .mount(&redirect_target)
        .await;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(
            "/rest/v1/rpc/gateway_read_public_session_share_snapshot_v2",
        ))
        .respond_with(
            ResponseTemplate::new(307)
                .insert_header("location", format!("{}/steal", redirect_target.uri())),
        )
        .mount(&server)
        .await;

    let response = test_router(&server)
        .oneshot(
            Request::get(format!("/shared-notes/public/{PUBLIC_SLUG}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert!(
        redirect_target
            .received_requests()
            .await
            .unwrap()
            .is_empty()
    );
}
