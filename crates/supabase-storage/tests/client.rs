use supabase_storage::{Error, SupabaseStorage};
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn storage(server: &MockServer) -> SupabaseStorage {
    SupabaseStorage::new(reqwest::Client::new(), &server.uri(), "service-secret")
}

#[tokio::test]
async fn creates_an_origin_and_path_bound_signed_download() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(
            "/storage/v1/object/sign/attachment-backups/user-id/object.anb1",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "signedURL": "/object/sign/attachment-backups/user-id/object.anb1?token=download-token"
        })))
        .mount(&server)
        .await;

    let url = storage(&server)
        .create_signed_url("attachment-backups", "user-id/object.anb1", 300)
        .await
        .unwrap();

    assert_eq!(
        url,
        format!(
            "{}/storage/v1/object/sign/attachment-backups/user-id/object.anb1?token=download-token",
            server.uri()
        )
    );
}

#[tokio::test]
async fn rejects_a_signed_download_for_another_origin_or_object() {
    for returned_url in [
        "https://attacker.example/object?token=stolen",
        "/object/sign/attachment-backups/user-id/other.anb1?token=valid",
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "signedURL": returned_url
            })))
            .mount(&server)
            .await;

        let error = storage(&server)
            .create_signed_url("attachment-backups", "user-id/object.anb1", 300)
            .await
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("not returned by Supabase Storage")
        );
    }
}

#[tokio::test]
async fn bounds_and_redacts_storage_responses() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string("x".repeat(64 * 1024 + 1)))
        .mount(&server)
        .await;

    let error = storage(&server)
        .create_signed_url("attachment-backups", "user-id/object.anb1", 300)
        .await
        .unwrap_err();

    assert!(error.to_string().contains("too large"));
    assert!(!error.to_string().contains(&"x".repeat(100)));
}

#[tokio::test]
async fn creates_an_immutable_signed_upload() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(
            "/storage/v1/object/upload/sign/attachment-backups/user-id/object.anb1",
        ))
        .and(header("authorization", "Bearer service-secret"))
        .and(header("apikey", "service-secret"))
        .and(body_json(serde_json::json!({})))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "url": "/object/upload/sign/attachment-backups/user-id/object.anb1?token=upload-token"
        })))
        .mount(&server)
        .await;

    let upload = storage(&server)
        .create_signed_upload("attachment-backups", "user-id/object.anb1")
        .await
        .unwrap();

    assert_eq!(upload.token, "upload-token");
    assert_eq!(
        format!("{upload:?}"),
        "SignedUpload { signed_url: \"[REDACTED]\", token: \"[REDACTED]\" }"
    );
    assert_eq!(
        upload.signed_url,
        format!(
            "{}/storage/v1/object/upload/sign/attachment-backups/user-id/object.anb1?token=upload-token",
            server.uri()
        )
    );
}

#[tokio::test]
async fn rejects_a_signed_upload_without_a_token() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "url": "/object/upload/sign/attachment-backups/user-id/object.anb1"
        })))
        .mount(&server)
        .await;

    let error = storage(&server)
        .create_signed_upload("attachment-backups", "user-id/object.anb1")
        .await
        .unwrap_err();

    assert!(error.to_string().contains("token was not returned"));
}

#[tokio::test]
async fn rejects_a_signed_upload_from_an_unexpected_origin() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "url": "https://attacker.example/upload?token=stolen"
        })))
        .mount(&server)
        .await;

    let error = storage(&server)
        .create_signed_upload("attachment-backups", "user-id/object.anb1")
        .await
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("not returned by Supabase Storage")
    );
}

#[tokio::test]
async fn reads_storage_object_metadata() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(
            "/storage/v1/object/info/attachment-backups/user-id/object.anb1",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "object-id",
            "size": 6291456,
            "content_type": "application/octet-stream",
            "metadata": {
                "ciphertextSha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "formatVersion": 1
            }
        })))
        .mount(&server)
        .await;

    let info = storage(&server)
        .object_info("attachment-backups", "user-id/object.anb1")
        .await
        .unwrap();

    assert_eq!(info.size_bytes, 6_291_456);
    assert_eq!(info.content_type, "application/octet-stream");
    assert_eq!(info.ciphertext_sha256, "a".repeat(64));
    assert_eq!(info.format_version, 1);
}

