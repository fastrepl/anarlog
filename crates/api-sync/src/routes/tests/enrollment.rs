use super::*;

const REQUEST_ID: &str = "11111111-1111-4111-8111-111111111111";
const PUBLIC_KEY: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const EPHEMERAL_PUBLIC_KEY: &str = "EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE";
const NONCE: &str = "NNNNNNNNNNNNNNNNNNNNNNNNNNNNNNNN";
const CIPHERTEXT: &str = "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC";

fn register_request(replace_fingerprint: Option<&str>) -> Request<Body> {
    Request::post("/e2ee/device-enrollments")
        .header(http_header::CONTENT_TYPE, "application/json")
        .header(DEVICE_FINGERPRINT_HEADER, "fingerprint-1234")
        .header(DEVICE_NAME_HEADER, "Johns MacBook")
        .body(Body::from(
            serde_json::to_vec(&json!({
                "publicKey": PUBLIC_KEY,
                "replaceFingerprint": replace_fingerprint,
            }))
            .unwrap(),
        ))
        .unwrap()
}

#[tokio::test]
async fn registers_and_polls_a_device_enrollment() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/register_e2ee_device_enrollment"))
        .and(header("apikey", "service-role-key"))
        .and(header("authorization", "Bearer service-role-key"))
        .and(body_partial_json(json!({
            "p_actor_user_id": "user-123",
            "p_device_fingerprint": "fingerprint-1234",
            "p_device_name": "Johns MacBook",
            "p_recipient_public_key": PUBLIC_KEY,
            "p_replace_device_fingerprint": null,
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "allowed": true,
            "requires_existing_key": false,
            "request_id": REQUEST_ID,
            "expires_at": "2099-08-20T00:00:00Z",
            "enrollment_status": "sealed",
            "ephemeral_public_key": EPHEMERAL_PUBLIC_KEY,
            "nonce": NONCE,
            "ciphertext": CIPHERTEXT,
            "device_count": 2,
        }])))
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(register_request(None))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[http_header::CACHE_CONTROL], "no-store");
    let body = response_json(response).await;
    assert_eq!(body["requestId"], REQUEST_ID);
    assert_eq!(body["status"], "sealed");
    assert_eq!(body["package"]["ephemeralPublicKey"], EPHEMERAL_PUBLIC_KEY);
    assert_eq!(body["package"]["nonce"], NONCE);
    assert_eq!(body["package"]["ciphertext"], CIPHERTEXT);
}

#[tokio::test]
async fn forwards_an_atomic_device_replacement() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/register_e2ee_device_enrollment"))
        .and(body_partial_json(json!({
            "p_replace_device_fingerprint": "fingerprint-old",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "allowed": true,
            "requires_existing_key": false,
            "request_id": REQUEST_ID,
            "expires_at": "2099-08-20T00:00:00Z",
            "enrollment_status": "pending",
            "ephemeral_public_key": null,
            "nonce": null,
            "ciphertext": null,
            "device_count": 5,
        }])))
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(register_request(Some("fingerprint-old")))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await["status"], "pending");
}

#[tokio::test]
async fn distinguishes_first_device_setup_from_a_full_roster() {
    for (row, status, code) in [
        (
            json!([{
                "allowed": false,
                "requires_existing_key": true,
                "request_id": null,
                "expires_at": null,
                "enrollment_status": null,
                "ephemeral_public_key": null,
                "nonce": null,
                "ciphertext": null,
                "device_count": 0,
            }]),
            StatusCode::CONFLICT,
            "e2ee_enrollment_requires_existing_key",
        ),
        (
            json!([{
                "allowed": false,
                "requires_existing_key": false,
                "request_id": null,
                "expires_at": null,
                "enrollment_status": null,
                "ephemeral_public_key": null,
                "nonce": null,
                "ciphertext": null,
                "device_count": 5,
            }]),
            StatusCode::FORBIDDEN,
            "sync_device_limit_reached",
        ),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/register_e2ee_device_enrollment"))
            .respond_with(ResponseTemplate::new(200).set_body_json(row))
            .mount(&server)
            .await;

        let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
            .oneshot(register_request(None))
            .await
            .unwrap();

        assert_eq!(response.status(), status);
        assert_eq!(response_json(response).await["error"]["code"], code);
    }
}

