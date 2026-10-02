use super::*;

#[tokio::test]
async fn reads_then_marks_integrity_before_signing_upload() {
    let server = MockServer::start().await;
    mount_rpc(
        &server,
        "read_attachment_backup_by_key",
        ResponseTemplate::new(200).set_body_json(json!([object_row("reserved", None)])),
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/mark_attachment_backup_signed"))
        .and(body_partial_json(json!({
            "p_owner_user_id": OWNER,
            "p_object_id": OBJECT_ID,
            "p_ciphertext_sha256": CIPHERTEXT_SHA256
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "object_id": OBJECT_ID,
            "object_key": object_key(),
            "ciphertext_sha256": CIPHERTEXT_SHA256,
            "last_signed_at": timestamp_after(-1),
            "upload_expires_at": timestamp_after(2 * 60 * 60 + 30),
            "cleanup_not_before": timestamp_after(27 * 60 * 60)
        }])))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!(
            "/storage/v1/object/upload/sign/{ATTACHMENT_BACKUP_BUCKET}/{}",
            object_key()
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "url": format!(
                "/object/upload/sign/{ATTACHMENT_BACKUP_BUCKET}/{}?token=upload-secret",
                object_key()
            )
        })))
        .mount(&server)
        .await;

    let response = test_router(&server, true)
        .oneshot(json_request(
            Method::POST,
            "/attachment-backups/upload-grant",
            json!({
                "objectKey": object_key(),
                "ciphertextSha256": CIPHERTEXT_SHA256
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["uploadToken"], "upload-secret");
    assert_eq!(body["ciphertextSha256"], CIPHERTEXT_SHA256);

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 3);
    assert!(requests.iter().any(|request| {
        request
            .url
            .path()
            .contains("/storage/v1/object/upload/sign/")
    }));
}

#[tokio::test]
async fn never_signs_upload_when_integrity_mark_is_unsafe_or_fails() {
    for (case, mark_response, expected_status, expected_error_code) in [
        (
            "unsafe cleanup window",
            ResponseTemplate::new(200).set_body_json(json!([{
                "object_id": OBJECT_ID,
                "object_key": object_key(),
                "ciphertext_sha256": CIPHERTEXT_SHA256,
                "last_signed_at": timestamp_after(-1),
                "upload_expires_at": timestamp_after(2 * 60 * 60 + 4 * 60),
                "cleanup_not_before": timestamp_after(26 * 60 * 60 + 6 * 60)
            }])),
            StatusCode::BAD_GATEWAY,
            None,
        ),
        (
            "integrity mark failure",
            ResponseTemplate::new(500).set_body_json(json!({
                "code": "XX000",
                "message": "database-secret-must-not-leak"
            })),
            StatusCode::BAD_GATEWAY,
            None,
        ),
        (
            "integrity mark race",
            ResponseTemplate::new(409).set_body_json(json!({
                "code": "40001",
                "message": "hash-secret-must-not-leak"
            })),
            StatusCode::CONFLICT,
            Some("attachment_backup_conflict"),
        ),
    ] {
        let server = MockServer::start().await;
        mount_rpc(
            &server,
            "read_attachment_backup_by_key",
            ResponseTemplate::new(200).set_body_json(json!([object_row("reserved", None)])),
        )
        .await;
        mount_rpc(&server, "mark_attachment_backup_signed", mark_response).await;

        let response = test_router(&server, true)
            .oneshot(json_request(
                Method::POST,
                "/attachment-backups/upload-grant",
                json!({
                    "objectKey": object_key(),
                    "ciphertextSha256": CIPHERTEXT_SHA256
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), expected_status, "{case}");
        if case == "integrity mark failure" {
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        }
        let body = response_json(response).await;
        if let Some(error_code) = expected_error_code {
            assert_eq!(body["error"]["code"], error_code, "{case}");
        }
        let body = body.to_string();
        assert!(!body.contains("secret"), "{case}");
        assert!(!body.contains("upload-secret"), "{case}");
        assert_no_storage_requests(&server).await;
    }
}
