use super::*;

const TEST_MEMBER_PUBLIC_KEY: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopq";
const TEST_SHARED_WORKSPACE_ID: &str = "11111111-1111-4111-8111-111111111111";
const TEST_RECIPIENT_USER_ID: &str = "22222222-2222-4222-8222-222222222222";

#[tokio::test]
async fn lists_workspace_key_recipients_with_the_user_session() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/list_workspace_key_recipients"))
        .and(header("apikey", "anon-key"))
        .and(header("authorization", "Bearer supabase-token"))
        .and(body_partial_json(json!({
            "p_workspace_id": TEST_SHARED_WORKSPACE_ID,
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "user_id": TEST_RECIPIENT_USER_ID,
            "user_email": "member@example.com",
            "role": "member",
            "public_key": TEST_MEMBER_PUBLIC_KEY,
            "granted_key_ids": ["AAAAAAAAAAAAAAAAAAAAAA"],
        }])))
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(
            Request::get(format!(
                "/e2ee/workspaces/{TEST_SHARED_WORKSPACE_ID}/recipients"
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body[0]["userId"], TEST_RECIPIENT_USER_ID);
    assert_eq!(body[0]["publicKey"], TEST_MEMBER_PUBLIC_KEY);
    assert_eq!(body[0]["grantedKeyIds"][0], "AAAAAAAAAAAAAAAAAAAAAA");
}

#[tokio::test]
async fn publishes_only_wrapped_workspace_key_grants_with_the_user_session() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/set_workspace_e2ee_key"))
        .and(header("apikey", "anon-key"))
        .and(header("authorization", "Bearer supabase-token"))
        .and(body_partial_json(json!({
            "p_workspace_id": TEST_SHARED_WORKSPACE_ID,
            "p_key_id": "AAAAAAAAAAAAAAAAAAAAAA",
            "p_grants": [{
                "userId": TEST_RECIPIENT_USER_ID,
                "ephemeralPublicKey": "A".repeat(43),
                "nonce": "B".repeat(32),
                "ciphertext": "C".repeat(64),
            }],
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "key_id": "AAAAAAAAAAAAAAAAAAAAAA",
            "granted_member_count": 1,
        }])))
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(
            Request::put(format!("/e2ee/workspaces/{TEST_SHARED_WORKSPACE_ID}/key"))
                .header(http_header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "keyId": "AAAAAAAAAAAAAAAAAAAAAA",
                        "grants": [{
                            "userId": TEST_RECIPIENT_USER_ID,
                            "ephemeralPublicKey": "A".repeat(43),
                            "nonce": "B".repeat(32),
                            "ciphertext": "C".repeat(64),
                        }],
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[http_header::CACHE_CONTROL], "no-store");
    let body = response_json(response).await;
    assert_eq!(body["keyId"], "AAAAAAAAAAAAAAAAAAAAAA");
    assert_eq!(body["grantedMemberCount"], 1);
}

#[tokio::test]
async fn rejects_malformed_workspace_key_grants_without_contacting_supabase() {
    let server = MockServer::start().await;
    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(
            Request::put(format!("/e2ee/workspaces/{TEST_SHARED_WORKSPACE_ID}/key"))
                .header(http_header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "keyId": "invalid",
                        "grants": [],
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(server.received_requests().await.unwrap().is_empty());
}

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
async fn publishes_the_member_identity_before_issuing_replica_credentials() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/publish_e2ee_member_identity"))
        .and(header("apikey", "anon-key"))
        .and(header("authorization", "Bearer supabase-token"))
        .and(body_partial_json(json!({
            "p_public_key": TEST_MEMBER_PUBLIC_KEY,
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "public_key": TEST_MEMBER_PUBLIC_KEY,
        }])))
        .mount(&server)
        .await;
    mock_e2ee_key_claim(&server, TEST_KEY_ID).await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(
            Request::post("/replica/credentials")
                .header(E2EE_KEY_ID_HEADER, TEST_KEY_ID)
                .header(E2EE_MEMBER_PUBLIC_KEY_HEADER, TEST_MEMBER_PUBLIC_KEY)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests[0].url.path(),
        "/rest/v1/rpc/publish_e2ee_member_identity"
    );
    assert_eq!(
        requests[1].url.path(),
        "/rest/v1/rpc/claim_personal_workspace_e2ee_key"
    );
}

#[tokio::test]
async fn rejects_a_malformed_member_identity_without_contacting_supabase() {
    let server = MockServer::start().await;
    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(
            Request::post("/replica/credentials")
                .header(E2EE_KEY_ID_HEADER, TEST_KEY_ID)
                .header(E2EE_MEMBER_PUBLIC_KEY_HEADER, "invalid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(server.received_requests().await.unwrap().is_empty());
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
    mock_workspace_key_grants(
        &server,
        json!([
            {
                "workspace_id": "workspace-team",
                "key_id": "AAAAAAAAAAAAAAAAAAAAAA",
                "ephemeral_public_key": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                "nonce": "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
                "ciphertext": "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC",
                "is_active": true
            }
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
    assert_eq!(
        body["workspaceKeyGrants"][0]["workspaceId"],
        "workspace-team"
    );
    assert_eq!(
        body["workspaceKeyGrants"][0]["keyId"],
        "AAAAAAAAAAAAAAAAAAAAAA"
    );
    assert_eq!(body["workspaceKeyGrants"][0]["isActive"], true);
    assert!(body["expiresAt"].as_str().unwrap().ends_with('Z'));

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests[0].url.path(), "/rest/v1/workspace_memberships");
    assert_eq!(
        requests[1].url.path(),
        "/rest/v1/rpc/list_all_my_workspace_e2ee_grants"
    );
    assert_eq!(
        requests[2].url.path(),
        "/rest/v1/rpc/claim_personal_workspace_e2ee_key"
    );
    assert_eq!(requests[3].url.path(), "/v2/tokens");
    let token_request: Value = serde_json::from_slice(&requests[3].body).unwrap();
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
    let claim_index = requests
        .iter()
        .position(|request| request.url.path() == "/rest/v1/rpc/claim_sync_device")
        .unwrap();
    let token_index = requests
        .iter()
        .position(|request| request.url.path() == "/v2/tokens")
        .unwrap();
    assert!(claim_index < token_index);
    let claim_request: Value = serde_json::from_slice(&requests[claim_index].body).unwrap();
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
    mock_workspace_projection(&server, json!([personal_workspace("user-123")])).await;
    mock_e2ee_key_claim(&server, TEST_KEY_ID).await;
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
