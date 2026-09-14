use super::*;
use axum::{
    extract::{DefaultBodyLimit, WebSocketUpgrade},
    routing::{any, get},
};
use futures_util::SinkExt;
use tokio_util::sync::CancellationToken;

async fn serve(router: Router, shutdown: CancellationToken) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await
            .unwrap();
    });
    format!("http://{address}")
}

async fn gateway(origin: &str, gate: &SessionGate, shutdown: CancellationToken) -> String {
    serve(
        route(
            Router::new().route("/{*path}", any(|| async { StatusCode::IM_A_TEAPOT })),
            &Some(origin.into()),
            gate,
            "test-signing-key",
        ),
        shutdown,
    )
    .await
}

#[tokio::test]
async fn large_uploads_keep_auth_query_and_response_headers_without_retrying_redirects() {
    let stop = CancellationToken::new();
    let upstream = serve(
        Router::new()
            .route(
                "/upload",
                any(|request: Request| async move {
                    assert_eq!(request.uri().query(), Some("key=a%2Fb&key=c"));
                    assert_eq!(
                        request.headers()[header::AUTHORIZATION],
                        "Bearer test-token"
                    );
                    assert_eq!(request.headers()["x-forwarded-host"], "api.example.test");
                    assert!(!request.headers().contains_key("x-remove-me"));
                    for name in [
                        "fly-force-instance-id",
                        "fly-prefer-instance-id",
                        "fly-force-region",
                    ] {
                        assert!(!request.headers().contains_key(name));
                    }
                    let bytes = axum::body::to_bytes(request.into_body(), 8 * 1024 * 1024)
                        .await
                        .unwrap();
                    assert_eq!(bytes.len(), 3 * 1024 * 1024);
                    assert!(bytes.iter().all(|byte| *byte == 42));
                    (
                        StatusCode::TEMPORARY_REDIRECT,
                        [
                            (header::LOCATION, "/do-not-follow"),
                            (header::SET_COOKIE, "session=one; Secure"),
                        ],
                        "redirect",
                    )
                }),
            )
            .layer(DefaultBodyLimit::disable()),
        stop.clone(),
    )
    .await;
    let gate = SessionGate::new();
    let base = gateway(&upstream, &gate, stop.clone()).await;
    let response = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
        .post(format!("{base}/upload?key=a%2Fb&key=c"))
        .header(header::HOST, "api.example.test")
        .bearer_auth("test-token")
        .header(header::CONNECTION, "x-remove-me")
        .header("x-remove-me", "private")
        .header("fly-force-instance-id", "gateway-machine")
        .header("fly-prefer-instance-id", "gateway-machine")
        .header("fly-force-region", "sjc")
        .body(vec![42u8; 3 * 1024 * 1024])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(response.headers()[header::LOCATION], "/do-not-follow");
    assert_eq!(
        response.headers()[header::SET_COOKIE],
        "session=one; Secure"
    );
    assert_eq!(response.text().await.unwrap(), "redirect");
    tokio::time::timeout(Duration::from_secs(2), gate.wait_until_idle())
        .await
        .unwrap();
    stop.cancel();
}

