use super::*;

#[tokio::test]
async fn publishes_only_the_sanitized_snapshot_as_the_authenticated_actor() {
    let server = MockServer::start().await;
    let share_id = "11111111-1111-4111-8111-111111111111";
    let mutation_id = "22222222-2222-4222-8222-222222222222";
    let sanitized_body = json!({
        "type": "doc",
        "content": [
            {
                "type": "paragraph",
                "content": [
                    { "type": "text", "text": "Shared note" },
                    { "type": "text", "text": "Planning" }
                ]
            },
            {
                "type": "paragraph",
                "content": [{ "type": "text", "text": "Attachment omitted" }]
            }
        ]
    });
    Mock::given(method("POST"))
        .and(path(
            "/rest/v1/rpc/publish_session_share_snapshot_with_preview_cas",
        ))
        .and(header("apikey", "service-role-key"))
        .and(header("authorization", "Bearer service-role-key"))
        .and(body_partial_json(json!({
            "p_share_id": share_id,
            "p_actor_user_id": "user-123",
            "p_expected_content_revision": 1,
            "p_mutation_id": mutation_id,
            "p_title": "Quarterly plan",
            "p_body_json": sanitized_body,
            "p_attachment_ids": [],
            "p_web_editable": false,
            "p_participants": ["John Jeong", "Sungbin Jo"],
            "p_meeting_at": "2026-08-06T01:30:00+00:00"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "outcome": "applied",
            "share_id": share_id,
            "schema_version": 1,
            "content_revision": 2,
            "title": "Quarterly plan",
            "body_json": sanitized_body,
            "attachments_json": [],
            "web_editable": false,
            "access_version": 4,
            "published_at": "2026-07-16T10:00:00Z"
        }])))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path(
            "/rest/v1/rpc/publish_session_share_snapshot_with_attachments",
        ))
        .and(header("apikey", "service-role-key"))
        .and(header("authorization", "Bearer service-role-key"))
        .and(body_partial_json(json!({
            "p_share_id": share_id,
            "p_actor_user_id": "user-123",
            "p_title": "Quarterly plan",
            "p_body_json": sanitized_body,
            "p_attachment_ids": []
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "share_id": share_id,
            "schema_version": 1,
            "content_revision": 3,
            "title": "Quarterly plan",
            "body_json": sanitized_body,
            "attachments_json": [],
            "published_at": "2026-07-16T10:01:00Z"
        }])))
        .expect(1)
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(put_json(
            &format!("/shares/{share_id}/snapshot"),
            json!({
                "baseRevision": 1,
                "mutationId": mutation_id,
                "title": "  Quarterly plan  ",
                "body": {
                    "type": "doc",
                    "attrs": { "workspaceId": "private-workspace" },
                    "content": [
                        {
                            "type": "paragraph",
                            "content": [
                                { "type": "text", "text": "Shared note" },
                                {
                                    "type": "mention-@",
                                    "attrs": {
                                        "id": "private-mention-id",
                                        "type": "session",
                                        "label": "Planning"
                                    }
                                }
                            ]
                        },
                        {
                            "type": "image",
                            "attrs": {
                                "src": "asset://localhost/Users/alice/secret.png",
                                "attachmentId": "private-attachment-id"
                            }
                        }
                    ]
                },
                "participants": [" John   Jeong ", "Sungbin Jo"],
                "meetingAt": "2026-08-06T01:30:00Z",
                "attachmentIds": []
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[http_header::CACHE_CONTROL], "no-store");
    let body = response_json(response).await;
    assert_eq!(body["shareId"], share_id);
    assert_eq!(body["schemaVersion"], 1);
    assert_eq!(body["contentRevision"], 2);
    assert_eq!(body["title"], "Quarterly plan");
    assert_eq!(body["body"], sanitized_body);
    assert_eq!(body["webEditable"], false);
    assert_eq!(body["accessVersion"], 4);

    let requests = server.received_requests().await.unwrap();
    let published = String::from_utf8(requests[0].body.clone()).unwrap();
    assert!(!published.contains("private-workspace"));
    assert!(!published.contains("private-mention-id"));
    assert!(!published.contains("/Users/alice"));
    assert!(!published.contains("private-attachment-id"));
    assert!(!published.contains("supabase-token"));

    let explicit_empty_response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(put_json(
            &format!("/shares/{share_id}/snapshot"),
            json!({
                "title": "Quarterly plan",
                "body": sanitized_body,
                "attachmentIds": []
            }),
        ))
        .await
        .unwrap();

    assert_eq!(explicit_empty_response.status(), StatusCode::OK);
    let legacy_body = response_json(explicit_empty_response).await;
    let legacy_object = legacy_body.as_object().unwrap();
    assert_eq!(legacy_object.len(), 7);
    for key in [
        "shareId",
        "schemaVersion",
        "contentRevision",
        "title",
        "body",
        "attachments",
        "publishedAt",
    ] {
        assert!(legacy_object.contains_key(key));
    }
    assert!(!legacy_object.contains_key("webEditable"));
    assert!(!legacy_object.contains_key("accessVersion"));
}

#[tokio::test]
async fn publishes_lossless_attachment_snapshots_as_web_editable() {
    let server = MockServer::start().await;
    let share_id = "11111111-1111-4111-8111-111111111111";
    let mutation_id = "22222222-2222-4222-8222-222222222222";
    let attachment_id = "33333333-3333-4333-8333-333333333333";
    let body = json!({
        "type": "doc",
        "content": [{
            "type": "image",
            "attrs": { "sharedAttachmentId": attachment_id }
        }]
    });
    let attachment = json!({
        "id": attachment_id,
        "filename": "diagram.png",
        "contentType": "image/png",
        "sizeBytes": 1024,
        "sha256": "a".repeat(64)
    });
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/publish_session_share_snapshot_cas"))
        .and(body_partial_json(json!({
            "p_share_id": share_id,
            "p_actor_user_id": "user-123",
            "p_expected_content_revision": 1,
            "p_mutation_id": mutation_id,
            "p_title": "Diagram",
            "p_body_json": body,
            "p_attachment_ids": [attachment_id],
            "p_web_editable": true
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "outcome": "applied",
            "share_id": share_id,
            "schema_version": 1,
            "content_revision": 2,
            "title": "Diagram",
            "body_json": body,
            "attachments_json": [attachment],
            "web_editable": true,
            "access_version": 4,
            "published_at": "2026-07-17T10:00:00Z"
        }])))
        .expect(1)
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(put_json(
            &format!("/shares/{share_id}/snapshot"),
            json!({
                "baseRevision": 1,
                "mutationId": mutation_id,
                "title": "Diagram",
                "body": body,
                "attachmentIds": [attachment_id]
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await["webEditable"], true);
}

#[tokio::test]
async fn rejects_invalid_snapshot_requests_before_calling_supabase() {
    let cases = [
        (
            "invalid share id",
            "/shares/not-a-uuid/snapshot".to_string(),
            json!({
                "baseRevision": 1,
                "mutationId": "22222222-2222-4222-8222-222222222222",
                "title": "Title",
                "body": { "type": "doc" }
            }),
        ),
        (
            "invalid document",
            "/shares/11111111-1111-4111-8111-111111111111/snapshot".to_string(),
            json!({
                "baseRevision": 1,
                "mutationId": "22222222-2222-4222-8222-222222222222",
                "title": "Title",
                "body": { "type": "paragraph" }
            }),
        ),
        (
            "missing mutation id",
            "/shares/11111111-1111-4111-8111-111111111111/snapshot".to_string(),
            json!({
                "baseRevision": 1,
                "title": "Title",
                "body": { "type": "doc" }
            }),
        ),
        (
            "missing base revision",
            "/shares/11111111-1111-4111-8111-111111111111/snapshot".to_string(),
            json!({
                "mutationId": "22222222-2222-4222-8222-222222222222",
                "title": "Title",
                "body": { "type": "doc" }
            }),
        ),
        (
            "null CAS fields",
            "/shares/11111111-1111-4111-8111-111111111111/snapshot".to_string(),
            json!({
                "baseRevision": null,
                "mutationId": null,
                "title": "Title",
                "body": { "type": "doc" }
            }),
        ),
        (
            "missing attachment manifest",
            "/shares/11111111-1111-4111-8111-111111111111/snapshot".to_string(),
            json!({
                "baseRevision": 1,
                "mutationId": "22222222-2222-4222-8222-222222222222",
                "title": "Title",
                "body": { "type": "doc", "content": [{ "type": "paragraph" }] }
            }),
        ),
        (
            "null attachment manifest",
            "/shares/11111111-1111-4111-8111-111111111111/snapshot".to_string(),
            json!({
                "baseRevision": 1,
                "mutationId": "22222222-2222-4222-8222-222222222222",
                "title": "Title",
                "body": { "type": "doc", "content": [{ "type": "paragraph" }] },
                "attachmentIds": null
            }),
        ),
    ];

    for (case, path, payload) in cases {
        let server = MockServer::start().await;
        let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
            .oneshot(put_json(&path, payload))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{case}");
        assert!(
            server.received_requests().await.unwrap().is_empty(),
            "{case}"
        );
    }
}

#[tokio::test]
async fn maps_legacy_publication_after_cas_cutover_to_a_redacted_conflict() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(
            "/rest/v1/rpc/publish_session_share_snapshot_with_attachments",
        ))
        .respond_with(ResponseTemplate::new(409).set_body_json(json!({
            "code": "23514",
            "message": "session_share_snapshots_last_mutation_check secret detail"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(put_json(
            "/shares/11111111-1111-4111-8111-111111111111/snapshot",
            json!({
                "title": "Legacy",
                "body": { "type": "doc", "content": [{ "type": "paragraph" }] },
                "attachmentIds": []
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = response_json(response).await.to_string();
    assert!(body.contains("snapshot_conflict"));
    assert!(!body.contains("last_mutation"));
    assert!(!body.contains("secret detail"));
}

#[tokio::test]
async fn rejects_snapshot_publication_without_pro_entitlement() {
    let server = MockServer::start().await;
    let response = test_router(&server, "issuer-key", &[])
        .oneshot(put_json(
            "/shares/11111111-1111-4111-8111-111111111111/snapshot",
            json!({
                "baseRevision": 1,
                "mutationId": "22222222-2222-4222-8222-222222222222",
                "title": "Title",
                "body": { "type": "doc" },
                "attachmentIds": []
            }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response_json(response).await["error"]["code"],
        "subscription_required"
    );
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn maps_snapshot_cas_errors_without_leaking_database_details() {
    for (case, upstream_status, pg_code, secret_message, expected_status, expected_code) in [
        (
            "manager denial",
            403,
            "42501",
            "secret database detail",
            StatusCode::FORBIDDEN,
            "shared_note_publication_forbidden",
        ),
        (
            "missing snapshot conflict",
            409,
            "40001",
            "missing snapshot secret database detail",
            StatusCode::CONFLICT,
            "snapshot_conflict",
        ),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/publish_session_share_snapshot_cas"))
            .respond_with(ResponseTemplate::new(upstream_status).set_body_json(json!({
                "code": pg_code,
                "message": secret_message
            })))
            .expect(1)
            .mount(&server)
            .await;

        let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
            .oneshot(put_json(
                "/shares/11111111-1111-4111-8111-111111111111/snapshot",
                json!({
                    "baseRevision": 1,
                    "mutationId": "22222222-2222-4222-8222-222222222222",
                    "title": "Title",
                    "body": { "type": "doc", "content": [{ "type": "paragraph" }] },
                    "attachmentIds": []
                }),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), expected_status, "{case}");
        let body = response_json(response).await;
        assert_eq!(body["error"]["code"], expected_code, "{case}");
        let body = body.to_string();
        assert!(!body.contains(secret_message), "{case}");
        assert!(!body.contains("missing snapshot"), "{case}");
        assert!(!body.contains("secret database detail"), "{case}");
        assert!(!body.contains("service-role-key"), "{case}");
    }
}
