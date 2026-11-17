use desktop_runtime::Result;
use futures::{
    StreamExt,
    future::BoxFuture,
    stream::{self, BoxStream},
};

use super::ai::{ProviderAdapter, ProviderEvent, Request};

pub struct FoundationModel;

impl ProviderAdapter for FoundationModel {
    fn stream(
        &self,
        request: Request,
    ) -> BoxFuture<'static, Result<BoxStream<'static, Result<ProviderEvent>>>> {
        Box::pin(async move {
            let text = generate(request).await?;
            Ok(stream::iter([Ok(ProviderEvent::Text(text))]).boxed())
        })
    }
}

#[cfg(not(target_os = "macos"))]
async fn generate(_: Request) -> Result<String> {
    Err(desktop_runtime::ServiceError::Unsupported(
        "Apple Foundation Models requires macOS and Apple Intelligence.".into(),
    ))
}

#[cfg(target_os = "macos")]
async fn generate(request: Request) -> Result<String> {
    platform::generate(request).await
}

#[cfg(target_os = "macos")]
mod platform {
    use super::super::{
        ai::{Part, Request, Role},
        model::{MAX_TEXT, failure},
    };
    use desktop_runtime::{Result, ServiceError};
    use serde_json::{Value, json};
    use std::time::Duration;
    use swift_rs::{SRString, swift};

    swift!(fn _foundation_model_availability() -> SRString);
    swift!(fn _foundation_model_begin(request_id: &SRString) -> SRString);
    swift!(fn _foundation_model_cancel(request_id: &SRString) -> SRString);
    swift!(fn _foundation_model_generate(request_json: &SRString) -> SRString);

    struct Generation(String);
    impl Drop for Generation {
        fn drop(&mut self) {
            let id = self.0.clone();
            tokio::task::spawn_blocking(move || unsafe {
                _foundation_model_cancel(&SRString::from(id.as_str()));
            });
        }
    }

    pub async fn generate(request: Request) -> Result<String> {
        if request.cancellation.is_cancelled() {
            return Err(ServiceError::Cancelled);
        }
        let id = uuid::Uuid::new_v4().to_string();
        let worker_id = id.clone();
        tokio::task::spawn_blocking(move || {
            let available = unsafe { _foundation_model_availability() };
            let status: Value = serde_json::from_str(available.as_str()).map_err(failure)?;
            if status["status"] != "available" {
                return Err(failure(
                    "Enable Apple Intelligence and download its model before generating.",
                ));
            }
            unsafe {
                _foundation_model_begin(&SRString::from(worker_id.as_str()));
            }
            Ok(())
        })
        .await
        .map_err(failure)??;
        let _generation = Generation(id.clone());
        let mut instructions = Vec::new();
        let mut prompt = Vec::new();
        for message in request.messages {
            let text = message
                .parts
                .into_iter()
                .filter_map(|p| match p {
                    Part::Text { text } => Some(text),
                    Part::Tool { output, .. } => Some(output.to_string()),
                    Part::Reasoning { .. } => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            if message.role == Role::System {
                instructions.push(text);
            } else {
                prompt.push(format!(
                    "{}: {text}",
                    if message.role == Role::User {
                        "User"
                    } else {
                        "Assistant"
                    }
                ));
            }
        }
        let payload = json!({"requestId":id,"instructions":instructions.join("\n\n"),"prompt":prompt.join("\n\n"),
            "maximumResponseTokens":4096,"useGreedySampling":false}).to_string();
        if payload.len() > MAX_TEXT {
            return Err(failure("Foundation Model input exceeds context limit."));
        }
        let generate = tokio::task::spawn_blocking(move || {
            let response = unsafe { _foundation_model_generate(&SRString::from(payload.as_str())) };
            let response: Value = serde_json::from_str(response.as_str()).map_err(failure)?;
            if response["error"].is_string() {
                return Err(failure("Foundation Model generation failed."));
            }
            response["text"]
                .as_str()
                .filter(|s| s.len() <= MAX_TEXT)
                .map(str::to_owned)
                .ok_or_else(|| failure("Invalid Foundation Model output."))
        });
        tokio::select! {
            _ = request.cancellation.cancelled() => Err(ServiceError::Cancelled),
            result = tokio::time::timeout(Duration::from_secs(300), generate) => result.map_err(|_| failure("Foundation Model generation timed out."))?.map_err(failure)?,
        }
    }
}
