use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
};
use tower::Service;

use crate::ModelManager;

use self::{
    request::{ChatCompletionRequest, build_options, convert_messages},
    response::{
        build_non_streaming_response, build_streaming_response, status_code_for_model_error,
    },
};

mod request;
mod response;

type CactusModelManager = ModelManager<hypr_cactus::Model>;

#[derive(Clone)]
pub struct CompleteService {
    manager: CactusModelManager,
}

impl CompleteService {
    pub fn new(manager: CactusModelManager) -> Self {
        Self { manager }
    }
}

impl Service<Request<Body>> for CompleteService {
    type Response = Response;
    type Error = crate::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let manager = self.manager.clone();

        Box::pin(async move {
            let body_bytes = match axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024).await {
                Ok(b) => b,
                Err(e) => {
                    return Ok((StatusCode::BAD_REQUEST, e.to_string()).into_response());
                }
            };

            let request: ChatCompletionRequest = match serde_json::from_slice(&body_bytes) {
                Ok(r) => r,
                Err(e) => {
                    return Ok((StatusCode::BAD_REQUEST, e.to_string()).into_response());
                }
            };

            let messages = match convert_messages(&request.messages) {
                Ok(messages) => messages,
                Err(error) => {
                    return Ok((StatusCode::BAD_REQUEST, error).into_response());
                }
            };
            if let Err(error) = hypr_cactus::validate_messages(&messages) {
                return Ok((status_code_for_model_error(&error), error.to_string()).into_response());
            }
            let options = build_options(&request);

            let model = match manager.get(request.model.as_deref()).await {
                Ok(m) => m,
                Err(e) => {
                    return Ok((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response());
                }
            };

            if request.stream.unwrap_or(false) {
                let completion_stream =
                    match hypr_cactus::complete_stream(&model, messages, options) {
                        Ok(s) => s,
                        Err(e) => {
                            return Ok(
                                (status_code_for_model_error(&e), e.to_string()).into_response()
                            );
                        }
                    };

                Ok(build_streaming_response(completion_stream, &request.model))
            } else {
                Ok(build_non_streaming_response(&model, messages, options, &request.model).await)
            }
        })
    }
}
