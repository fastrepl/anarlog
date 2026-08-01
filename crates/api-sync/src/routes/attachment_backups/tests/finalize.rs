use super::*;

#[tokio::test]
async fn verifies_storage_metadata_before_finalizing() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "read_attachment_backup_by_key",
        ResponseTemplate::new(200)
            .set_body_json(json!([object_row("reserved", Some(CIPHERTEXT_SHA256))])),
    )
    .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/storage/v1/object/info/{ATTACHMENT_BACKUP_BUCKET}/{}",
            object_key()
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1234,
            "content_type": "application/octet-stream",
            "metadata": {
                "ciphertextSha256": CIPHERTEXT_SHA256,
                "formatVersion": 1
            }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/storage/v1/object/authenticated/{ATTACHMENT_BACKUP_BUCKET}/{}",
            object_key()
        )))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![0_u8; 1_234]))
        .mount(&server)
        .await;
    mount_rpc(
        &server,
        "finalize_attachment_backup",
        ResponseTemplate::new(200).set_body_json(json!([{
            "object_id": OBJECT_ID,
            "object_key": object_key(),
            "object_state": "ready",
            "was_finalized": true
        }])),
    )
    .await;

    let response = test_router(&server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/finalize",
            json!({ "objectKey": object_key() }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await["objectState"], "ready");
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 4);
    assert_eq!(
        requests[0].url.path(),
        "/rest/v1/rpc/read_attachment_backup_by_key"
    );
    assert!(requests[1].url.path().contains("/storage/v1/object/info/"));
    assert!(
        requests[2]
            .url
            .path()
            .contains("/storage/v1/object/authenticated/")
    );
    assert_eq!(
        requests[3].url.path(),
        "/rest/v1/rpc/finalize_attachment_backup"
    );
}

#[tokio::test]
async fn rejects_expired_finalize_before_storage_verification() {
    let server = MockServer::start().await;
    let mut row = object_row("reserved", Some(CIPHERTEXT_SHA256));
    row["reservation_expires_at"] = json!(timestamp_after(-3 * 60 * 60));
    row["upload_expires_at"] = json!(timestamp_after(-2 * 60 * 60));
    row["cleanup_not_before"] = json!(timestamp_after(-60 * 60));
    mount_rpc(
        &server,
        "read_attachment_backup_by_key",
        ResponseTemplate::new(200).set_body_json(json!([row])),
    )
    .await;

    let response = test_router(&server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/finalize",
            json!({ "objectKey": object_key() }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(response).await["error"]["code"],
        "attachment_backup_conflict"
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn accepts_finalize_retry_after_the_candidate_is_current() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "read_attachment_backup_by_key",
        ResponseTemplate::new(200)
            .set_body_json(json!([object_row("current", Some(CIPHERTEXT_SHA256))])),
    )
    .await;
    let response = test_router(&server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/finalize",
            json!({ "objectKey": object_key() }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["objectState"], "current");
    assert_eq!(body["wasFinalized"], false);
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn rejects_storage_integrity_mismatch_without_finalizing() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "read_attachment_backup_by_key",
        ResponseTemplate::new(200)
            .set_body_json(json!([object_row("reserved", Some(CIPHERTEXT_SHA256))])),
    )
    .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/storage/v1/object/info/{ATTACHMENT_BACKUP_BUCKET}/{}",
            object_key()
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1234,
            "content_type": "application/octet-stream",
            "metadata": {
                "ciphertextSha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                "formatVersion": 1
            }
        })))
        .mount(&server)
        .await;
    let response = test_router(&server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/finalize",
            json!({ "objectKey": object_key() }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn rejects_storage_content_mismatch_without_finalizing() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "read_attachment_backup_by_key",
        ResponseTemplate::new(200)
            .set_body_json(json!([object_row("reserved", Some(CIPHERTEXT_SHA256))])),
    )
    .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/storage/v1/object/info/{ATTACHMENT_BACKUP_BUCKET}/{}",
            object_key()
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1234,
            "content_type": "application/octet-stream",
            "metadata": {
                "ciphertextSha256": CIPHERTEXT_SHA256,
                "formatVersion": 1
            }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/storage/v1/object/authenticated/{ATTACHMENT_BACKUP_BUCKET}/{}",
            object_key()
        )))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![1_u8; 1_234]))
        .mount(&server)
        .await;

    let response = test_router(&server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/finalize",
            json!({ "objectKey": object_key() }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
}

#[tokio::test]
async fn throttles_parallel_storage_content_verification() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "read_attachment_backup_by_key",
        ResponseTemplate::new(200)
            .set_body_json(json!([object_row("reserved", Some(CIPHERTEXT_SHA256))])),
    )
    .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/storage/v1/object/info/{ATTACHMENT_BACKUP_BUCKET}/{}",
            object_key()
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "size": 1234,
            "content_type": "application/octet-stream",
            "metadata": {
                "ciphertextSha256": CIPHERTEXT_SHA256,
                "formatVersion": 1
            }
        })))
        .mount(&server)
        .await;
    let state = test_state(&server);
    let _verification_slot = state
        .attachment_verification_slots
        .clone()
        .try_acquire_owned()
        .unwrap();

    let response = test_router_with_state(state, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/finalize",
            json!({ "objectKey": object_key() }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn rejects_spoofed_storage_metadata_when_object_bytes_do_not_match() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "read_attachment_backup_by_key",
        ResponseTemplate::new(200)
            .set_body_json(json!([object_row("reserved", Some(CIPHERTEXT_SHA256))])),
    )
    .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/storage/v1/object/info/{ATTACHMENT_BACKUP_BUCKET}/{}",
            object_key()
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "metadata": { "size": 1234, "mimetype": "application/octet-stream" },
            "user_metadata": {
                "ciphertextSha256": CIPHERTEXT_SHA256,
                "formatVersion": 1
            }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/storage/v1/object/authenticated/{ATTACHMENT_BACKUP_BUCKET}/{}",
            object_key()
        )))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![1; 1234]))
        .mount(&server)
        .await;

    let response = test_router(&server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/finalize",
            json!({ "objectKey": object_key() }),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
}

#[tokio::test]
async fn reports_storage_verification_failures_as_unavailable() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "read_attachment_backup_by_key",
        ResponseTemplate::new(200)
            .set_body_json(json!([object_row("reserved", Some(CIPHERTEXT_SHA256))])),
    )
    .await;
    Mock::given(method("GET"))
        .and(path(format!(
            "/storage/v1/object/info/{ATTACHMENT_BACKUP_BUCKET}/{}",
            object_key()
        )))
        .respond_with(
            ResponseTemplate::new(500)
                .set_body_json(json!({ "message": "storage-secret-must-not-leak" })),
        )
        .mount(&server)
        .await;

    let response = test_router(&server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/finalize",
            json!({ "objectKey": object_key() }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert!(
        !response_json(response)
            .await
            .to_string()
            .contains("storage-secret")
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}