#[tokio::test]
async fn rejects_malformed_enrollment_without_contacting_supabase() {
    let server = MockServer::start().await;
    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(
            Request::post("/e2ee/device-enrollments")
                .header(http_header::CONTENT_TYPE, "application/json")
                .header(DEVICE_FINGERPRINT_HEADER, "fingerprint-1234")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "publicKey": "invalid",
                        "replaceFingerprint": null,
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
async fn seals_an_enrollment_package_once() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/seal_e2ee_device_enrollment"))
        .and(header("apikey", "service-role-key"))
        .and(body_partial_json(json!({
            "p_actor_user_id": "user-123",
            "p_request_id": REQUEST_ID,
            "p_ephemeral_public_key": EPHEMERAL_PUBLIC_KEY,
            "p_nonce": NONCE,
            "p_ciphertext": CIPHERTEXT,
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "result": "sealed",
        }])))
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(
            Request::post(format!("/e2ee/device-enrollments/{REQUEST_ID}/seal"))
                .header(http_header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "ephemeralPublicKey": EPHEMERAL_PUBLIC_KEY,
                        "nonce": NONCE,
                        "ciphertext": CIPHERTEXT,
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn maps_a_resealed_enrollment_to_conflict() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/seal_e2ee_device_enrollment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "result": "conflict",
        }])))
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(
            Request::post(format!("/e2ee/device-enrollments/{REQUEST_ID}/seal"))
                .header(http_header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "ephemeralPublicKey": EPHEMERAL_PUBLIC_KEY,
                        "nonce": NONCE,
                        "ciphertext": CIPHERTEXT,
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(response).await["error"]["code"],
        "e2ee_enrollment_conflict"
    );
}

#[tokio::test]
async fn acknowledges_a_consumed_enrollment_package() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/consume_e2ee_device_enrollment"))
        .and(body_partial_json(json!({
            "p_actor_user_id": "user-123",
            "p_request_id": REQUEST_ID,
            "p_device_fingerprint": "fingerprint-1234",
            "p_recipient_public_key": PUBLIC_KEY,
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "consumed": true,
        }])))
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(
            Request::post(format!("/e2ee/device-enrollments/{REQUEST_ID}/consume"))
                .header(http_header::CONTENT_TYPE, "application/json")
                .header(DEVICE_FINGERPRINT_HEADER, "fingerprint-1234")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "publicKey": PUBLIC_KEY })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn lists_active_and_pending_devices_without_ciphertext() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/v1/sync_devices"))
        .and(query_param("user_id", "eq.user-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "device_fingerprint": "fingerprint-active",
            "device_name": "Active Mac",
            "created_at": "2026-08-19T00:00:00Z",
            "last_seen_at": "2026-08-20T00:00:00Z",
        }])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/v1/e2ee_device_enrollment_requests"))
        .and(query_param("user_id", "eq.user-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "id": REQUEST_ID,
            "device_fingerprint": "fingerprint-pending",
            "device_name": "Pending Mac",
            "recipient_public_key": PUBLIC_KEY,
            "created_at": "2026-08-20T00:00:00Z",
            "expires_at": "2099-08-21T00:00:00Z",
            "sealed_at": null,
            "consumed_at": null,
        }])))
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(Request::get("/devices").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["maxDevices"], 5);
    assert_eq!(
        body["devices"][0]["deviceFingerprint"],
        "fingerprint-active"
    );
    assert_eq!(
        body["pendingDevices"][0]["deviceFingerprint"],
        "fingerprint-pending"
    );
    assert_eq!(body["pendingDevices"][0]["status"], "pending");
    assert!(body["pendingDevices"][0].get("ciphertext").is_none());
}

#[tokio::test]
async fn removes_active_or_pending_devices_through_one_rpc() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/remove_sync_device"))
        .and(body_partial_json(json!({
            "p_actor_user_id": "user-123",
            "p_device_fingerprint": "fingerprint-1234",
        })))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(
            Request::delete("/devices/fingerprint-1234")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn renames_a_sync_device_through_the_account_scoped_rpc() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/rename_sync_device"))
        .and(body_partial_json(json!({
            "p_actor_user_id": "user-123",
            "p_device_fingerprint": "fingerprint-1234",
            "p_device_name": "Desk Mac",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(true)))
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(
            Request::patch("/devices/fingerprint-1234")
                .header(http_header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "deviceName": "  Desk Mac  " })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn rejects_an_empty_sync_device_name() {
    let server = MockServer::start().await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(
            Request::patch("/devices/fingerprint-1234")
                .header(http_header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({ "deviceName": "   " })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(server.received_requests().await.unwrap().is_empty());
}
