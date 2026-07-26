use std::str::FromStr;

use owhisper_client::{AdapterKind, DeepgramModel, Provider, is_meta_model};
use owhisper_interface::ListenParams;

fn should_override_deepgram_model(model: &str, languages: &[hypr_language::Language]) -> bool {
    if let Ok(parsed_model) = DeepgramModel::from_str(model) {
        !languages
            .iter()
            .all(|lang| parsed_model.supports_language(lang))
    } else {
        false
    }
}

fn resolve_model_with_mode(provider: Provider, listen_params: &mut ListenParams, for_batch: bool) {
    let needs_resolution = match &listen_params.model {
        None => true,
        Some(m) if is_meta_model(m) => true,
        Some(model) if provider == Provider::Deepgram => {
            should_override_deepgram_model(model, &listen_params.languages)
        }
        _ => false,
    };

    if needs_resolution {
        let model = if for_batch {
            AdapterKind::from(provider).recommended_model_batch(&listen_params.languages)
        } else {
            AdapterKind::from(provider).recommended_model_live(&listen_params.languages)
        };
        listen_params.model = model.map(|m| m.to_string());
    }
}

pub(super) fn resolve_model_live(provider: Provider, listen_params: &mut ListenParams) {
    resolve_model_with_mode(provider, listen_params, false);
}

pub(super) fn resolve_model_batch(provider: Provider, listen_params: &mut ListenParams) {
    resolve_model_with_mode(provider, listen_params, true);
}
