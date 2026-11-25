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
    mock_workspace_projection(
        &server,
        json!([team_membership(), personal_workspace("user-123")]),
    )
    .await;
    mock_workspace_key_grants(&server, json!([team_key_grant()])).await;
    mock_e2ee_key_claim(&server, TEST_KEY_ID).await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(
            Request::post("/replica/credentials")
                .header(E2EE_KEY_ID_HEADER, TEST_KEY_ID)
                .header(
                    CLOUDSYNC_TRANSPORTS_HEADER,
                    format!("replica, {ACCEPTED_REPLICA_TRANSPORT}"),
                )
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
    assert_eq!(body["personalWorkspaceId"], "user-123");
    assert_eq!(body["workspaces"][0]["id"], "user-123");
    assert_eq!(body["workspaces"][0]["role"], "owner");
    assert_eq!(body["workspaces"][1]["id"], "workspace-team");
    assert_eq!(body["workspaces"][1]["kind"], "shared");
    assert_eq!(
        body["workspaceKeyGrants"][0]["workspaceId"],
        "workspace-team"
    );
    assert_eq!(body["workspaceKeyGrants"][0]["isActive"], true);
    assert!(body["expiresAt"].as_str().unwrap().ends_with('Z'));
    assert!(body.get("databaseId").is_none());
    assert!(body.get("token").is_none());
    assert!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .all(|request| request.url.path() != "/v2/tokens")
    );
}

async fn mock_personal_only_e2ee_account(server: &MockServer) {
    mock_workspace_projection(server, json!([personal_workspace("user-123")])).await;
    mock_workspace_key_grants(server, json!([])).await;
    mock_e2ee_key_claim(server, TEST_KEY_ID).await;
}

#[tokio::test]
async fn token_route_issues_replica_credentials_only_for_accepted_replica_clients() {
    for (case, request, expect_replica) in [
        (
            "accepted replica client",
            accepted_replica_token_request(),
            true,
        ),
        (
            "client that predates the accepted replica",
            replica_capable_token_request(),
            false,
        ),
        (
            "client without the replica transport",
            token_request(),
            false,
        ),
        (
            "legacy client without key identity or transports",
            Request::post("/token").body(Body::empty()).unwrap(),
            false,
        ),
    ] {
        let server = MockServer::start().await;
        mock_personal_only_e2ee_account(&server).await;

        let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
            .oneshot(request)
            .await
            .unwrap();

        if expect_replica {
            assert_eq!(response.status(), StatusCode::OK, "{case}");
            assert_eq!(
                response.headers()[http_header::CACHE_CONTROL],
                "no-store",
                "{case}"
            );
            let body = response_json(response).await;
            assert_eq!(body["transport"], "replica", "{case}");
            assert_eq!(body["encryptionKeyId"], TEST_KEY_ID, "{case}");
            assert!(body.get("databaseId").is_none(), "{case}");
            assert!(body.get("token").is_none(), "{case}");
        } else {
            assert_eq!(response.status(), StatusCode::UPGRADE_REQUIRED, "{case}");
            let body = response_json(response).await;
            assert_eq!(
                body["error"]["code"], "cloudsync_upgrade_required",
                "{case}"
            );
            assert!(
                server.received_requests().await.unwrap().is_empty(),
                "{case}"
            );
        }
    }
}

