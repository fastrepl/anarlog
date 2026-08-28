mod batch;
mod denoise;
mod error;
mod events;
mod runtime;
mod subtitle;

pub use batch::{
    BatchParams, BatchProvider, BatchRunMode, BatchRunOutput, expects_progressive_batch, run_batch,
};
pub use denoise::{DenoiseParams, run_denoise};
pub use error::*;
pub use events::*;
pub use runtime::*;
pub use subtitle::*;

use std::str::FromStr;

use owhisper_client::AdapterKind;

fn is_anarlog_provider(provider: &str) -> bool {
    matches!(provider, "anarlog" | "hyprnote")
}

pub fn is_supported_languages_live(
    provider: &str,
    model: Option<&str>,
    languages: &[anlg_language::Language],
) -> std::result::Result<bool, String> {
    if provider == "custom" {
        return Ok(true);
    }

    if provider == "soniqo" {
        let model = model
            .ok_or_else(|| "missing_model: soniqo".to_string())?
            .parse::<anlg_transcribe_soniqo::SoniqoModel>()
            .map_err(|e| e.to_string())?;

        return Ok(model.supports_live_on_current_platform() && model.supports_languages(languages));
    }

    if provider == "apple-speech" {
        let model = model
            .ok_or_else(|| "missing_model: apple-speech".to_string())?
            .parse::<anlg_transcribe_speechanalyzer::AppleSpeechModel>()
            .map_err(|e| e.to_string())?;

        return Ok(model.supports_live_on_current_platform() && model.supports_languages(languages));
    }

    if is_anarlog_provider(provider)
        && let Some(model) = model
        && model != "cloud"
    {
        if let Ok(model) = model.parse::<anlg_transcribe_soniqo::SoniqoModel>() {
            return Ok(
                model.supports_live_on_current_platform() && model.supports_languages(languages)
            );
        }

        if let Ok(model) = model.parse::<anlg_transcribe_speechanalyzer::AppleSpeechModel>() {
            return Ok(
                model.supports_live_on_current_platform() && model.supports_languages(languages)
            );
        }

        if model.starts_with("am-") || model.starts_with("whisper-") {
            return Ok(false);
        }
    }

    let adapter_provider = if is_anarlog_provider(provider) {
        "anarlog"
    } else {
        provider
    };
    let adapter_kind = AdapterKind::from_str(adapter_provider)
        .map_err(|_| format!("unknown_provider: {}", provider))?;

    Ok(adapter_kind.is_supported_languages_live(languages, model))
}

pub fn is_supported_languages_batch(
    provider: &str,
    model: Option<&str>,
    languages: &[anlg_language::Language],
) -> std::result::Result<bool, String> {
    if provider == "custom" {
        return Ok(true);
    }

    if provider == "soniqo" {
        let model = model
            .ok_or_else(|| "missing_model: soniqo".to_string())?
            .parse::<anlg_transcribe_soniqo::SoniqoModel>()
            .map_err(|e| e.to_string())?;

        return Ok(model.supports_languages(languages));
    }

    if provider == "apple-speech" {
        let model = model
            .ok_or_else(|| "missing_model: apple-speech".to_string())?
            .parse::<anlg_transcribe_speechanalyzer::AppleSpeechModel>()
            .map_err(|e| e.to_string())?;

        return Ok(model.supports_languages(languages));
    }

    if is_anarlog_provider(provider) {
        if let Some(model) =
            model.and_then(|model| model.parse::<anlg_transcribe_soniqo::SoniqoModel>().ok())
        {
            return Ok(model.supports_languages(languages));
        }

        if let Some(model) = model.and_then(|model| {
            model
                .parse::<anlg_transcribe_speechanalyzer::AppleSpeechModel>()
                .ok()
        }) {
            return Ok(model.supports_languages(languages));
        }

        return Ok(true);
    }

    let adapter_kind =
        AdapterKind::from_str(provider).map_err(|_| format!("unknown_provider: {}", provider))?;

    Ok(adapter_kind.is_supported_languages_batch(languages, model))
}

