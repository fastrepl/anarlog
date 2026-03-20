use sqlx::SqlitePool;

use crate::config::paths;
use crate::error::{CliError, CliResult};

use super::LlmProvider;

#[derive(Clone, Debug)]
pub struct ResolvedLlmConfig {
    pub provider: LlmProvider,
    pub api_key: String,
    pub model: String,
    pub base_url: String,
}

pub async fn resolve_config(
    pool: &SqlitePool,
    provider_override: Option<LlmProvider>,
    base_url_override: Option<String>,
    api_key_override: Option<String>,
    model_override: Option<String>,
) -> CliResult<ResolvedLlmConfig> {
    let settings = paths::load_settings_from_db(pool).await;
    let current_provider_id = settings
        .as_ref()
        .and_then(|value| value.current_llm_provider.as_deref());
    let provider = match provider_override {
        Some(provider) => provider,
        None => resolve_provider_from_settings(current_provider_id)?,
    };
    let saved_provider = settings
        .as_ref()
        .and_then(|value| value.llm_providers.get(provider.id()));

    let api_key = api_key_override
        .or_else(|| saved_provider.and_then(|value| value.api_key.clone()))
        .ok_or_else(|| {
            CliError::required_argument_with_hint(
                "--api-key",
                format!(
                    "Pass --api-key, or run `char connect --type llm --provider {}` first.",
                    provider.id()
                ),
            )
        })?;
    let model = match model_override {
        Some(model) => model,
        None => resolve_model_from_settings(&settings, provider, current_provider_id)?,
    };
    let base_url = normalize_base_url(
        provider,
        base_url_override
            .or_else(|| saved_provider.and_then(|value| value.base_url.clone()))
            .unwrap_or_else(|| provider.default_base_url().to_string()),
    );

    Ok(ResolvedLlmConfig {
        provider,
        api_key,
        model,
        base_url,
    })
}

fn resolve_provider_from_settings(current_provider_id: Option<&str>) -> CliResult<LlmProvider> {
    let Some(current_provider_id) = current_provider_id else {
        return Err(CliError::required_argument_with_hint(
            "--provider",
            "Pass --provider anthropic|openai|openrouter, or run `char connect --type llm --provider ...` first.",
        ));
    };

    LlmProvider::from_id(current_provider_id).ok_or_else(|| {
        CliError::invalid_argument_with_hint(
            "--provider",
            current_provider_id,
            "chat currently supports anthropic, openai, and openrouter",
            "Pass --provider anthropic|openai|openrouter, or set one of them as the current LLM provider in your desktop settings.",
        )
    })
}

fn resolve_model_from_settings(
    settings: &Option<paths::Settings>,
    provider: LlmProvider,
    current_provider_id: Option<&str>,
) -> CliResult<String> {
    let Some(settings) = settings.as_ref() else {
        return Err(CliError::required_argument_with_hint(
            "--model",
            "Pass --model <name>, or configure a current LLM model in the desktop app.",
        ));
    };

    if current_provider_id == Some(provider.id())
        && let Some(model) = settings.current_llm_model.clone()
        && !model.trim().is_empty()
    {
        return Ok(model);
    }

    Err(CliError::required_argument_with_hint(
        "--model",
        format!(
            "Pass --model <name>. The saved current LLM model only applies when the selected provider matches `{}`.",
            provider.id()
        ),
    ))
}

fn normalize_base_url(provider: LlmProvider, value: String) -> String {
    let trimmed = value.trim().trim_end_matches('/').to_string();
    if provider == LlmProvider::Anthropic {
        trimmed
            .strip_suffix("/v1")
            .map(ToString::to_string)
            .unwrap_or(trimmed)
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_anthropic_base_url_strips_v1_suffix() {
        assert_eq!(
            normalize_base_url(
                LlmProvider::Anthropic,
                "https://api.anthropic.com/v1".to_string()
            ),
            "https://api.anthropic.com"
        );
    }

    #[test]
    fn normalize_openrouter_base_url_preserves_api_path() {
        assert_eq!(
            normalize_base_url(
                LlmProvider::Openrouter,
                "https://openrouter.ai/api/v1".to_string()
            ),
            "https://openrouter.ai/api/v1"
        );
    }
}