#[tokio::test]
async fn replica_credentials_require_the_accepted_replica() {
    let accepted = format!("replica, {ACCEPTED_REPLICA_TRANSPORT}");
    for (transports, expect_credentials) in [
        (None, false),
        (Some("replica"), false),
        (Some(accepted.as_str()), true),
    ] {
        let case = format!("transports {transports:?}");
        let server = MockServer::start().await;
        mock_personal_only_e2ee_account(&server).await;
        let mut request =
            Request::post("/replica/credentials").header(E2EE_KEY_ID_HEADER, TEST_KEY_ID);
        if let Some(transports) = transports {
            request = request.header(CLOUDSYNC_TRANSPORTS_HEADER, transports);
        }

        let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();

        if expect_credentials {
            assert_eq!(response.status(), StatusCode::OK, "{case}");
            assert_eq!(
                response_json(response).await["transport"],
                "replica",
                "{case}"
            );
        } else {
            assert_eq!(response.status(), StatusCode::UPGRADE_REQUIRED, "{case}");
            let body = response_json(response).await;
            assert_eq!(
                body["error"]["code"], "cloudsync_upgrade_required",
                "{case}"
            );
            assert!(
                server.received_requests().await.unwrap().is_empty(),
                "{case}"
            );
        }
    }
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
    mock_workspace_projection(&server, json!([personal_workspace("user-123")])).await;
    mock_e2ee_key_claim(&server, TEST_KEY_ID).await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(
            Request::post("/replica/credentials")
                .header(E2EE_KEY_ID_HEADER, TEST_KEY_ID)
                .header(E2EE_MEMBER_PUBLIC_KEY_HEADER, TEST_MEMBER_PUBLIC_KEY)
                .header(
                    CLOUDSYNC_TRANSPORTS_HEADER,
                    format!("replica, {ACCEPTED_REPLICA_TRANSPORT}"),
                )
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
    assert_eq!(requests[1].url.path(), "/rest/v1/workspace_memberships");
    assert_eq!(
        requests[2].url.path(),
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
                .header(
                    CLOUDSYNC_TRANSPORTS_HEADER,
                    format!("replica, {ACCEPTED_REPLICA_TRANSPORT}"),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn issues_replica_credentials_from_token_route_for_verified_supabase_subject() {
    let server = MockServer::start().await;
    mock_workspace_projection(
        &server,
        json!([team_membership(), personal_workspace("user-123")]),
    )
    .await;
    mock_workspace_key_grants(&server, json!([team_key_grant()])).await;
    mock_e2ee_key_claim(&server, TEST_KEY_ID).await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(accepted_replica_token_request())
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
    assert!(body.get("databaseId").is_none());
    assert!(body.get("token").is_none());
    assert!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .all(|request| request.url.path() != "/v2/tokens")
    );
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
        .header(
            CLOUDSYNC_TRANSPORTS_HEADER,
            format!("replica, {ACCEPTED_REPLICA_TRANSPORT}"),
        )
        .header(DEVICE_FINGERPRINT_HEADER, fingerprint);
    if let Some(name) = name {
        builder = builder.header(DEVICE_NAME_HEADER, name);
    }
    builder.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn registers_the_device_before_issuing_replica_credentials() {
    let server = MockServer::start().await;
    mock_sync_device_claim(&server, true, 2).await;
    mock_workspace_projection(&server, json!([personal_workspace("user-123")])).await;
    mock_e2ee_key_claim(&server, TEST_KEY_ID).await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(token_request_with_device(
            "fingerprint-1234",
            Some("Johns-MacBook"),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["transport"], "replica");

    let requests = server.received_requests().await.unwrap();
    assert!(
        requests
            .iter()
            .all(|request| request.url.path() != "/v2/tokens")
    );
    let claim_request = requests
        .iter()
        .find(|request| request.url.path() == "/rest/v1/rpc/claim_sync_device")
        .unwrap();
    let claim_request: Value = serde_json::from_slice(&claim_request.body).unwrap();
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
}

#[tokio::test]
async fn skips_device_claim_when_fingerprint_is_invalid() {
    let server = MockServer::start().await;
    mock_workspace_projection(&server, json!([personal_workspace("user-123")])).await;
    mock_e2ee_key_claim(&server, TEST_KEY_ID).await;

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
async fn rejects_a_different_recovery_key_before_issuing_credentials() {
    let server = MockServer::start().await;
    mock_workspace_projection(&server, json!([personal_workspace("user-123")])).await;
    mock_e2ee_key_claim(&server, "zyxwvutsrqponmlkjihgfe").await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(accepted_replica_token_request())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(response).await["error"]["code"],
        "e2ee_key_mismatch"
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
async fn rejects_a_malformed_e2ee_header_on_token_route() {
    for invalid_header in [
        HeaderValue::from_bytes(&[0xff]).unwrap(),
        HeaderValue::from_static("invalid"),
    ] {
        let server = MockServer::start().await;
        let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
            .oneshot(
                Request::post("/token")
                    .header(E2EE_KEY_ID_HEADER, invalid_header)
                    .header(
                        CLOUDSYNC_TRANSPORTS_HEADER,
                        format!("replica, {ACCEPTED_REPLICA_TRANSPORT}"),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(server.received_requests().await.unwrap().is_empty());
    }
}

#[tokio::test]
async fn rejects_users_without_pro_entitlement() {
    let cases: &[&[&str]] = &[&[], &["hyprnote_lite"]];

    for entitlements in cases {
        let server = MockServer::start().await;
        let response = test_router(&server, "issuer-key", entitlements)
            .oneshot(accepted_replica_token_request())
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
    let mut other_personal_workspace = personal_workspace("user-123");
    other_personal_workspace["id"] = json!("other-owner-membership");
    other_personal_workspace["workspace"]["id"] = json!("other-personal");
    other_personal_workspace["workspace"]["owner_user_id"] = json!("other-personal");
    other_personal_workspace["workspace"]["name"] = json!("Other");

    let mut admin_personal_workspace = personal_workspace("user-123");
    admin_personal_workspace["role"] = json!("admin");

    let mut missing_membership_id = team_membership();
    missing_membership_id["id"] = json!("");

    let mut unsupported_member_role = team_membership();
    unsupported_member_role["role"] = json!("editor");

    let mut unsupported_workspace_kind = team_membership();
    unsupported_workspace_kind["workspace"]["kind"] = json!("team");

    let mut invalid_workspace_timestamp = team_membership();
    invalid_workspace_timestamp["workspace"]["created_at"] = json!("not-a-timestamp");

    let mut invalid_membership_timestamp = team_membership();
    invalid_membership_timestamp["created_at"] = json!("not-a-timestamp");

    let invalid_projections = [
        ("no memberships", json!([])),
        (
            "personal workspace belongs to another user",
            json!([personal_workspace("different-user")]),
        ),
        (
            "another personal workspace is also owned",
            json!([personal_workspace("user-123"), other_personal_workspace]),
        ),
        (
            "personal workspace membership has an admin role",
            json!([admin_personal_workspace]),
        ),
        (
            "team membership id is empty",
            json!([personal_workspace("user-123"), missing_membership_id]),
        ),
        (
            "team membership role is unsupported",
            json!([personal_workspace("user-123"), unsupported_member_role]),
        ),
        (
            "team workspace kind is unsupported",
            json!([personal_workspace("user-123"), unsupported_workspace_kind]),
        ),
        (
            "team workspace timestamp is invalid",
            json!([personal_workspace("user-123"), invalid_workspace_timestamp]),
        ),
        (
            "team membership timestamp is invalid",
            json!([personal_workspace("user-123"), invalid_membership_timestamp]),
        ),
    ];

    for (case, projection) in invalid_projections {
        let server = MockServer::start().await;
        mock_workspace_projection(&server, projection).await;

        let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
            .oneshot(accepted_replica_token_request())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY, "{case}");
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1, "{case}");
        assert_eq!(
            requests[0].url.path(),
            "/rest/v1/workspace_memberships",
            "{case}"
        );
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
        .oneshot(accepted_replica_token_request())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = response_json(response).await.to_string();
    assert!(!body.contains("anon-key"));
    assert!(!body.contains("supabase-token"));
    assert!(!body.contains("supabase detail"));
}
