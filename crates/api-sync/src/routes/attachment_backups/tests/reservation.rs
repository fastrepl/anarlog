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
        .oneshot(reserve_request())
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
    assert_no_storage_requests(&server).await;
}

#[tokio::test]
async fn reports_reservation_conflicts_without_leaking_database_details() {
    for (case, pg_code, secret_message, secret_fragment) in [
        (
            "reservation race",
            "40001",
            "reservation-secret-must-not-leak",
            "reservation-secret",
        ),
        (
            "reservation limit",
            "55000",
            "reservation-limit-secret-must-not-leak",
            "reservation-limit-secret",
        ),
    ] {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "reserve_attachment_backup",
            ResponseTemplate::new(409).set_body_json(json!({
                "code": pg_code,
                "message": secret_message
            })),
        )
        .await;

        let response = test_router(&server, true)
            .oneshot(reserve_request())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT, "{case}");
        let body = response_json(response).await;
        assert_eq!(
            body["error"]["code"], "attachment_backup_conflict",
            "{case}"
        );
        let body = body.to_string();
        assert!(!body.contains(secret_message), "{case}");
        assert!(!body.contains(secret_fragment), "{case}");
        assert_no_storage_requests(&server).await;
    }
}
