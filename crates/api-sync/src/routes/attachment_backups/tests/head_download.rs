use super::*;

#[tokio::test]
async fn reports_head_cas_conflict_without_leaking_upstream_details() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "read_attachment_backup_by_key",
        ResponseTemplate::new(200)
            .set_body_json(json!([object_row("ready", Some(CIPHERTEXT_SHA256))])),
    )
    .await;
    mount_rpc(
        &server,
        "promote_attachment_backup",
        ResponseTemplate::new(409).set_body_json(json!({
            "code": "40001",
            "message": "current-key-secret"
        })),
    )
    .await;

    let response = test_router(&server, true)
        .oneshot(json_request(
            Method::PUT,
            "/attachment-backups/head",
            json!({
                "objectKey": object_key(),
                "expectedCurrentObjectKey": displaced_key()
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = response_json(response).await;
    assert_eq!(body["error"]["code"], "attachment_backup_conflict");
    assert!(!body.to_string().contains("current-key-secret"));
}

#[tokio::test]
async fn returns_version_and_integrity_for_current_and_promoted_heads() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "read_current_attachment_backup",
        ResponseTemplate::new(200).set_body_json(json!([{
            "object_id": OBJECT_ID,
            "version_ref": VERSION_REF,
            "object_key": object_key(),
            "ciphertext_sha256": CIPHERTEXT_SHA256,
            "ciphertext_size_bytes": 1234,
            "format_version": 1
        }])),
    )
    .await;
    let current = test_router(&server, true)
        .oneshot(
            Request::get(format!("/attachment-backups/head/{ATTACHMENT_REF}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(current.status(), StatusCode::OK);
    let body = response_json(current).await;
    assert_eq!(body["versionRef"], VERSION_REF);
    assert_eq!(body["ciphertextSha256"], CIPHERTEXT_SHA256);

    let promote_server = MockServer::start().await;
    mount_rpc(
        &promote_server,
        "read_attachment_backup_by_key",
        ResponseTemplate::new(200)
            .set_body_json(json!([object_row("ready", Some(CIPHERTEXT_SHA256))])),
    )
    .await;
    mount_rpc(
        &promote_server,
        "promote_attachment_backup",
        ResponseTemplate::new(200).set_body_json(json!([{
            "current_object_id": OBJECT_ID,
            "current_object_key": object_key(),
            "current_version_ref": VERSION_REF,
            "current_ciphertext_sha256": CIPHERTEXT_SHA256,
            "displaced_object_id": null,
            "displaced_object_key": null,
            "was_promoted": true
        }])),
    )
    .await;
    let promoted = test_router(&promote_server, true)
        .oneshot(json_request(
            Method::PUT,
            "/attachment-backups/head",
            json!({ "objectKey": object_key(), "expectedCurrentObjectKey": null }),
        ))
        .await
        .unwrap();
    assert_eq!(promoted.status(), StatusCode::OK);
    let body = response_json(promoted).await;
    assert_eq!(body["currentVersionRef"], VERSION_REF);
    assert_eq!(body["currentCiphertextSha256"], CIPHERTEXT_SHA256);
}

#[tokio::test]
async fn prepares_current_download_before_signing() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "prepare_attachment_backup_download",
        ResponseTemplate::new(200).set_body_json(json!([{
            "object_id": OBJECT_ID,
            "object_key": object_key(),
            "ciphertext_sha256": CIPHERTEXT_SHA256,
            "ciphertext_size_bytes": 1234,
            "format_version": 1,
            "cleanup_not_before": timestamp_after(60 * 60)
        }])),
    )
    .await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/storage/v1/object/sign/{ATTACHMENT_BACKUP_BUCKET}/{}",
            object_key()
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "signedURL": format!(
                "/object/sign/{ATTACHMENT_BACKUP_BUCKET}/{}?token=download-secret",
                object_key()
            )
        })))
        .mount(&server)
        .await;

    let response = test_router(&server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/download",
            json!({ "objectKey": object_key() }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["objectId"], OBJECT_ID);
    assert_eq!(body["ciphertextSha256"], CIPHERTEXT_SHA256);
    assert!(
        body["signedUrl"]
            .as_str()
            .unwrap()
            .contains("download-secret")
    );
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].url.path(),
        "/rest/v1/rpc/prepare_attachment_backup_download"
    );
    assert!(requests[1].url.path().contains("/storage/v1/object/sign/"));
}

#[tokio::test]
async fn never_signs_a_download_for_an_unsafe_cleanup_window() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "prepare_attachment_backup_download",
        ResponseTemplate::new(200).set_body_json(json!([{
            "object_id": OBJECT_ID,
            "object_key": object_key(),
            "ciphertext_sha256": CIPHERTEXT_SHA256,
            "ciphertext_size_bytes": 1234,
            "format_version": 1,
            "cleanup_not_before": timestamp_after(DOWNLOAD_URL_TTL_SECONDS + 60)
        }])),
    )
    .await;

    let response = test_router(&server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/download",
            json!({ "objectKey": object_key() }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert!(
        !requests
            .iter()
            .any(|request| request.url.path().contains("/storage/v1/"))
    );
}

#[tokio::test]
async fn never_signs_a_noncurrent_download() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "prepare_attachment_backup_download",
        ResponseTemplate::new(409).set_body_json(json!({
            "code": "55000",
            "message": "not current"
        })),
    )
    .await;
    let response = test_router(&server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/download",
            json!({ "objectKey": object_key() }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}
