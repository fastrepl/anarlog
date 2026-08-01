use super::*;

#[tokio::test]
async fn reserves_identity_without_issuing_a_storage_capability() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "reserve_attachment_backup",
        ResponseTemplate::new(200).set_body_json(json!([reserved_row("reserved", None)])),
    )
    .await;

    let response = test_router(&server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/reserve",
            json!({
                "attachmentRef": ATTACHMENT_REF,
                "versionRef": VERSION_REF,
                "ciphertextSizeBytes": 1234,
                "formatVersion": 1
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let body = response_json(response).await;
    assert_eq!(body["objectId"], OBJECT_ID);
    assert_eq!(body["ciphertextSha256"], Value::Null);
    assert!(body.get("uploadToken").is_none());
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].url.path(),
        "/rest/v1/rpc/reserve_attachment_backup"
    );
}

#[tokio::test]
async fn reports_reservation_races_as_conflicts() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "reserve_attachment_backup",
        ResponseTemplate::new(409).set_body_json(json!({
            "code": "40001",
            "message": "reservation-secret-must-not-leak"
        })),
    )
    .await;

    let response = test_router(&server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/reserve",
            json!({
                "attachmentRef": ATTACHMENT_REF,
                "versionRef": VERSION_REF,
                "ciphertextSizeBytes": 1234,
                "formatVersion": 1
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = response_json(response).await;
    assert_eq!(body["error"]["code"], "attachment_backup_conflict");
    assert!(!body.to_string().contains("reservation-secret"));
}

#[tokio::test]
async fn reports_reservation_limit_as_conflict() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "reserve_attachment_backup",
        ResponseTemplate::new(409).set_body_json(json!({
            "code": "55000",
            "message": "reservation-limit-secret-must-not-leak"
        })),
    )
    .await;

    let response = test_router(&server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/reserve",
            json!({
                "attachmentRef": ATTACHMENT_REF,
                "versionRef": VERSION_REF,
                "ciphertextSizeBytes": 1234,
                "formatVersion": 1
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = response_json(response).await;
    assert_eq!(body["error"]["code"], "attachment_backup_conflict");
    assert!(!body.to_string().contains("reservation-limit-secret"));
}
