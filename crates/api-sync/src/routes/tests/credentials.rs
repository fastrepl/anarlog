use super::*;

#[tokio::test]
async fn issues_replica_credentials_without_contacting_sqlitecloud() {
    let server = MockServer::start().await;
    mock_e2ee_key_claim(&server, TEST_KEY_ID).await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(
            Request::post("/replica/credentials")
                .header(E2EE_KEY_ID_HEADER, TEST_KEY_ID)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[http_header::CACHE_CONTROL], "no-store");
    let body = response_json(response).await;
    assert_eq!(body["transport"], "replica");
    assert_eq!(body["encryptionVersion"], 2);
    assert_eq!(body["encryptionKeyId"], TEST_KEY_ID);
    assert_eq!(body["workspaceId"], "user-123");
    assert_eq!(body["accountUserId"], "user-123");
    assert!(body["expiresAt"].as_str().unwrap().ends_with('Z'));
    assert!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .all(|request| request.url.path() != "/v2/tokens")
    );
}

#[tokio::test]
async fn mints_token_for_verified_supabase_subject() {
    let server = MockServer::start().await;
    mock_workspace_projection(
        &server,
        json!([
            {
                "id": "membership-team",
                "user_id": "user-123",
                "role": "member",
                "created_at": "2026-07-16T09:01:00Z",
                "updated_at": "2026-07-16T10:01:00Z",
                "workspace": {
                    "id": "workspace-team",
                    "owner_user_id": "user-456",
                    "kind": "shared",
                    "name": "Acme",
                    "created_at": "2026-07-16T09:00:00Z",
                    "updated_at": "2026-07-16T10:00:00Z"
                }
            },
            personal_workspace("user-123")
        ]),
    )
    .await;
    mock_e2ee_key_claim(&server, TEST_KEY_ID).await;
    Mock::given(method("POST"))
        .and(path("/v2/tokens"))
        .and(header("authorization", "Bearer issuer-key"))
        .and(body_partial_json(json!({
            "name": "anarlog-cloudsync",
            "userId": "user-123"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "token": "sqlite-token" }
        })))
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(token_request())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[http_header::CACHE_CONTROL], "no-store");
    let body = response_json(response).await;
    assert_eq!(body["databaseId"], "database-id");
    assert_eq!(body["encryptionVersion"], 2);
    assert_eq!(body["encryptionKeyId"], TEST_KEY_ID);
    assert_eq!(body["token"], "sqlite-token");
    assert_eq!(body["workspaceId"], "user-123");
    assert_eq!(body["accountUserId"], "user-123");
    assert_eq!(body["personalWorkspaceId"], "user-123");
    assert_eq!(body["workspaces"][0]["id"], "user-123");
    assert_eq!(body["workspaces"][0]["membershipId"], "user-123");
    assert_eq!(body["workspaces"][0]["role"], "owner");
    assert_eq!(body["workspaces"][1]["id"], "workspace-team");
    assert_eq!(body["workspaces"][1]["ownerUserId"], "user-456");
    assert_eq!(body["workspaces"][1]["kind"], "shared");
    assert_eq!(body["workspaces"][1]["name"], "Acme");
    assert_eq!(
        body["workspaces"][1]["membershipCreatedAt"],
        "2026-07-16T09:01:00Z"
    );
    assert_eq!(
        body["workspaces"][1]["membershipUpdatedAt"],
        "2026-07-16T10:01:00Z"
    );
    assert_eq!(body["workspaces"][1]["createdAt"], "2026-07-16T09:00:00Z");
    assert_eq!(body["workspaces"][1]["updatedAt"], "2026-07-16T10:00:00Z");
    assert!(body["expiresAt"].as_str().unwrap().ends_with('Z'));

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests[0].url.path(), "/rest/v1/workspace_memberships");
    assert_eq!(
        requests[1].url.path(),
        "/rest/v1/rpc/claim_personal_workspace_e2ee_key"
    );
    assert_eq!(requests[2].url.path(), "/v2/tokens");
    let token_request: Value = serde_json::from_slice(&requests[2].body).unwrap();
    assert_eq!(token_request.as_object().unwrap().len(), 4);
    assert_eq!(token_request["userId"], "user-123");
    let attributes: Value =
        serde_json::from_str(token_request["attributes"].as_str().unwrap()).unwrap();
    assert_eq!(
        attributes,
        json!({ "workspace_ids": ["user-123", "workspace-team"] })
    );
    assert!(token_request.get("workspaceId").is_none());
    assert!(token_request.get("workspaceIds").is_none());
}

