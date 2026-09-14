use std::{sync::LazyLock, time::Duration};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::DefaultBodyLimit,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};

const ORIGIN: &str = "http://127.0.0.1:8788";
static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(Duration::from_secs(2))
        .build()
        .expect("billing webhook client")
});

pub fn router() -> Router {
    Router::new()
        .route("/webhook/stripe", post(forward))
        .layer(DefaultBodyLimit::max(5 * 1024 * 1024))
}

pub async fn ready() -> bool {
    CLIENT
        .get(format!("{ORIGIN}/health"))
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .is_ok_and(|response| response.status().is_success())
}

async fn forward(headers: HeaderMap, body: Bytes) -> Response {
    forward_to(ORIGIN, headers, body).await
}

async fn forward_to(origin: &str, headers: HeaderMap, body: Bytes) -> Response {
    let Some(signature) = headers.get("stripe-signature") else {
        return (StatusCode::BAD_REQUEST, "missing_stripe_signature").into_response();
    };
    // Stripe verifies the original bytes; never deserialize or retry this write.
    let response = CLIENT
        .post(format!("{origin}/webhook/stripe"))
        .header("stripe-signature", signature)
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await;
    match response {
        Ok(response) => {
            let status = response.status();
            let mut response_headers = HeaderMap::new();
            if let Some(content_type) = response.headers().get("content-type") {
                response_headers.insert("content-type", content_type.clone());
            }
            (
                status,
                response_headers,
                Body::from_stream(response.bytes_stream()),
            )
                .into_response()
        }
        Err(error) => {
            tracing::error!(error = %error.without_url(), "billing_webhook_unavailable");
            (StatusCode::BAD_GATEWAY, "Billing webhook unavailable").into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_bytes, method, path},
    };

    #[tokio::test]
    async fn preserves_signed_bytes_and_failure_without_retrying() {
        let server = MockServer::start().await;
        let raw = b"{ \"id\" : \"evt_test\", \"value\": 1.00 }\n";
        Mock::given(method("POST"))
            .and(path("/webhook/stripe"))
            .and(body_bytes(raw.as_slice()))
            .respond_with(ResponseTemplate::new(500).set_body_string("{\"error\":\"retry_later\"}"))
            .expect(1)
            .mount(&server)
            .await;
        let mut headers = HeaderMap::new();
        headers.insert("stripe-signature", "t=123,v1=signature".parse().unwrap());
        let response = forward_to(&server.uri(), headers, Bytes::from_static(raw)).await;
        let received = server.received_requests().await.unwrap();
        assert_eq!(received[0].body, raw);
        assert_eq!(
            received[0].headers.get("stripe-signature").unwrap(),
            "t=123,v1=signature"
        );
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap(),
            "{\"error\":\"retry_later\"}"
        );
    }

    #[tokio::test]
    async fn unsigned_requests_do_not_reach_worker() {
        let server = MockServer::start().await;
        let response = forward_to(&server.uri(), HeaderMap::new(), Bytes::new()).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(server.received_requests().await.unwrap().is_empty());
    }
}
