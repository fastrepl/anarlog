//! Turns a finished recording into text the way shipping's `useRunBatch` does for dictation:
//! the selected speech-to-text provider when it can batch the spoken languages, otherwise Pro
//! cloud transcription or the on-device model. Dictionary terms are the only keywords.

use std::{path::Path, sync::Arc};

use anlg_listener2_core::{BatchEvent, BatchParams, BatchProvider, BatchRuntime, run_batch};
use desktop_runtime::{CancellationToken, Result, ServiceError};
use owhisper_interface::batch::Response;

use super::preview::PreviewConfig;
use crate::meeting::config::{
    Preferences, ProviderKind, ProviderServices, batch_provider, is_local, normalize_keywords,
};

fn failed(message: impl Into<String>) -> ServiceError {
    ServiceError::Failed(message.into().into())
}

#[derive(Clone, Debug)]
pub struct Target {
    pub provider: BatchProvider,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
}

impl Target {
    fn same(&self, other: &Self) -> bool {
        std::mem::discriminant(&self.provider) == std::mem::discriminant(&other.provider)
            && self.model == other.model
    }
}

/// The selected provider as batch transcription sees it.
pub struct Selected {
    pub target: Option<Target>,
    pub label: String,
    /// It can transcribe the spoken languages on this platform.
    pub supported: bool,
    /// An on-device model this platform cannot run.
    pub unavailable_here: bool,
}

/// Shipping's batch target choice; `fallback` is Pro cloud or the on-device model.
pub fn choose(selected: Selected, fallback: Option<Target>) -> Result<Target> {
    let use_selected = selected.supported
        || fallback
            .as_ref()
            .zip(selected.target.as_ref())
            .is_some_and(|(fallback, target)| fallback.same(target));
    let target = if use_selected {
        selected.target.clone().or(fallback)
    } else {
        fallback
    };
    target.ok_or_else(|| {
        failed(if selected.target.is_some() && !selected.unavailable_here {
            format!(
                "{} is not available for batch transcription with the selected languages. Choose languages it supports, or configure another speech-to-text provider.",
                selected.label
            )
        } else {
            format!(
                "{} is not available for batch transcription on this platform. Configure a batch-capable speech-to-text provider.",
                selected.label
            )
        })
    })
}

const LOCAL_AVAILABLE: bool = cfg!(all(target_os = "macos", target_arch = "aarch64"));

fn language_provider(provider: &str) -> &str {
    match provider {
        "local_file" => "anarlog",
        "custom" | "cloudflare_workers_ai" => "deepgram",
        "apple_speech" => "apple-speech",
        provider => provider,
    }
}

/// Settings dictation reads when it starts.
pub struct Request {
    pub preferences: Preferences,
}

impl Request {
    pub fn keywords(&self) -> Vec<String> {
        let mut keywords = self
            .preferences
            .strings("personalization_dictionary_terms")
            .unwrap_or_default();
        normalize_keywords(&mut keywords);
        keywords
    }

    pub fn languages(&self) -> Vec<anlg_language::Language> {
        self.preferences.languages().unwrap_or_default()
    }
}

/// The live preview connection, when the selected provider resolves.
pub async fn preview(services: &ProviderServices, request: &Request) -> Option<PreviewConfig> {
    let connection = services
        .connection(&request.preferences, ProviderKind::Stt, false)
        .await
        .ok()?;
    Some(PreviewConfig {
        provider: connection.provider,
        base_url: connection.base_url,
        api_key: connection.api_key,
        params: owhisper_interface::ListenParams {
            model: Some(connection.model),
            channels: 1,
            sample_rate: super::recorder::SAMPLE_RATE,
            languages: request.languages(),
            keywords: request.keywords(),
            num_speakers: Some(1),
            ..Default::default()
        },
    })
}

pub(crate) async fn target(services: &ProviderServices, request: &Request) -> Result<Target> {
    let preferences = &request.preferences;
    let provider = preferences.text("current_stt_provider");
    let model = preferences.text("current_stt_model");
    let label = if model.is_empty() && provider.is_empty() {
        "the selected speech-to-text provider".to_owned()
    } else if model.is_empty() {
        provider.clone()
    } else {
        model.clone()
    };
    let unavailable_here = is_local(&provider, &model) && !LOCAL_AVAILABLE;
    let batch = batch_provider(&provider, &model).ok();
    let target = match (&batch, unavailable_here) {
        (Some(batch), false) => services
            .connection(preferences, ProviderKind::Stt, true)
            .await
            .ok()
            .map(|connection| Target {
                provider: batch.clone(),
                model: connection.model,
                base_url: connection.base_url,
                api_key: connection.api_key,
            }),
        _ => None,
    };
    let supported = target.is_some()
        && anlg_listener2_core::is_supported_languages_batch(
            language_provider(&provider),
            Some(&model),
            &request.languages(),
        )
        .unwrap_or(true);
    let cloud = (services.cloud)()
        .await
        .ok()
        .filter(|access| access.is_paid && !access.access_token.is_empty());
    let fallback = match cloud {
        Some(access) => Some(Target {
            provider: BatchProvider::Anarlog,
            model: "cloud".into(),
            base_url: format!("{}/stt", services.api_url.trim_end_matches('/')),
            api_key: access.access_token,
        }),
        None if LOCAL_AVAILABLE => Some(Target {
            provider: BatchProvider::Soniqo,
            model: "soniqo-parakeet-batch".into(),
            base_url: "soniqo://local".into(),
            api_key: String::new(),
        }),
        None => None,
    };
    choose(
        Selected {
            target,
            label,
            supported,
            unavailable_here,
        },
        fallback,
    )
}

