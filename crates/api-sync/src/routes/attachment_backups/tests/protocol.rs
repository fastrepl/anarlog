use super::*;

#[tokio::test]
async fn requires_pro_and_rejects_bad_inputs_before_upstream_calls() {
    let server = MockServer::start().await;
    let non_pro = test_router(&server, false)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/download",
            json!({ "objectKey": object_key() }),
        ))
        .await
        .unwrap();
    assert_eq!(non_pro.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response_json(non_pro).await["error"]["code"],
        "subscription_required"
    );

    for path in [
        "/attachment-backups/delete",
        "/attachment-backups/delete/cancel",
    ] {
        let non_pro_delete = test_router(&server, false)
            .oneshot(json_request(Method::POST, path, delete_request_body()))
            .await
            .unwrap();
        assert_eq!(non_pro_delete.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response_json(non_pro_delete).await["error"]["code"],
            "subscription_required"
        );
    }

    let invalid_key = test_router(&server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/download",
            json!({ "objectKey": "../other-user/object" }),
        ))
        .await
        .unwrap();
    assert_eq!(invalid_key.status(), StatusCode::BAD_REQUEST);

    let invalid_hash = test_router(&server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/upload-grant",
            json!({ "objectKey": object_key(), "ciphertextSha256": "ABC" }),
        ))
        .await
        .unwrap();
    assert_eq!(invalid_hash.status(), StatusCode::BAD_REQUEST);

    let unknown_field = test_router(&server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/delete",
            json!({
                "objectKey": object_key(),
                "attachmentRef": ATTACHMENT_REF,
                "versionRef": VERSION_REF,
                "deleteRequestId": DELETE_REQUEST_ID,
                "ownerUserId": "attacker"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(unknown_field.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let noncanonical_request_id = test_router(&server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/delete/cancel",
            json!({
                "objectKey": object_key(),
                "attachmentRef": ATTACHMENT_REF,
                "versionRef": VERSION_REF,
                "deleteRequestId": "{44444444-4444-4444-8444-444444444444}"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(noncanonical_request_id.status(), StatusCode::BAD_REQUEST);
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn rejects_oversized_or_origin_injecting_ledger_responses() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "read_current_attachment_backup",
        ResponseTemplate::new(200).set_body_string("x".repeat(MAX_BACKUP_RPC_RESPONSE_BYTES + 1)),
    )
    .await;
    let oversized = test_router(&server, true)
        .oneshot(
            Request::get(format!("/attachment-backups/head/{ATTACHMENT_REF}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(oversized.status(), StatusCode::BAD_GATEWAY);

    let second_server = MockServer::start().await;
    mount_rpc(
        &second_server,
        "read_current_attachment_backup",
        ResponseTemplate::new(200).set_body_json(json!([{
            "object_id": OBJECT_ID,
            "version_ref": VERSION_REF,
            "object_key": "https://attacker.example/secret",
            "ciphertext_sha256": CIPHERTEXT_SHA256,
            "ciphertext_size_bytes": 1234,
            "format_version": 1
        }])),
    )
    .await;
    let malformed = test_router(&second_server, true)
        .oneshot(
            Request::get(format!("/attachment-backups/head/{ATTACHMENT_REF}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(malformed.status(), StatusCode::BAD_GATEWAY);
    assert!(
        !response_json(malformed)
            .await
            .to_string()
            .contains("attacker.example")
    );
}