#[tokio::test]
async fn reads_legacy_storage_object_metadata() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "metadata": {
                "size": 42,
                "mimetype": "application/octet-stream"
            },
            "user_metadata": {
                "ciphertextSha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "formatVersion": 1
            }
        })))
        .mount(&server)
        .await;

    let info = storage(&server)
        .object_info("attachment-backups", "user-id/object.anb1")
        .await
        .unwrap();

    assert_eq!(info.size_bytes, 42);
    assert_eq!(info.content_type, "application/octet-stream");
    assert_eq!(info.ciphertext_sha256, "a".repeat(64));
    assert_eq!(info.format_version, 1);
}

#[tokio::test]
async fn reads_storage_object_metadata_split_across_response_fields() {
    for metadata in [
        serde_json::json!({
            "size": 42,
            "content_type": "application/octet-stream",
            "metadata": {
                "ciphertextSha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            },
            "user_metadata": {
                "formatVersion": 1
            }
        }),
        serde_json::json!({
            "size": 42,
            "content_type": "application/octet-stream",
            "metadata": {
                "formatVersion": 1
            },
            "user_metadata": {
                "ciphertextSha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            }
        }),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(metadata))
            .mount(&server)
            .await;

        let info = storage(&server)
            .object_info("attachment-backups", "user-id/object.anb1")
            .await
            .unwrap();

        assert_eq!(info.ciphertext_sha256, "a".repeat(64));
        assert_eq!(info.format_version, 1);
    }
}

#[tokio::test]
async fn rejects_missing_or_malformed_private_object_metadata() {
    for user_metadata in [
        serde_json::json!(null),
        serde_json::json!({
            "ciphertextSha256": "not-a-checksum",
            "formatVersion": 1
        }),
        serde_json::json!({
            "ciphertextSha256": "a".repeat(64),
            "formatVersion": 2
        }),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "size": 42,
                "content_type": "application/octet-stream",
                "metadata": user_metadata
            })))
            .mount(&server)
            .await;

        let error = storage(&server)
            .object_info("attachment-backups", "user-id/object.anb1")
            .await
            .unwrap_err();

        assert!(error.to_string().contains("storage object"));
    }
}

#[tokio::test]
async fn streams_a_trusted_storage_object_checksum() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(
            "/storage/v1/object/authenticated/attachment-backups/user-id/object.anb1",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![0_u8; 1_234]))
        .mount(&server)
        .await;

    let checksum = storage(&server)
        .object_sha256("attachment-backups", "user-id/object.anb1", 1_234)
        .await
        .unwrap();

    assert_eq!(
        checksum,
        "ad47fd9e87159d651a53b3dfba3ef200684a9ed88c2528b62e18f3881fe203b0"
    );
}

#[tokio::test]
async fn rejects_storage_object_size_mismatches_while_hashing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes([1_u8, 2, 3]))
        .mount(&server)
        .await;

    let error = storage(&server)
        .object_sha256("attachment-backups", "user-id/object.anb1", 4)
        .await
        .unwrap_err();

    assert!(error.to_string().contains("size did not match"));
}

#[tokio::test]
async fn percent_encodes_object_path_segments() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path(
            "/storage/v1/object/attachment-backups/user-id/object%20name.anb1",
        ))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    storage(&server)
        .delete_file("attachment-backups", "user-id/object name.anb1")
        .await
        .unwrap();
}

#[tokio::test]
async fn deleting_a_missing_object_is_idempotent() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "statusCode": "404",
            "code": "NoSuchKey",
            "message": "Object not found"
        })))
        .mount(&server)
        .await;

    storage(&server)
        .delete_file("attachment-backups", "user-id/missing.anb1")
        .await
        .unwrap();
}

#[tokio::test]
async fn does_not_hide_other_not_found_errors() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "statusCode": "404",
            "code": "NoSuchBucket",
            "message": "Bucket not found"
        })))
        .mount(&server)
        .await;

    let error = storage(&server)
        .delete_file("attachment-backups", "user-id/object.anb1")
        .await
        .unwrap_err();

    assert!(error.to_string().contains("failed to delete file: 404"));
    assert!(!error.to_string().contains("Bucket not found"));
}

