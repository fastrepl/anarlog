#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FoundationModelAvailability {
    pub status: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FoundationModelRequest {
    pub request_id: String,
    pub instructions: String,
    pub prompt: String,
    pub maximum_response_tokens: Option<u32>,
    pub temperature: Option<f64>,
    pub use_greedy_sampling: bool,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FoundationModelResponse {
    pub text: String,
}

#[derive(serde::Deserialize)]
struct NativeGenerationPayload {
    text: Option<String>,
    error: Option<String>,
}

pub fn availability() -> Result<FoundationModelAvailability, String> {
    #[cfg(target_os = "macos")]
    {
        platform::availability()
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(FoundationModelAvailability {
            status: "unsupported".to_string(),
            reason: Some("unsupported_os".to_string()),
        })
    }
}

pub async fn begin(request_id: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        tokio::task::spawn_blocking(move || platform::begin(&request_id))
            .await
            .map_err(|error| error.to_string())??;
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = request_id;
    }

    Ok(())
}

pub async fn cancel(request_id: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        tokio::task::spawn_blocking(move || platform::cancel(&request_id))
            .await
            .map_err(|error| error.to_string())??;
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = request_id;
    }

    Ok(())
}

pub async fn generate(request: FoundationModelRequest) -> Result<FoundationModelResponse, String> {
    #[cfg(target_os = "macos")]
    {
        let request_json = serde_json::to_string(&request).map_err(|error| error.to_string())?;
        let payload = tokio::task::spawn_blocking(move || platform::generate(&request_json))
            .await
            .map_err(|error| error.to_string())??;

        if let Some(error) = payload.error {
            return Err(error);
        }

        let text = payload
            .text
            .ok_or_else(|| "Apple Foundation Models returned no text.".to_string())?;
        Ok(FoundationModelResponse { text })
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = request;
        Err("Apple Foundation Models is only available on macOS.".to_string())
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use swift_rs::{SRString, swift};

    use super::{FoundationModelAvailability, NativeGenerationPayload};

    swift!(fn _foundation_model_availability() -> SRString);
    swift!(fn _foundation_model_begin(request_id: &SRString) -> SRString);
    swift!(fn _foundation_model_cancel(request_id: &SRString) -> SRString);
    swift!(fn _foundation_model_generate(request_json: &SRString) -> SRString);

    pub(super) fn availability() -> Result<FoundationModelAvailability, String> {
        let payload = unsafe { _foundation_model_availability() };
        serde_json::from_str(payload.as_str()).map_err(|error| error.to_string())
    }

    pub(super) fn begin(request_id: &str) -> Result<(), String> {
        let request_id = SRString::from(request_id);
        unsafe { _foundation_model_begin(&request_id) };
        Ok(())
    }

    pub(super) fn cancel(request_id: &str) -> Result<(), String> {
        let request_id = SRString::from(request_id);
        unsafe { _foundation_model_cancel(&request_id) };
        Ok(())
    }

    pub(super) fn generate(request_json: &str) -> Result<NativeGenerationPayload, String> {
        let request_json = SRString::from(request_json);
        let payload = unsafe { _foundation_model_generate(&request_json) };
        serde_json::from_str(payload.as_str()).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn reports_a_known_availability_status() {
        let availability = super::availability().unwrap();
        assert!(matches!(
            availability.status.as_str(),
            "available" | "unavailable" | "unsupported"
        ));
    }
}