async fn mock_sync_device_claim(server: &MockServer, allowed: bool, device_count: i64) {
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/claim_sync_device"))
        .and(header("apikey", "service-role-key"))
        .and(header("authorization", "Bearer service-role-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "allowed": allowed,
            "device_count": device_count,
        }])))
        .mount(server)
        .await;
}

fn token_request_with_device(fingerprint: &str, name: Option<&str>) -> Request<Body> {
    let mut builder = Request::post("/token")
        .header(E2EE_KEY_ID_HEADER, TEST_KEY_ID)
        .header(DEVICE_FINGERPRINT_HEADER, fingerprint);
    if let Some(name) = name {
        builder = builder.header(DEVICE_NAME_HEADER, name);
    }
    builder.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn registers_the_device_before_minting_a_token() {
    let server = MockServer::start().await;
    mock_sync_device_claim(&server, true, 2).await;
    mock_workspace_projection(&server, json!([personal_workspace("user-123")])).await;
    mock_e2ee_key_claim(&server, TEST_KEY_ID).await;
    Mock::given(method("POST"))
        .and(path("/v2/tokens"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "token": "sqlite-token" }
        })))
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(token_request_with_device(
            "fingerprint-1234",
            Some("Johns-MacBook"),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests[0].url.path(), "/rest/v1/rpc/claim_sync_device");
    let claim_request: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(
        claim_request,
        json!({
            "p_actor_user_id": "user-123",
            "p_device_fingerprint": "fingerprint-1234",
            "p_device_name": "Johns-MacBook",
        })
    );
}

#[tokio::test]
async fn refuses_token_when_device_limit_is_reached() {
    let server = MockServer::start().await;
    mock_sync_device_claim(&server, false, 5).await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(token_request_with_device("fingerprint-1234", None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = response_json(response).await;
    assert_eq!(body["error"]["code"], "sync_device_limit_reached");

    let requests = server.received_requests().await.unwrap();
    assert!(
        requests
            .iter()
            .all(|request| request.url.path() != "/v2/tokens")
    );
}

#[tokio::test]
async fn skips_device_claim_when_fingerprint_is_invalid() {
    let server = MockServer::start().await;
    mock_workspace_projection(&server, json!([personal_workspace("user-123")])).await;
    mock_e2ee_key_claim(&server, TEST_KEY_ID).await;
    Mock::given(method("POST"))
        .and(path("/v2/tokens"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "token": "sqlite-token" }
        })))
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(token_request_with_device("not a valid fingerprint!", None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let requests = server.received_requests().await.unwrap();
    assert!(
        requests
            .iter()
            .all(|request| request.url.path() != "/rest/v1/rpc/claim_sync_device")
    );
}

#[tokio::test]
async fn rejects_a_different_recovery_key_before_minting_a_token() {
    let server = MockServer::start().await;
    mock_workspace_projection(&server, json!([personal_workspace("user-123")])).await;
    mock_e2ee_key_claim(&server, "zyxwvutsrqponmlkjihgfe").await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(token_request())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(response).await["error"]["code"],
        "e2ee_key_mismatch"
    );
    assert!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .all(|request| request.url.path() != "/v2/tokens")
    );
}

#[tokio::test]
async fn claims_the_recovery_key_identity_without_receiving_the_key() {
    let server = MockServer::start().await;
    mock_e2ee_key_claim(&server, TEST_KEY_ID).await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(
            Request::put("/e2ee/identity")
                .header(http_header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "keyId": TEST_KEY_ID })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[http_header::CACHE_CONTROL], "no-store");
    assert_eq!(response_json(response).await["keyId"], TEST_KEY_ID);
    let request = server.received_requests().await.unwrap().pop().unwrap();
    let body = String::from_utf8(request.body).unwrap();
    assert!(body.contains(TEST_KEY_ID));
    assert!(!body.contains("anarlog-e2ee-v1"));
}

#[tokio::test]
async fn requires_an_upgrade_for_legacy_clients_after_dual_mode() {
    for protocol_mode in [
        CloudsyncProtocolMode::E2eeOnly,
        CloudsyncProtocolMode::E2eeEnforced,
    ] {
        let server = MockServer::start().await;
        let response = test_router_with_protocol(
            &server,
            "issuer-key",
            &["hyprnote_pro"],
            protocol_mode,
            Some("legacy-database-id"),
        )
        .oneshot(Request::post("/token").body(Body::empty()).unwrap())
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::UPGRADE_REQUIRED);
        assert_eq!(
            response_json(response).await["error"]["code"],
            "cloudsync_upgrade_required"
        );
        assert!(server.received_requests().await.unwrap().is_empty());
    }
}

