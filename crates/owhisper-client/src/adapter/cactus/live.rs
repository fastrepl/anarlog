use hypr_ws_client::client::Message;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::StreamResponse;

use super::CactusAdapter;
use crate::adapter::RealtimeSttAdapter;
use crate::adapter::argmax::keywords::ArgmaxKeywordStrategy;
use crate::adapter::argmax::language::ArgmaxLanguageStrategy;
use crate::adapter::deepgram_compat::build_listen_ws_url;

impl RealtimeSttAdapter for CactusAdapter {
    fn provider_name(&self) -> &'static str {
        "cactus"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> bool {
        CactusAdapter::is_supported_languages_live(languages, model)
    }

    fn supports_native_multichannel(&self) -> bool {
        true
    }

    fn build_ws_url(&self, api_base: &str, params: &ListenParams, channels: u8) -> url::Url {
        build_listen_ws_url(
            api_base,
            params,
            channels,
            &ArgmaxLanguageStrategy,
            &ArgmaxKeywordStrategy,
        )
    }

    fn build_auth_header(&self, api_key: Option<&str>) -> Option<(&'static str, String)> {
        api_key.and_then(|k| crate::providers::Provider::Deepgram.build_auth_header(k))
    }

    fn keep_alive_message(&self) -> Option<Message> {
        Some(Message::Text(
            serde_json::to_string(&owhisper_interface::ControlMessage::KeepAlive)
                .unwrap()
                .into(),
        ))
    }

    fn finalize_message(&self) -> Message {
        Message::Text(
            serde_json::to_string(&owhisper_interface::ControlMessage::Finalize)
                .unwrap()
                .into(),
        )
    }

    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        match serde_json::from_str::<StreamResponse>(raw) {
            Ok(response) => vec![response],
            Err(_) => {
                if let Ok(error) = serde_json::from_str::<CactusError>(raw) {
                    tracing::error!(
                        error.type = %error.error_type,
                        error.message = %error.message,
                        "cactus_error"
                    );
                    vec![StreamResponse::ErrorResponse {
                        error_code: None,
                        error_message: format!("{}: {}", error.error_type, error.message),
                        provider: "cactus".to_string(),
                    }]
                } else {
                    tracing::warn!(hyprnote.payload.raw = raw, "cactus_unknown_message");
                    vec![]
                }
            }
        }
    }
}

#[derive(serde::Deserialize)]
struct CactusError {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}
