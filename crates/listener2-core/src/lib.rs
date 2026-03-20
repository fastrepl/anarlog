mod batch;
mod denoise;
mod error;
mod events;
mod runtime;
mod subtitle;

pub use batch::{BatchParams, BatchProvider, BatchRunMode, BatchRunOutput, run_batch};
pub use denoise::{DenoiseParams, run_denoise};
pub use error::*;
pub use events::*;
pub use runtime::*;
pub use subtitle::*;

use std::str::FromStr;

use owhisper_client::AdapterKind;

pub fn is_supported_languages_batch(
    provider: &str,
    model: Option<&str>,
    languages: &[hypr_language::Language],
) -> std::result::Result<bool, String> {
    if provider == "custom" || provider == "hyprnote" {
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
