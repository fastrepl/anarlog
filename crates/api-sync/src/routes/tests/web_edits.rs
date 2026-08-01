use super::*;

#[tokio::test]
async fn saves_an_explicit_editor_web_edit_without_a_pro_entitlement() {
    let server = MockServer::start().await;
    let share_id = "11111111-1111-4111-8111-111111111111";
    let mutation_id = "22222222-2222-4222-8222-222222222222";
    let body = json!({
        "type": "doc",
        "content": [{
            "type": "heading",
            "attrs": { "level": 1 },
            "content": [{ "type": "text", "text": "Edited note" }]
        }]
    });
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/edit_session_share_snapshot_cas"))
        .and(header("apikey", "service-role-key"))
        .and(header("authorization", "Bearer service-role-key"))
        .and(body_partial_json(json!({
            "p_share_id": share_id,
            "p_actor_user_id": "user-123",
            "p_expected_content_revision": 3,
            "p_mutation_id": mutation_id,
            "p_title": "Edited note",
            "p_body_json": body,
            "p_attachment_ids": []
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "outcome": "applied",
            "share_id": share_id,
            "schema_version": 1,
            "content_revision": 4,
            "title": "Edited note",
            "body_json": body,
            "attachments_json": [],
            "web_editable": true,
            "access_version": 7,
            "published_at": "2026-07-17T10:00:00Z"
        }])))
        .expect(1)
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &[])
        .oneshot(
            Request::put(format!("/shares/{share_id}/web-edit"))
                .header(http_header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "baseRevision": 3,
                        "mutationId": mutation_id,
                        "title": "Edited note",
                        "body": body,
                        "attachmentIds": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[http_header::CACHE_CONTROL], "no-store");
    let response = response_json(response).await;
    assert_eq!(response["contentRevision"], 4);
    assert_eq!(response["webEditable"], true);
    assert_eq!(response["accessVersion"], 7);
}

#[tokio::test]
async fn saves_web_edits_that_preserve_the_attachment_manifest() {
    let server = MockServer::start().await;
    let share_id = "11111111-1111-4111-8111-111111111111";
    let mutation_id = "22222222-2222-4222-8222-222222222222";
    let attachment_id = "33333333-3333-4333-8333-333333333333";
    let body = json!({
        "type": "doc",
        "content": [{
            "type": "fileAttachment",
            "attrs": { "sharedAttachmentId": attachment_id }
        }]
    });
    let attachment = json!({
        "id": attachment_id,
        "filename": "notes.pdf",
        "contentType": "application/pdf",
        "sizeBytes": 2048,
        "sha256": "b".repeat(64)
    });
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/edit_session_share_snapshot_cas"))
        .and(body_partial_json(json!({
            "p_share_id": share_id,
            "p_actor_user_id": "user-123",
            "p_expected_content_revision": 3,
            "p_mutation_id": mutation_id,
            "p_title": "Edited note",
            "p_body_json": body,
            "p_attachment_ids": [attachment_id]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "outcome": "applied",
            "share_id": share_id,
            "schema_version": 1,
            "content_revision": 4,
            "title": "Edited note",
            "body_json": body,
            "attachments_json": [attachment],
            "web_editable": true,
            "access_version": 7,
            "published_at": "2026-07-17T10:00:00Z"
        }])))
        .expect(1)
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &[])
        .oneshot(
            Request::put(format!("/shares/{share_id}/web-edit"))
                .header(http_header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "baseRevision": 3,
                        "mutationId": mutation_id,
                        "title": "Edited note",
                        "body": body,
                        "attachmentIds": [attachment_id]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await["webEditable"], true);
}

