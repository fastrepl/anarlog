use super::*;

const MEETING_KEY: &str = "meeting-key-0123456789abcdef";

fn heartbeat_request(intent: &str, fingerprint: Option<&str>) -> Request<Body> {
    let mut request = Request::post(format!("/meetings/{MEETING_KEY}/devices"))
        .header(http_header::CONTENT_TYPE, "application/json");
    if let Some(fingerprint) = fingerprint {
        request = request.header(DEVICE_FINGERPRINT_HEADER, fingerprint);
    }
    request
        .body(Body::from(
            serde_json::to_vec(&json!({ "intent": intent })).unwrap(),
        ))
        .unwrap()
}

#[tokio::test]
async fn claims_meeting_and_lists_present_devices() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/heartbeat_meeting_device"))
        .and(body_partial_json(json!({
            "p_actor_user_id": "user-123",
            "p_meeting_key": MEETING_KEY,
            "p_device_fingerprint": "fingerprint-1234",
            "p_intent": "claim",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "device_fingerprint": "fingerprint-1234",
                "device_name": "Work Mac",
                "is_primary": true,
            },
            {
                "device_fingerprint": "fingerprint-5678",
                "device_name": null,
                "is_primary": false,
            },
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(heartbeat_request("claim", Some("fingerprint-1234")))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(
        body,
        json!({
            "devices": [
                {
                    "deviceFingerprint": "fingerprint-1234",
                    "deviceName": "Work Mac",
                    "primary": true,
                },
                {
                    "deviceFingerprint": "fingerprint-5678",
                    "deviceName": null,
                    "primary": false,
                },
            ],
        })
    );
}

#[tokio::test]
async fn rejects_heartbeat_without_device_fingerprint() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/heartbeat_meeting_device"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(0)
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(heartbeat_request("present", None))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rejects_invalid_meeting_key() {
    let server = MockServer::start().await;
    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(
            Request::post("/meetings/short/devices")
                .header(http_header::CONTENT_TYPE, "application/json")
                .header(DEVICE_FINGERPRINT_HEADER, "fingerprint-1234")
                .body(Body::from(r#"{"intent":"present"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn reports_unregistered_device() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/v1/rpc/heartbeat_meeting_device"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "code": "42501",
            "message": "sync device is not registered",
        })))
        .mount(&server)
        .await;

    let response = test_router(&server, "issuer-key", &["hyprnote_pro"])
        .oneshot(heartbeat_request("present", Some("fingerprint-1234")))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = response_json(response).await;
    assert_eq!(body["error"]["code"], "meeting_device_not_registered");
}

#[tokio::test]
async fn requires_pro_entitlement() {
    let server = MockServer::start().await;
    let response = test_router(&server, "issuer-key", &[])
        .oneshot(heartbeat_request("present", Some("fingerprint-1234")))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
