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

pub fn is_supported_languages_live(
    provider: &str,
    model: Option<&str>,
    languages: &[hypr_language::Language],
) -> std::result::Result<bool, String> {
    if provider == "custom" {
        return Ok(true);
    }

    if provider == "soniqo" {
        let model = model
            .ok_or_else(|| "missing_model: soniqo".to_string())?
            .parse::<hypr_transcribe_soniqo::SoniqoModel>()
            .map_err(|e| e.to_string())?;

        return Ok(model.supports_live_on_current_platform() && model.supports_languages(languages));
    }

    if provider == "hyprnote"
        && let Some(model) = model
        && model != "cloud"
    {
        if let Ok(model) = model.parse::<hypr_transcribe_soniqo::SoniqoModel>() {
            return Ok(
                model.supports_live_on_current_platform() && model.supports_languages(languages)
            );
        }

        if model.starts_with("am-") || model.starts_with("whisper-") || model.starts_with("cactus-")
        {
            return Ok(false);
        }
    }

    let adapter_kind =
        AdapterKind::from_str(provider).map_err(|_| format!("unknown_provider: {}", provider))?;

    Ok(adapter_kind.is_supported_languages_live(languages, model))
}

pub fn is_supported_languages_batch(
    provider: &str,
    model: Option<&str>,
    languages: &[hypr_language::Language],
) -> std::result::Result<bool, String> {
    if provider == "custom" {
        return Ok(true);
    }

    if provider == "soniqo" {
        let model = model
            .ok_or_else(|| "missing_model: soniqo".to_string())?
            .parse::<hypr_transcribe_soniqo::SoniqoModel>()
            .map_err(|e| e.to_string())?;

        return Ok(model.supports_languages(languages));
    }

    if provider == "hyprnote" {
        if let Some(model) =
            model.and_then(|model| model.parse::<hypr_transcribe_soniqo::SoniqoModel>().ok())
        {
            return Ok(model.supports_languages(languages));
        }

        return Ok(true);
    }

    let adapter_kind =
        AdapterKind::from_str(provider).map_err(|_| format!("unknown_provider: {}", provider))?;

    Ok(adapter_kind.is_supported_languages_batch(languages, model))
}

pub fn suggest_providers_for_languages_batch(languages: &[hypr_language::Language]) -> Vec<String> {
    let all_providers = [
        AdapterKind::Argmax,
        AdapterKind::Soniox,
        AdapterKind::Fireworks,
        AdapterKind::Deepgram,
        AdapterKind::AssemblyAI,
        AdapterKind::OpenAI,
        AdapterKind::Gladia,
        AdapterKind::ElevenLabs,
        AdapterKind::DashScope,
        AdapterKind::Mistral,
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
        .map(|(kind, _)| format!("{:?}", kind).to_lowercase())
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
    fn hyprnote_soniqo_batch_rejects_unsupported_parakeet_languages() {
        let languages = vec!["ko".parse().unwrap()];

        assert_eq!(
            is_supported_languages_batch("hyprnote", Some("soniqo-parakeet-batch"), &languages)
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
    fn hyprnote_non_soniqo_batch_keeps_existing_language_support() {
        let languages = vec!["fr".parse().unwrap()];

        assert!(is_supported_languages_batch("hyprnote", Some("cloud"), &languages).unwrap());
    }

    #[test]
    fn hyprnote_soniqo_live_rejects_unsupported_parakeet_languages() {
        let languages = vec!["ko".parse().unwrap()];

        assert_eq!(
            is_supported_languages_live("hyprnote", Some("soniqo-parakeet-streaming"), &languages)
                .unwrap(),
            false
        );
    }

    #[test]
    fn hyprnote_soniqo_live_respects_platform_support() {
        let languages = vec!["fr".parse().unwrap()];
        let expected = cfg!(all(target_os = "macos", target_arch = "aarch64"));

        assert_eq!(
            is_supported_languages_live("hyprnote", Some("soniqo-parakeet-streaming"), &languages)
                .unwrap(),
            expected
        );
    }

    #[test]
    fn hyprnote_cloud_live_keeps_existing_language_support() {
        let languages = vec!["ko".parse().unwrap()];

        assert!(is_supported_languages_live("hyprnote", Some("cloud"), &languages).unwrap());
    }
}