pub fn suggest_providers_for_languages_batch(languages: &[anlg_language::Language]) -> Vec<String> {
    let all_providers = [
        AdapterKind::Argmax,
        AdapterKind::Soniox,
        AdapterKind::Fireworks,
        AdapterKind::Deepgram,
        AdapterKind::AssemblyAI,
        AdapterKind::OpenAI,
        AdapterKind::OpenRouter,
        AdapterKind::SiliconFlow,
        AdapterKind::Zai,
        AdapterKind::Gladia,
        AdapterKind::ElevenLabs,
        AdapterKind::DashScope,
        AdapterKind::Mistral,
        AdapterKind::Cohere,
        AdapterKind::AwsTranscribe,
        AdapterKind::AzureSpeech,
        AdapterKind::GoogleCloud,
        AdapterKind::GoogleGenerativeAi,
        AdapterKind::Groq,
        AdapterKind::RevAi,
        AdapterKind::Speechmatics,
        AdapterKind::Together,
        AdapterKind::Xai,
    ];

    let mut with_support: Vec<_> = all_providers
        .iter()
        .map(|kind| {
            let support = kind.language_support_batch(languages, None);
            (*kind, support)
        })
        .filter(|(_, support)| support.is_supported())
        .collect();

    with_support.sort_by(|(_, s1), (_, s2)| s2.cmp(s1));

    with_support
        .into_iter()
        .map(|(kind, _)| kind.to_string())
        .collect()
}

pub fn list_documented_language_codes_batch() -> Vec<String> {
    owhisper_client::documented_language_codes_batch()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soniqo_batch_accepts_documented_european_languages_for_parakeet() {
        let languages = vec!["fr".parse().unwrap()];

        assert_eq!(
            is_supported_languages_batch("soniqo", Some("soniqo-parakeet-batch"), &languages)
                .unwrap(),
            true
        );
    }

    #[test]
    fn anarlog_soniqo_batch_rejects_unsupported_parakeet_languages() {
        let languages = vec!["ko".parse().unwrap()];

        assert_eq!(
            is_supported_languages_batch("anarlog", Some("soniqo-parakeet-batch"), &languages)
                .unwrap(),
            false
        );
    }

    #[test]
    fn soniqo_batch_accepts_non_english_for_multilingual_models() {
        let languages = vec!["fr".parse().unwrap()];

        assert!(
            is_supported_languages_batch("soniqo", Some("soniqo-omnilingual"), &languages).unwrap()
        );
    }

    #[test]
    fn anarlog_non_soniqo_batch_keeps_existing_language_support() {
        let languages = vec!["fr".parse().unwrap()];

        assert!(is_supported_languages_batch("anarlog", Some("cloud"), &languages).unwrap());
    }

    #[test]
    fn mistral_batch_accepts_regional_locales_for_documented_languages() {
        let languages = vec!["de-DE".parse().unwrap(), "en-US".parse().unwrap()];

        assert!(
            is_supported_languages_batch("mistral", Some("voxtral-mini-2602"), &languages).unwrap()
        );
    }

    #[test]
    fn apple_speech_language_support_reflects_installed_framework() {
        // Drives the settings warning that names unsupported spoken languages.
        let available = anlg_transcribe_speechanalyzer::availability()
            .is_ok_and(|value| value.status == "available");
        if !available {
            return;
        }

        let korean = vec!["ko".parse().unwrap()];
        let hindi = vec!["hi".parse().unwrap()];

        assert!(
            is_supported_languages_live("apple-speech", Some("apple-speech"), &korean).unwrap()
        );
        assert!(
            !is_supported_languages_live("apple-speech", Some("apple-speech"), &hindi).unwrap()
        );

        // Local models are surfaced under the Anarlog provider in settings.
        assert!(is_supported_languages_live("anarlog", Some("apple-speech"), &korean).unwrap());
        assert!(!is_supported_languages_live("anarlog", Some("apple-speech"), &hindi).unwrap());
        assert!(is_supported_languages_batch("anarlog", Some("apple-speech"), &korean).unwrap());
        assert!(!is_supported_languages_batch("anarlog", Some("apple-speech"), &hindi).unwrap());
    }

    #[test]
    fn anarlog_soniqo_live_rejects_unsupported_parakeet_languages() {
        let languages = vec!["ko".parse().unwrap()];

        assert_eq!(
            is_supported_languages_live("anarlog", Some("soniqo-parakeet-streaming"), &languages)
                .unwrap(),
            false
        );
    }

    #[test]
    fn anarlog_soniqo_live_respects_platform_support() {
        let languages = vec!["fr".parse().unwrap()];
        let expected = cfg!(all(target_os = "macos", target_arch = "aarch64"));

        assert_eq!(
            is_supported_languages_live("anarlog", Some("soniqo-parakeet-streaming"), &languages)
                .unwrap(),
            expected
        );
    }

    #[test]
    fn anarlog_cloud_live_keeps_existing_language_support() {
        let languages = vec!["ko".parse().unwrap()];

        assert!(is_supported_languages_live("anarlog", Some("cloud"), &languages).unwrap());
    }

    #[test]
    fn legacy_anarlog_provider_name_remains_supported() {
        let languages = vec!["ko".parse().unwrap()];

        assert!(is_supported_languages_live("hyprnote", Some("cloud"), &languages).unwrap());
        assert!(is_supported_languages_batch("hyprnote", Some("cloud"), &languages).unwrap());
    }
}