#[tokio::test]
async fn rejects_traversal_before_sending_a_request() {
    let server = MockServer::start().await;
    let error = storage(&server)
        .delete_file("attachment-backups", "user-id/../object.anb1")
        .await
        .unwrap_err();

    assert!(matches!(error, Error::InvalidPath));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn clears_nested_prefixes_through_bounded_storage_api_calls() {
    let server = MockServer::start().await;
    let list_path = "/storage/v1/object/list/audio-files";
    let root_body = serde_json::json!({
        "prefix": "user-id/",
        "limit": 100,
        "offset": 0,
        "sortBy": { "column": "name", "order": "asc" }
    });
    let child_body = serde_json::json!({
        "prefix": "user-id/nested/",
        "limit": 100,
        "offset": 0,
        "sortBy": { "column": "name", "order": "asc" }
    });
    Mock::given(method("POST"))
        .and(path(list_path))
        .and(body_json(root_body.clone()))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                "name": "nested",
                "id": null,
                "updated_at": null,
                "created_at": null,
                "last_accessed_at": null,
                "metadata": null
            }])),
        )
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(list_path))
        .and(body_json(root_body))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .with_priority(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(list_path))
        .and(body_json(child_body.clone()))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                "name": "recording.wav",
                "id": "object-id",
                "updated_at": "2026-07-17T00:00:00Z",
                "created_at": "2026-07-17T00:00:00Z",
                "last_accessed_at": "2026-07-17T00:00:00Z",
                "metadata": { "size": 42 }
            }])),
        )
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(list_path))
        .and(body_json(child_body))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .with_priority(2)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/storage/v1/object/audio-files"))
        .and(body_json(serde_json::json!({
            "prefixes": ["user-id/nested/recording.wav"]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(1)
        .mount(&server)
        .await;

    let deleted = storage(&server)
        .clear_prefix("audio-files", "user-id/", 10)
        .await
        .unwrap();

    assert_eq!(deleted, 1);
}

#[tokio::test]
async fn rejects_malformed_or_traversing_storage_list_entries() {
    for object in [
        serde_json::json!({
            "name": "..",
            "id": null,
            "updated_at": null,
            "created_at": null,
            "last_accessed_at": null,
            "metadata": null
        }),
        serde_json::json!({
            "name": "ambiguous.wav",
            "id": "object-id",
            "updated_at": null,
            "created_at": null,
            "last_accessed_at": null,
            "metadata": null
        }),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([object])))
            .mount(&server)
            .await;

        let error = storage(&server)
            .clear_prefix("audio-files", "user-id/", 10)
            .await
            .unwrap_err();

        assert!(error.to_string().contains("invalid object"));
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
}

#[tokio::test]
async fn fails_closed_when_prefix_cleanup_exceeds_its_limit() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "name": "one.wav",
                "id": "one",
                "updated_at": "2026-07-17T00:00:00Z",
                "created_at": "2026-07-17T00:00:00Z",
                "last_accessed_at": "2026-07-17T00:00:00Z",
                "metadata": {}
            },
            {
                "name": "two.wav",
                "id": "two",
                "updated_at": "2026-07-17T00:00:00Z",
                "created_at": "2026-07-17T00:00:00Z",
                "last_accessed_at": "2026-07-17T00:00:00Z",
                "metadata": {}
            }
        ])))
        .mount(&server)
        .await;

    let error = storage(&server)
        .clear_prefix("audio-files", "user-id/", 1)
        .await
        .unwrap_err();

    assert!(error.to_string().contains("cleanup limit"));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn propagates_storage_list_failures_without_deleting() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503).set_body_json(serde_json::json!({
            "message": "unavailable"
        })))
        .mount(&server)
        .await;

    let error = storage(&server)
        .clear_prefix("audio-files", "user-id/", 10)
        .await
        .unwrap_err();

    assert!(error.to_string().contains("failed to list folder: 503"));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}