#[tokio::test]
async fn rejects_an_oversized_chunked_snapshot_mutation_response() {
    let server = MockServer::start().await;
    let share_id = "11111111-1111-4111-8111-111111111111";
    let mutation_id = "22222222-2222-4222-8222-222222222222";
    let body = json!({
        "type": "doc",
        "content": [{ "type": "paragraph" }]
    });
    let upstream_body = json!([{
        "outcome": "applied",
        "share_id": share_id,
        "schema_version": 1,
        "content_revision": 2,
        "title": "Title",
        "body_json": body,
        "attachments_json": [],
        "web_editable": true,
        "access_version": 1,
        "published_at": "2026-07-17T10:00:00Z",
        "padding": "x".repeat(MAX_SNAPSHOT_RESPONSE_BYTES)
    }])
    .to_string();
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/edit_session_share_snapshot_cas"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("transfer-encoding", "chunked")
                .set_body_raw(upstream_body, "application/json"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &[])
        .oneshot(
            Request::put(format!("/shares/{share_id}/web-edit"))
                .header(http_header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "baseRevision": 1,
                        "mutationId": mutation_id,
                        "title": "Title",
                        "body": body,
                        "attachmentIds": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(
        response_json(response).await["error"]["code"],
        "shared_note_service_unavailable"
    );
}

#[tokio::test]
async fn maps_a_stale_web_edit_to_a_redacted_conflict_snapshot() {
    let server = MockServer::start().await;
    let share_id = "11111111-1111-4111-8111-111111111111";
    let mutation_id = "22222222-2222-4222-8222-222222222222";
    let draft = json!({
        "type": "doc",
        "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Draft" }] }]
    });
    let current = json!({
        "type": "doc",
        "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Current" }] }]
    });
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/edit_session_share_snapshot_cas"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "outcome": "conflict",
            "share_id": share_id,
            "schema_version": 1,
            "content_revision": 5,
            "title": "Current",
            "body_json": current,
            "attachments_json": [],
            "web_editable": true,
            "access_version": 9,
            "published_at": "2026-07-17T10:05:00Z"
        }])))
        .expect(1)
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &[])
        .oneshot(
            Request::put(format!("/shares/{share_id}/web-edit"))
                .header(http_header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "baseRevision": 4,
                        "mutationId": mutation_id,
                        "title": "Draft",
                        "body": draft,
                        "attachmentIds": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(response.headers()[http_header::CACHE_CONTROL], "no-store");
    let response = response_json(response).await;
    assert_eq!(response["code"], "snapshot_conflict");
    assert_eq!(response["snapshot"]["contentRevision"], 5);
    assert_eq!(response["snapshot"]["body"], current);
}

#[tokio::test]
async fn rejects_legacy_payloads_on_the_web_edit_route() {
    let server = MockServer::start().await;
    let response = test_router(&server, "issuer-key", &[])
        .oneshot(
            Request::put("/shares/11111111-1111-4111-8111-111111111111/web-edit")
                .header(http_header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "title": "Legacy",
                        "body": { "type": "doc", "content": [{ "type": "paragraph" }] },
                        "attachmentIds": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn rejects_a_lossy_web_edit_before_calling_supabase() {
    let server = MockServer::start().await;
    let response = test_router(&server, "issuer-key", &[])
        .oneshot(
            Request::put("/shares/11111111-1111-4111-8111-111111111111/web-edit")
                .header(http_header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "baseRevision": 1,
                        "mutationId": "22222222-2222-4222-8222-222222222222",
                        "title": "Unsupported",
                        "body": {
                            "type": "doc",
                            "content": [{ "type": "privateNode", "attrs": { "secret": "value" } }]
                        },
                        "attachmentIds": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn maps_revoked_editor_denial_without_leaking_database_details() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/edit_session_share_snapshot_cas"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "code": "42501",
            "message": "revoked editor secret detail"
        })))
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &[])
        .oneshot(
            Request::put("/shares/11111111-1111-4111-8111-111111111111/web-edit")
                .header(http_header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "baseRevision": 1,
                        "mutationId": "22222222-2222-4222-8222-222222222222",
                        "title": "Title",
                        "body": { "type": "doc", "content": [{ "type": "paragraph" }] },
                        "attachmentIds": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = response_json(response).await.to_string();
    assert!(body.contains("shared_note_publication_forbidden"));
    assert!(!body.contains("revoked editor secret detail"));
    assert!(!body.contains("service-role-key"));
}
