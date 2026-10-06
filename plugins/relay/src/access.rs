use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

#[derive(Clone)]
pub(crate) struct RelayAccess {
    port: u16,
    secret: String,
}

impl RelayAccess {
    pub(crate) fn new(port: u16) -> Self {
        Self {
            port,
            secret: uuid::Uuid::new_v4().to_string(),
        }
    }

    fn host(&self, headers: &HeaderMap) -> Option<String> {
        let host = headers.get(header::HOST)?.to_str().ok()?;
        [
            format!("localhost:{}", self.port),
            format!("127.0.0.1:{}", self.port),
        ]
        .into_iter()
        .find(|allowed| allowed == host)
    }

    fn cookie_name(&self) -> String {
        format!("anarlog-relay-{}", self.port)
    }

    fn authenticated(&self, headers: &HeaderMap) -> bool {
        let expected = format!("{}={}", self.cookie_name(), self.secret);
        headers.get_all(header::COOKIE).iter().any(|value| {
            value
                .to_str()
                .is_ok_and(|cookies| cookies.split(';').any(|cookie| cookie.trim() == expected))
        })
    }
}

pub(crate) async fn authorize(
    State(access): State<RelayAccess>,
    req: Request,
    next: Next,
) -> Response {
    let Some(host) = access.host(req.headers()) else {
        return StatusCode::FORBIDDEN.into_response();
    };
    let origin = req.headers().get(header::ORIGIN);
    let same_origin = origin.is_some_and(|origin| {
        origin
            .to_str()
            .is_ok_and(|value| value == format!("http://{host}"))
    });
    if (origin.is_some() && !same_origin)
        || req
            .headers()
            .get("sec-fetch-site")
            .is_some_and(|site| site == "cross-site")
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    if req.uri().path() == "/ws" && (!same_origin || !access.authenticated(req.headers())) {
        return StatusCode::FORBIDDEN.into_response();
    }

    // Only a top-level browser navigation can bootstrap the HttpOnly credential.
    let bootstrap = req
        .headers()
        .get("sec-fetch-mode")
        .is_some_and(|mode| mode == "navigate")
        && req
            .headers()
            .get("sec-fetch-dest")
            .is_some_and(|dest| dest == "document");
    let mut response = next.run(req).await;
    // Vite's development CORS policy must not expose this proxy to websites.
    response
        .headers_mut()
        .remove(header::ACCESS_CONTROL_ALLOW_ORIGIN);
    response
        .headers_mut()
        .remove(header::ACCESS_CONTROL_ALLOW_CREDENTIALS);
    response.headers_mut().remove(header::SET_COOKIE);
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert(header::REFERRER_POLICY, "no-referrer".parse().unwrap());
    response
        .headers_mut()
        .insert("x-frame-options", "DENY".parse().unwrap());
    if bootstrap
        && response.status().is_success()
        && response
            .headers()
            .get(header::CONTENT_TYPE)
            .is_some_and(|content_type| {
                content_type
                    .to_str()
                    .is_ok_and(|value| value.starts_with("text/html"))
            })
    {
        response.headers_mut().insert(
            header::SET_COOKIE,
            format!(
                "{}={}; HttpOnly; SameSite=Strict; Path=/ws",
                access.cookie_name(),
                access.secret
            )
            .parse()
            .unwrap(),
        );
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, body::Body, routing::get};
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
    use tower::ServiceExt;

    #[tokio::test]
    async fn relay_requires_a_bootstrapped_credential_and_its_exact_origin() {
        let access = RelayAccess::new(1423);
        let cookie = format!("{}={}", access.cookie_name(), access.secret);
        let router = Router::new()
            .route(
                "/",
                get(|| async {
                    (
                        [
                            (header::CONTENT_TYPE, "text/html"),
                            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
                        ],
                        "<html></html>",
                    )
                }),
            )
            .route("/ws", get(|| async { "accepted" }))
            .layer(axum::middleware::from_fn_with_state(access, authorize));
        for (host, origin, credential, expected) in [
            (
                "localhost:1423",
                "http://localhost:1423",
                "",
                StatusCode::FORBIDDEN,
            ),
            (
                "localhost:1423",
                "https://attacker.example",
                cookie.as_str(),
                StatusCode::FORBIDDEN,
            ),
            (
                "attacker.example:1423",
                "http://attacker.example:1423",
                cookie.as_str(),
                StatusCode::FORBIDDEN,
            ),
            ("localhost:1423", "", cookie.as_str(), StatusCode::FORBIDDEN),
            (
                "localhost:1423",
                "http://localhost:1423",
                "anarlog-relay-1423=forged",
                StatusCode::FORBIDDEN,
            ),
            (
                "localhost:1423",
                "http://localhost:1423",
                cookie.as_str(),
                StatusCode::OK,
            ),
        ] {
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/ws")
                        .header(header::HOST, host)
                        .header(header::ORIGIN, origin)
                        .header(header::COOKIE, credential)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), expected);
        }
        for dest in ["iframe", "document"] {
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/")
                        .header(header::HOST, "localhost:1423")
                        .header("sec-fetch-mode", "navigate")
                        .header("sec-fetch-dest", dest)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                response.headers().contains_key(header::SET_COOKIE),
                dest == "document"
            );
            assert!(
                !response
                    .headers()
                    .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            );
            if dest == "document" {
                assert_eq!(
                    response.headers()[header::SET_COOKIE],
                    format!("{cookie}; HttpOnly; SameSite=Strict; Path=/ws")
                );
            }
        }
        let restarted = RelayAccess::new(1423);
        assert!(!restarted.authenticated(&HeaderMap::from_iter([(
            header::COOKIE,
            cookie.parse().unwrap()
        )])));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let access = RelayAccess::new(addr.port());
        let cookie = format!("{}={}", access.cookie_name(), access.secret);
        let router = Router::new()
            .route(
                "/ws",
                get(|ws: axum::extract::ws::WebSocketUpgrade| async {
                    ws.on_upgrade(|mut socket| async move {
                        if let Some(Ok(message)) = socket.recv().await {
                            socket.send(message).await.unwrap();
                        }
                    })
                }),
            )
            .layer(axum::middleware::from_fn_with_state(access, authorize));
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let mut request = format!("ws://{addr}/ws").into_client_request().unwrap();
        request
            .headers_mut()
            .insert(header::COOKIE, cookie.parse().unwrap());
        request
            .headers_mut()
            .insert(header::ORIGIN, "https://attacker.example".parse().unwrap());
        let error = tokio_tungstenite::connect_async(request.clone())
            .await
            .unwrap_err();
        assert!(
            matches!(error, tokio_tungstenite::tungstenite::Error::Http(response) if response.status() == StatusCode::FORBIDDEN)
        );
        request
            .headers_mut()
            .insert(header::ORIGIN, format!("http://{addr}").parse().unwrap());
        let (mut socket, response) = tokio_tungstenite::connect_async(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);
        socket
            .send(Message::Text("authenticated".into()))
            .await
            .unwrap();
        assert_eq!(
            socket.next().await.unwrap().unwrap(),
            Message::Text("authenticated".into())
        );
        server.abort();
    }
}