#[tokio::test]
async fn draining_waits_for_the_last_stream_chunk() {
    let stop = CancellationToken::new();
    let (send, recv) = tokio::sync::mpsc::channel::<Result<&'static str, std::io::Error>>(4);
    let recv = Arc::new(std::sync::Mutex::new(Some(recv)));
    let upstream = serve(
        Router::new()
            .route("/late-request", get(|| async { "completed" }))
            .route(
                "/stream",
                get(move || {
                    let recv = recv.lock().unwrap().take().unwrap();
                    async move {
                        Body::from_stream(futures_util::stream::unfold(recv, |mut recv| async {
                            recv.recv().await.map(|item| (item, recv))
                        }))
                    }
                }),
            ),
        stop.clone(),
    )
    .await;
    let gate = SessionGate::new();
    let base = gateway(&upstream, &gate, stop.clone()).await;
    send.send(Ok("first\n")).await.unwrap();
    let mut response = reqwest::get(format!("{base}/stream")).await.unwrap();
    assert_eq!(response.chunk().await.unwrap().unwrap(), "first\n");
    gate.begin_drain();
    assert_eq!(gate.active(), 1);
    assert!(
        tokio::time::timeout(Duration::from_millis(25), gate.wait_until_idle())
            .await
            .is_err()
    );
    assert_eq!(
        reqwest::get(format!("{base}/late-request"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    send.send(Ok("last\n")).await.unwrap();
    drop(send);
    assert_eq!(response.chunk().await.unwrap().unwrap(), "last\n");
    assert!(response.chunk().await.unwrap().is_none());
    tokio::time::timeout(Duration::from_secs(2), gate.wait_until_idle())
        .await
        .unwrap();
    stop.cancel();
}

#[tokio::test]
async fn websocket_upgrade_survives_drain_and_releases_its_permit_on_close() {
    let stop = CancellationToken::new();
    let upstream = serve(
        Router::new().route(
            "/listen",
            get(|ws: WebSocketUpgrade| async {
                ws.on_upgrade(|mut socket| async move {
                    while let Some(Ok(message)) = socket.recv().await {
                        if socket.send(message).await.is_err() {
                            break;
                        }
                    }
                })
            }),
        ),
        stop.clone(),
    )
    .await;
    let gate = SessionGate::new();
    let base = gateway(&upstream, &gate, stop.clone()).await;
    let (mut socket, _) =
        tokio_tungstenite::connect_async(format!("{}/listen", base.replacen("http", "ws", 1)))
            .await
            .unwrap();
    gate.begin_drain();
    socket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            "still recording".into(),
        ))
        .await
        .unwrap();
    assert_eq!(
        socket.next().await.unwrap().unwrap().into_text().unwrap(),
        "still recording"
    );
    assert_eq!(gate.active(), 1);
    socket.close(None).await.unwrap();
    drop(socket);
    tokio::time::timeout(Duration::from_secs(2), gate.wait_until_idle())
        .await
        .unwrap();
    stop.cancel();
}

#[test]
fn origins_cannot_enable_a_proxy_loop_on_a_service_or_embed_credentials() {
    for origin in [
        "https://user:secret@example.com",
        "https://example.com/path",
        "https://example.com?secret=x",
        "file:///tmp/test",
    ] {
        let env = Env {
            anarlog_ai_origin: Some(origin.into()),
            ..Default::default()
        };
        assert!(env.validate(crate::service::Service::All).is_err());
    }
    let env = Env {
        anarlog_ai_origin: Some("https://example.com".into()),
        ..Default::default()
    };
    assert!(env.validate(crate::service::Service::All).is_ok());
    assert!(env.validate(crate::service::Service::Ai).is_err());
}

#[tokio::test]
async fn fly_second_hop_preserves_distinct_client_rate_limit_identities() {
    let stop = CancellationToken::new();
    let upstream = serve(
        Router::new()
            .route(
                "/identity",
                get(|request: Request| async move {
                    assert!(
                        !request
                            .headers()
                            .contains_key("x-anarlog-client-ip-signature")
                    );
                    request.headers()["fly-client-ip"]
                        .to_str()
                        .unwrap()
                        .to_owned()
                }),
            )
            .layer(middleware::from_fn_with_state(
                Arc::<str>::from("test-signing-key"),
                client_ip::restore,
            ))
            .layer(middleware::from_fn(
                |mut request: Request, next: Next| async move {
                    request
                        .headers_mut()
                        .insert("fly-client-ip", HeaderValue::from_static("198.51.100.1"));
                    next.run(request).await
                },
            )),
        stop.clone(),
    )
    .await;
    let base = gateway(&upstream, &SessionGate::new(), stop.clone()).await;
    let client = reqwest::Client::new();
    for ip in ["192.0.2.1", "192.0.2.2", "2001:db8::1"] {
        let response = client
            .get(format!("{base}/identity"))
            .header("fly-client-ip", ip)
            .header("x-anarlog-client-ip", "192.0.2.99")
            .header("x-anarlog-client-ip-signature", "spoofed")
            .send()
            .await
            .unwrap();
        assert_eq!(response.text().await.unwrap(), ip);
    }
    let direct = client
        .get(format!("{upstream}/identity"))
        .header("x-anarlog-client-ip", "192.0.2.99")
        .header("x-anarlog-client-ip-signature", "spoofed")
        .send()
        .await
        .unwrap();
    assert_eq!(direct.text().await.unwrap(), "198.51.100.1");
    stop.cancel();
}

#[tokio::test]
async fn a_late_http_response_finishes_during_graceful_shutdown() {
    let stop = CancellationToken::new();
    let (send, recv) = tokio::sync::mpsc::channel::<Result<&'static str, std::io::Error>>(4);
    let recv = Arc::new(std::sync::Mutex::new(Some(recv)));
    let upstream = serve(
        Router::new().route(
            "/late",
            get(move || {
                let recv = recv.lock().unwrap().take().unwrap();
                async move {
                    Body::from_stream(futures_util::stream::unfold(recv, |mut recv| async {
                        recv.recv().await.map(|item| (item, recv))
                    }))
                }
            }),
        ),
        stop.clone(),
    )
    .await;
    let gate = SessionGate::new();
    gate.begin_drain();
    let base = gateway(&upstream, &gate, stop.clone()).await;
    send.send(Ok("first")).await.unwrap();
    let mut response = reqwest::get(format!("{base}/late")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.chunk().await.unwrap().unwrap(), "first");
    assert_eq!(gate.active(), 0);
    stop.cancel();
    send.send(Ok("last")).await.unwrap();
    drop(send);
    assert_eq!(response.chunk().await.unwrap().unwrap(), "last");
    assert!(response.chunk().await.unwrap().is_none());
}

#[tokio::test]
async fn new_websocket_handshakes_are_replayed_before_forwarding_during_drain() {
    let stop = CancellationToken::new();
    let gate = SessionGate::new();
    gate.begin_drain();
    let base = gateway("http://127.0.0.1:1", &gate, stop.clone()).await;
    let response = reqwest::Client::new()
        .get(format!("{base}/listen"))
        .header(header::CONNECTION, "upgrade")
        .header(header::UPGRADE, "websocket")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()["fly-replay"], "elsewhere=true");
    assert_eq!(gate.active(), 0);
    stop.cancel();
}