#[tokio::test]
async fn preserves_the_desktop_1_2_2_credential_and_issuer_contract_in_dual_mode() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v2/tokens"))
        .and(header("authorization", "Bearer issuer-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "token": "legacy-sqlite-token" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let response = test_router_with_protocol(
        &server,
        "issuer-key",
        &["hyprnote_pro"],
        CloudsyncProtocolMode::Dual,
        Some("legacy-database-id"),
    )
    .oneshot(Request::post("/token").body(Body::empty()).unwrap())
    .await
    .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[http_header::CACHE_CONTROL], "no-store");
    let body = response_json(response).await;
    assert_eq!(body.as_object().unwrap().len(), 4);
    assert_eq!(body["databaseId"], "legacy-database-id");
    assert_eq!(body["token"], "legacy-sqlite-token");
    assert_eq!(body["workspaceId"], "user-123");
    assert!(body["expiresAt"].as_str().unwrap().ends_with('Z'));

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let token_request: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(token_request.as_object().unwrap().len(), 3);
    assert_eq!(token_request["name"], "anarlog-cloudsync");
    assert_eq!(token_request["userId"], "user-123");
    assert!(token_request["expiresAt"].as_str().unwrap().ends_with('Z'));
    assert!(token_request.get("attributes").is_none());
}

