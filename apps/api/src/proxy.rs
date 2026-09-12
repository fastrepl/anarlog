use std::{sync::Arc, time::Duration};

use anlg_transcribe_proxy::SessionGate;
use axum::{
    Router,
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use futures_util::StreamExt;
use serde::Deserialize;

#[derive(Default, Deserialize)]
pub struct Env {
    pub anarlog_ai_origin: Option<String>,
    pub anarlog_sync_origin: Option<String>,
    pub anarlog_core_origin: Option<String>,
    pub anarlog_billing_origin: Option<String>,
}

impl Env {
    pub fn validate(&self, role: crate::service::Service) -> Result<(), String> {
        for origin in [
            &self.anarlog_ai_origin,
            &self.anarlog_sync_origin,
            &self.anarlog_core_origin,
            &self.anarlog_billing_origin,
        ]
        .into_iter()
        .flatten()
        {
            if role != crate::service::Service::All {
                return Err(
                    "Upstream origins are only supported by the combined compatibility API".into(),
                );
            }
            let url = url::Url::parse(origin).map_err(|_| "Invalid upstream origin")?;
            if !matches!(url.scheme(), "http" | "https")
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.path() != "/"
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err("Upstream must be an HTTP(S) origin without credentials, path, query or fragment".into());
            }
        }
        Ok(())
    }
}

struct Proxy {
    origin: String,
    client: reqwest::Client,
    gate: SessionGate,
    signing_key: Arc<str>,
}

pub fn route(
    router: Router,
    origin: &Option<String>,
    gate: &SessionGate,
    signing_key: &str,
) -> Router {
    let Some(origin) = origin else { return router };
    let state = Arc::new(Proxy {
        origin: origin.trim_end_matches('/').to_string(),
        client: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(10))
            .http1_only()
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .build()
            .expect("proxy HTTP client"),
        gate: gate.clone(),
        signing_key: signing_key.into(),
    });
    router.route_layer(middleware::from_fn_with_state(state, forward))
}

fn strip_hop_headers(headers: &mut HeaderMap) {
    let connection_headers: Vec<_> = headers
        .get_all(header::CONNECTION)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(',').map(|part| part.trim().to_string()))
        .collect();
    for name in connection_headers {
        headers.remove(name);
    }
    for name in [
        "connection",
        "keep-alive",
        "proxy-authenticate",
        "proxy-authorization",
        "te",
        "trailer",
        "transfer-encoding",
        "upgrade",
    ] {
        headers.remove(name);
    }
}

async fn forward(State(proxy): State<Arc<Proxy>>, mut request: Request, _next: Next) -> Response {
    if request.headers().contains_key("x-anarlog-proxy-hop") {
        return (StatusCode::LOOP_DETECTED, "Upstream routing loop").into_response();
    }
    let websocket = request
        .headers()
        .get(header::UPGRADE)
        .is_some_and(|value| value.as_bytes().eq_ignore_ascii_case(b"websocket"));
    let permit = match proxy.gate.try_acquire() {
        Ok(permit) => Some(permit),
        Err(_) if websocket => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                [("fly-replay", "elsewhere=true")],
                "Server is draining",
            )
                .into_response();
        }
        // Axum waits for accepted HTTP responses during graceful shutdown. Residual
        // requests may still arrive after cordoning; complete them without replaying writes.
        Err(_) => None,
    };
    let upgrade = websocket.then(|| hyper::upgrade::on(&mut request));
    let url = format!(
        "{}{}",
        proxy.origin,
        request
            .uri()
            .path_and_query()
            .map_or("/", |value| value.as_str())
    );
    let (parts, body) = request.into_parts();
    let mut headers = parts.headers;
    let host = headers.get(header::HOST).cloned();
    strip_hop_headers(&mut headers);
    headers.remove(header::HOST);
    // Instance selection belongs to this Fly app, not the destination service.
    for name in [
        "fly-force-instance-id",
        "fly-prefer-instance-id",
        "fly-force-region",
    ] {
        headers.remove(name);
    }
    client_ip::sign(&mut headers, &proxy.signing_key);
    headers.insert("x-anarlog-proxy-hop", HeaderValue::from_static("1"));
    if let Some(host) = host {
        headers.insert("x-forwarded-host", host);
    }
    if websocket {
        headers.insert(header::CONNECTION, HeaderValue::from_static("upgrade"));
        headers.insert(header::UPGRADE, HeaderValue::from_static("websocket"));
    }
    let response = proxy
        .client
        .request(parts.method, url)
        .headers(headers)
        .body(reqwest::Body::wrap_stream(body.into_data_stream()))
        .send()
        .await;
    let response = match response {
        Ok(response) => response,
        Err(error) => {
            tracing::error!(error = %error.without_url(), "service_proxy_failed");
            return (StatusCode::BAD_GATEWAY, "Upstream unavailable").into_response();
        }
    };
    let status = response.status();
    let mut headers = response.headers().clone();
    strip_hop_headers(&mut headers);
    if status == StatusCode::SWITCHING_PROTOCOLS {
        let Some(downstream) = upgrade else {
            return StatusCode::BAD_GATEWAY.into_response();
        };
        headers.insert(header::CONNECTION, HeaderValue::from_static("upgrade"));
        headers.insert(header::UPGRADE, HeaderValue::from_static("websocket"));
        tokio::spawn(async move {
            let _permit = permit;
            match tokio::try_join!(
                async {
                    downstream
                        .await
                        .map(reqwest::Upgraded::from)
                        .map_err(|e| e.to_string())
                },
                async { response.upgrade().await.map_err(|e| e.to_string()) },
            ) {
                Ok((mut downstream, mut upstream)) => {
                    if let Err(error) =
                        tokio::io::copy_bidirectional(&mut downstream, &mut upstream).await
                    {
                        tracing::debug!(%error, "service_proxy_upgrade_closed");
                    }
                }
                Err(error) => tracing::warn!(%error, "service_proxy_upgrade_failed"),
            }
        });
        return (status, headers, Body::empty()).into_response();
    }
    // Keep the drain permit until the streamed response completes or the client disconnects.
    let stream = futures_util::stream::unfold(
        (response.bytes_stream(), permit),
        |(mut stream, permit)| async move { stream.next().await.map(|item| (item, (stream, permit))) },
    );
    (status, headers, Body::from_stream(stream)).into_response()
}

pub mod client_ip;

#[cfg(test)]
mod tests;