struct Batch(CancellationToken);

impl BatchRuntime for Batch {
    fn emit(&self, _: BatchEvent) {}
    fn is_cancelled(&self) -> bool {
        self.0.is_cancelled()
    }
}

pub async fn transcribe(
    services: &ProviderServices,
    request: &Request,
    owner: &str,
    path: &Path,
    cancellation: CancellationToken,
) -> Result<String> {
    let target = target(services, request).await?;
    let params = BatchParams {
        session_id: owner.into(),
        provider: target.provider,
        file_path: path
            .to_str()
            .ok_or_else(|| failed("Audio path is not valid UTF-8."))?
            .into(),
        model: Some(target.model),
        base_url: target.base_url,
        api_key: target.api_key,
        languages: request.languages(),
        keywords: request.keywords(),
        num_speakers: Some(1),
        min_speakers: None,
        max_speakers: None,
        known_speakers: Vec::new(),
    };
    let output = tokio::select! {
        _ = cancellation.cancelled() => return Err(ServiceError::Cancelled),
        result = run_batch(Arc::new(Batch(cancellation.clone())), params) => {
            result.map_err(|error| failed(error.to_string()))?
        }
    };
    Ok(text(&output.response))
}

/// The recording's words in spoken order as one line.
pub fn text(response: &Response) -> String {
    let mut words: Vec<_> = response
        .results
        .channels
        .iter()
        .filter_map(|channel| channel.alternatives.first())
        .flat_map(|alternative| alternative.words.iter())
        .collect();
    words.sort_by(|a, b| a.start.total_cmp(&b.start));
    words
        .iter()
        .map(|word| word.punctuated_word.as_deref().unwrap_or(&word.word))
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(provider: BatchProvider, model: &str) -> Target {
        Target {
            provider,
            model: model.into(),
            base_url: "https://example.test".into(),
            api_key: "key".into(),
        }
    }

    fn selected(target: Option<Target>, supported: bool) -> Selected {
        Selected {
            target,
            label: "nova-3".into(),
            supported,
            unavailable_here: false,
        }
    }

    #[test]
    fn the_selected_provider_is_used_only_when_it_supports_the_languages() {
        let deepgram = target(BatchProvider::Deepgram, "nova-3");
        let cloud = target(BatchProvider::Anarlog, "cloud");
        let model = |target: Result<Target>| target.unwrap().model;
        assert_eq!(
            model(choose(
                selected(Some(deepgram.clone()), true),
                Some(cloud.clone())
            )),
            "nova-3"
        );
        assert_eq!(
            model(choose(
                selected(Some(deepgram.clone()), false),
                Some(cloud.clone())
            )),
            "cloud"
        );
        assert_eq!(model(choose(selected(None, false), Some(cloud))), "cloud");
        let error = choose(selected(Some(deepgram), false), None).unwrap_err();
        assert!(error.to_string().contains(
            "nova-3 is not available for batch transcription with the selected languages"
        ));
        let mut local = selected(Some(target(BatchProvider::Soniqo, "soniqo-x")), false);
        local.unavailable_here = true;
        assert!(
            choose(local, None)
                .unwrap_err()
                .to_string()
                .contains("on this platform")
        );
    }

    #[test]
    fn words_are_joined_in_spoken_order() {
        let response: Response = serde_json::from_value(serde_json::json!({
            "metadata": {},
            "results": {"channels": [{"alternatives": [{
                "transcript": "",
                "confidence": 1.0,
                "words": [
                    {"word": "world", "punctuated_word": "world.", "start": 0.6, "end": 0.9, "confidence": 1.0, "speaker": null},
                    {"word": "hello", "punctuated_word": "Hello", "start": 0.1, "end": 0.5, "confidence": 1.0, "speaker": null},
                    {"word": " ", "start": 0.55, "end": 0.56, "confidence": 1.0, "speaker": null}
                ]
            }]}]}
        }))
        .unwrap();
        assert_eq!(text(&response), "Hello world.");
    }
}