#[tokio::test]
async fn rejects_a_malformed_e2ee_header_instead_of_downgrading_to_legacy() {
    let server = MockServer::start().await;
    let response = test_router_with_protocol(
        &server,
        "issuer-key",
        &["hyprnote_pro"],
        CloudsyncProtocolMode::Dual,
        Some("legacy-database-id"),
    )
    .oneshot(
        Request::post("/token")
            .header(
                E2EE_KEY_ID_HEADER,
                HeaderValue::from_bytes(&[0xff]).unwrap(),
            )
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[test]
fn bounds_workspace_token_attributes() {
    let workspace = |id: String| CloudsyncWorkspace {
        id,
        owner_user_id: "user-123".to_string(),
        kind: "shared".to_string(),
        name: "Shared".to_string(),
        membership_id: "membership".to_string(),
        role: "member".to_string(),
        membership_created_at: "2026-07-16T08:00:00Z".to_string(),
        membership_updated_at: "2026-07-16T08:00:00Z".to_string(),
        created_at: "2026-07-16T08:00:00Z".to_string(),
        updated_at: "2026-07-16T08:00:00Z".to_string(),
    };

    let too_many = (0..=MAX_TOKEN_WORKSPACES)
        .map(|index| workspace(format!("workspace-{index}")))
        .collect::<Vec<_>>();
    assert!(matches!(
        encode_workspace_token_attributes(&too_many),
        Err(SyncError::Upstream)
    ));

    let oversized = [workspace("x".repeat(MAX_TOKEN_ATTRIBUTES_BYTES))];
    assert!(matches!(
        encode_workspace_token_attributes(&oversized),
        Err(SyncError::Upstream)
    ));
}

#[tokio::test]
async fn rejects_users_without_pro_entitlement() {
    let cases: &[&[&str]] = &[&[], &["hyprnote_lite"]];

    for entitlements in cases {
        let server = MockServer::start().await;
        let response = test_router(&server, "issuer-key", entitlements)
            .oneshot(token_request())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = response_json(response).await;
        assert_eq!(body["error"]["code"], "subscription_required");
        assert!(server.received_requests().await.unwrap().is_empty());
    }
}

#[tokio::test]
async fn refuses_token_when_workspace_projection_is_invalid() {
    let invalid_projections = [
        json!([]),
        json!([personal_workspace("different-user")]),
        json!([
            personal_workspace("user-123"),
            {
                "id": "other-owner-membership",
                "user_id": "user-123",
                "role": "owner",
                "created_at": "2026-07-16T09:00:00Z",
                "updated_at": "2026-07-16T09:00:00Z",
                "workspace": {
                    "id": "other-personal",
                    "owner_user_id": "other-personal",
                    "kind": "personal",
                    "name": "Other",
                    "created_at": "2026-07-16T09:00:00Z",
                    "updated_at": "2026-07-16T09:00:00Z"
                }
            }
        ]),
        json!([{
            "id": "user-123",
            "user_id": "user-123",
            "role": "admin",
            "created_at": "2026-07-16T08:00:00Z",
            "updated_at": "2026-07-16T08:00:00Z",
            "workspace": {
                "id": "user-123",
                "owner_user_id": "user-123",
                "kind": "personal",
                "name": "Personal",
                "created_at": "2026-07-16T08:00:00Z",
                "updated_at": "2026-07-16T08:00:00Z"
            }
        }]),
        json!([
            personal_workspace("user-123"),
            {
                "id": "",
                "user_id": "user-123",
                "role": "member",
                "created_at": "2026-07-16T09:00:00Z",
                "updated_at": "2026-07-16T09:00:00Z",
                "workspace": {
                    "id": "workspace-team",
                    "owner_user_id": "user-456",
                    "kind": "shared",
                    "name": "Acme",
                    "created_at": "2026-07-16T09:00:00Z",
                    "updated_at": "2026-07-16T09:00:00Z"
                }
            }
        ]),
        json!([
            personal_workspace("user-123"),
            {
                "id": "membership-team",
                "user_id": "user-123",
                "role": "editor",
                "created_at": "2026-07-16T09:00:00Z",
                "updated_at": "2026-07-16T09:00:00Z",
                "workspace": {
                    "id": "workspace-team",
                    "owner_user_id": "user-456",
                    "kind": "shared",
                    "name": "Acme",
                    "created_at": "2026-07-16T09:00:00Z",
                    "updated_at": "2026-07-16T09:00:00Z"
                }
            }
        ]),
        json!([
            personal_workspace("user-123"),
            {
                "id": "membership-team",
                "user_id": "user-123",
                "role": "member",
                "created_at": "2026-07-16T09:00:00Z",
                "updated_at": "2026-07-16T09:00:00Z",
                "workspace": {
                    "id": "workspace-team",
                    "owner_user_id": "user-456",
                    "kind": "team",
                    "name": "Acme",
                    "created_at": "2026-07-16T09:00:00Z",
                    "updated_at": "2026-07-16T09:00:00Z"
                }
            }
        ]),
        json!([
            personal_workspace("user-123"),
            {
                "id": "membership-team",
                "user_id": "user-123",
                "role": "member",
                "created_at": "2026-07-16T09:00:00Z",
                "updated_at": "2026-07-16T09:00:00Z",
                "workspace": {
                    "id": "workspace-team",
                    "owner_user_id": "user-456",
                    "kind": "shared",
                    "name": "Acme",
                    "created_at": "not-a-timestamp",
                    "updated_at": "2026-07-16T09:00:00Z"
                }
            }
        ]),
        json!([
            personal_workspace("user-123"),
            {
                "id": "membership-team",
                "user_id": "user-123",
                "role": "member",
                "created_at": "not-a-timestamp",
                "updated_at": "2026-07-16T09:00:00Z",
                "workspace": {
                    "id": "workspace-team",
                    "owner_user_id": "user-456",
                    "kind": "shared",
                    "name": "Acme",
                    "created_at": "2026-07-16T09:00:00Z",
                    "updated_at": "2026-07-16T09:00:00Z"
                }
            }
        ]),
    ];

    for projection in invalid_projections {
        let server = MockServer::start().await;
        mock_workspace_projection(&server, projection).await;

        let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
            .oneshot(token_request())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].url.path(), "/rest/v1/workspace_memberships");
    }
}

#[tokio::test]
async fn workspace_projection_failure_is_redacted() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/v1/workspace_memberships"))
        .respond_with(ResponseTemplate::new(403).set_body_string("secret supabase detail"))
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(token_request())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = response_json(response).await.to_string();
    assert!(!body.contains("anon-key"));
    assert!(!body.contains("supabase-token"));
    assert!(!body.contains("supabase detail"));
}

#[tokio::test]
async fn sqlite_cloud_failure_is_redacted() {
    let server = MockServer::start().await;
    mock_workspace_projection(&server, json!([personal_workspace("user-123")])).await;
    mock_e2ee_key_claim(&server, TEST_KEY_ID).await;
    Mock::given(method("POST"))
        .and(path("/v2/tokens"))
        .respond_with(ResponseTemplate::new(403).set_body_string("secret upstream detail"))
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-secret", &["hyprnote_pro"])
        .oneshot(token_request())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = response_json(response).await.to_string();
    assert!(!body.contains("issuer-secret"));
    assert!(!body.contains("upstream detail"));
}
