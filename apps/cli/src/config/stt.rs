use std::path::PathBuf;

use hypr_listener2_core::{BatchParams, BatchProvider};
use hypr_local_model::LocalModel;
use hypr_local_stt_server::LocalSttServer;

use crate::commands::Provider;
use crate::config::cactus::{
    CACTUS_ENABLED, canonical_cactus_name, not_found_cactus_model, resolve_cactus_model,
    suggest_cactus_models, unsupported_cactus_error,
};
use crate::config::desktop;
use crate::error::{CliError, CliResult};

pub struct CactusServerInfo {
    pub server: LocalSttServer,
    pub base_url: String,
    pub model_name: String,
}

pub struct ResolvedSttConfig {
    pub provider: Provider,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub language: hypr_language::Language,
    pub server: Option<LocalSttServer>,
}

impl ResolvedSttConfig {
    pub fn model_option(&self) -> Option<String> {
        if self.model.is_empty() {
            None
        } else {
            Some(self.model.clone())
        }
    }

    pub fn batch_provider(&self) -> BatchProvider {
        self.provider.into()
    }

    pub fn to_batch_params(
        &self,
        session_id: String,
        file_path: String,
        keywords: Vec<String>,
    ) -> BatchParams {
        BatchParams {
            session_id,
            provider: self.batch_provider(),
            file_path,
            model: self.model_option(),
            base_url: self.base_url.clone(),
            api_key: self.api_key.clone(),
            languages: vec![self.language.clone()],
            keywords,
        }
    }
}

pub async fn resolve_config(
    provider: Provider,
    base_url: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
    language_code: impl Into<String>,
) -> CliResult<ResolvedSttConfig> {
    let language_code = language_code.into();
    let language = language_code
        .parse::<hypr_language::Language>()
        .map_err(|e| CliError::invalid_argument("--language", language_code, e.to_string()))?;

    if provider.is_local() {
        let info = resolve_and_spawn_cactus(model.as_deref()).await?;
        return Ok(ResolvedSttConfig {
            provider,
            base_url: info.base_url,
            api_key: api_key.unwrap_or_default(),
            model: info.model_name,
            language,
            server: Some(info.server),
        });
    }

    if let Some(cloud) = provider.cloud_provider() {
        let base_url = base_url.unwrap_or_else(|| cloud.default_api_base().to_string());
        let api_key = api_key
            .or_else(|| std::env::var(cloud.env_key_name()).ok())
            .ok_or_else(|| {
                CliError::required_argument_with_hint(
                    format!("--api-key (or {})", cloud.env_key_name()),
                    format!("Or set {} in your environment", cloud.env_key_name()),
                )
            })?;
        return Ok(ResolvedSttConfig {
            provider,
            base_url,
            api_key,
            model: model.unwrap_or_default(),
            language,
            server: None,
        });
    }

    let base_url =
        base_url.ok_or_else(|| CliError::required_argument("--base-url (or CHAR_BASE_URL)"))?;
    let api_key =
        api_key.ok_or_else(|| CliError::required_argument("--api-key (or CHAR_API_KEY)"))?;

    Ok(ResolvedSttConfig {
        provider,
        base_url,
        api_key,
        model: model.unwrap_or_default(),
        language,
        server: None,
    })
}

pub async fn resolve_and_spawn_cactus(model_name: Option<&str>) -> CliResult<CactusServerInfo> {
    let (model, model_path) = resolve_cactus_model(model_name)?;

    let server = LocalSttServer::start(model_path)
        .await
        .map_err(|e| CliError::operation_failed("start local cactus server", e.to_string()))?;

    Ok(CactusServerInfo {
        base_url: server.base_url().to_string(),
        model_name: model.to_string(),
        server,
    })
}

pub fn resolve_local_model_path(
    model_id: Option<&str>,
    model_path: Option<PathBuf>,
) -> CliResult<PathBuf> {
    if let Some(path) = model_path {
        if !path.exists() {
            return Err(CliError::not_found(
                format!("model path '{}'", path.display()),
                None,
            ));
        }
        return Ok(path);
    }

    if let Some(name) = model_id {
        return resolve_cactus_model_path(name);
    }

    Err(CliError::required_argument_with_hint(
        "--model or --model-path",
        suggest_cactus_models(),
    ))
}

pub fn resolve_cactus_model_path(name: &str) -> CliResult<PathBuf> {
    if !CACTUS_ENABLED {
        return Err(unsupported_cactus_error());
    }

    let paths = desktop::resolve_paths();
    let models_base = &paths.models_base;
    let canonical = canonical_cactus_name(name);

    let model = LocalModel::all()
        .into_iter()
        .find(|model| {
            matches!(model, LocalModel::Cactus(_))
                && (model.cli_name() == name || model.cli_name() == canonical)
        })
        .ok_or_else(|| not_found_cactus_model(name, true))?;

    let path = model.install_path(models_base);
    if !path.exists() {
        return Err(CliError::not_found(
            format!("cactus model '{name}' (not downloaded)"),
            Some(format!(
                "Download it first: char model cactus download {name}"
            )),
        ));
    }

    Ok(path)
}
