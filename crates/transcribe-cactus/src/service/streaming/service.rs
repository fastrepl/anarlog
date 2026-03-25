use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use axum::{
    body::Body,
    extract::{FromRequestParts, ws::WebSocketUpgrade},
    http::{Request, StatusCode, header},
    response::{IntoResponse, Response},
};
use tower::Service;

use hypr_model_manager::{ModelManager, TryGetResult};
use hypr_ws_utils::ConnectionManager;
use owhisper_interface::ListenParams;

use super::super::batch;
use super::session;
use crate::CactusConfig;

const MODEL_NAME: &str = "default";

#[derive(Clone)]
pub struct TranscribeService {
    model_manager: ModelManager<hypr_cactus::Model>,
    cactus_config: CactusConfig,
    connection_manager: ConnectionManager,
}

impl TranscribeService {
    pub fn builder() -> TranscribeServiceBuilder {
        TranscribeServiceBuilder::default()
    }

    pub fn model_manager(&self) -> &ModelManager<hypr_cactus::Model> {
        &self.model_manager
    }
}

#[derive(Default)]
pub struct TranscribeServiceBuilder {
    model_manager: Option<ModelManager<hypr_cactus::Model>>,
    cactus_config: CactusConfig,
    connection_manager: Option<ConnectionManager>,
}

impl TranscribeServiceBuilder {
    pub fn model_manager(mut self, model_manager: ModelManager<hypr_cactus::Model>) -> Self {
        self.model_manager = Some(model_manager);
        self
    }

    pub fn model_path(self, model_path: std::path::PathBuf) -> Self {
        let model_manager = ModelManager::<hypr_cactus::Model>::builder()
            .register(MODEL_NAME, &model_path)
            .default_model(MODEL_NAME)
            .build();
        self.model_manager(model_manager)
    }

    pub fn cactus_config(mut self, config: CactusConfig) -> Self {
        self.cactus_config = config;
        self
    }

    pub fn build(self) -> TranscribeService {
        TranscribeService {
            model_manager: self
                .model_manager
                .expect("TranscribeServiceBuilder requires model_manager"),
            cactus_config: self.cactus_config,
            connection_manager: self.connection_manager.unwrap_or_default(),
        }
    }
}

impl Service<Request<Body>> for TranscribeService {
    type Response = Response;
    type Error = String;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let model_manager = self.model_manager.clone();
        let cactus_config = self.cactus_config.clone();
        let connection_manager = self.connection_manager.clone();

        Box::pin(async move {
            let is_ws = req
                .headers()
                .get("upgrade")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.eq_ignore_ascii_case("websocket"))
                .unwrap_or(false);

            let query_string = req.uri().query().unwrap_or("");
            let params = match parse_listen_params(query_string) {
                Ok(p) => p,
                Err(e) => {
                    return Ok((StatusCode::BAD_REQUEST, e.to_string()).into_response());
                }
            };

            // Non-blocking model check — return 503 if still loading
            let model = match model_manager.try_get(None).await {
                TryGetResult::Ready(model) => model,
                TryGetResult::Loading => {
                    return Ok(Response::builder()
                        .status(StatusCode::SERVICE_UNAVAILABLE)
                        .header(header::RETRY_AFTER, "1")
                        .body(Body::from("model is loading"))
                        .unwrap());
                }
                TryGetResult::Failed(msg) => {
                    tracing::error!(error = %msg, "model_load_failed");
                    return Ok((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("failed to load model: {msg}"),
                    )
                        .into_response());
                }
                TryGetResult::NotRegistered => {
                    return Ok((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "model not registered".to_string(),
                    )
                        .into_response());
                }
            };

            let model_path = model_manager.get_default_path().await;
            let metadata = crate::service::build_metadata(model_path.as_deref());

            if is_ws {
                let (mut parts, _body) = req.into_parts();
                let ws_upgrade = match WebSocketUpgrade::from_request_parts(&mut parts, &()).await {
                    Ok(ws) => ws,
                    Err(e) => {
                        return Ok((StatusCode::BAD_REQUEST, e.to_string()).into_response());
                    }
                };

                let guard = connection_manager.acquire_connection();

                Ok(ws_upgrade
                    .on_upgrade(move |socket| async move {
                        session::handle_websocket(
                            socket,
                            params,
                            model,
                            metadata,
                            cactus_config,
                            guard,
                        )
                        .await;
                    })
                    .into_response())
            } else {
                let content_type = req
                    .headers()
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("application/octet-stream")
                    .to_string();

                let accept = req
                    .headers()
                    .get("accept")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_string();

                let body_bytes =
                    match axum::body::to_bytes(req.into_body(), 100 * 1024 * 1024).await {
                        Ok(b) => b,
                        Err(e) => {
                            return Ok((StatusCode::BAD_REQUEST, e.to_string()).into_response());
                        }
                    };

                if body_bytes.is_empty() {
                    return Ok((StatusCode::BAD_REQUEST, "request body is empty").into_response());
                }

                if accept.contains("text/event-stream") {
                    Ok(batch::handle_batch_sse(body_bytes, &content_type, &params, model).await)
                } else {
                    Ok(batch::handle_batch(body_bytes, &content_type, &params, model).await)
                }
            }
        })
    }
}

fn parse_listen_params(query: &str) -> Result<ListenParams, serde_html_form::de::Error> {
    serde_html_form::from_str(query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_language::ISO639;

    #[test]
    fn parse_single_language() {
        let params = parse_listen_params("language=en").unwrap();
        assert_eq!(params.languages.len(), 1);
        assert_eq!(params.languages[0].iso639(), ISO639::En);
    }

    #[test]
    fn parse_multiple_languages() {
        let params = parse_listen_params("language=en&language=ko").unwrap();
        assert_eq!(params.languages.len(), 2);
        assert_eq!(params.languages[0].iso639(), ISO639::En);
        assert_eq!(params.languages[1].iso639(), ISO639::Ko);
    }

    #[test]
    fn parse_no_languages() {
        let params = parse_listen_params("").unwrap();
        assert!(params.languages.is_empty());
    }

    #[test]
    fn parse_with_keywords() {
        let params = parse_listen_params("language=en&keywords=hello&keywords=world").unwrap();
        assert_eq!(params.languages.len(), 1);
        assert_eq!(params.keywords, vec!["hello", "world"]);
    }

    #[test]
    fn defaults_channels_and_sample_rate_when_omitted() {
        let params = parse_listen_params("language=en").unwrap();
        assert_eq!(params.channels, 1);
        assert_eq!(params.sample_rate, 16000);
    }
}
