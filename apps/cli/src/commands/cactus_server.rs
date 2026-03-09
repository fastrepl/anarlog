use std::path::PathBuf;

use hypr_local_model::{CactusSttModel, LocalModel};
use hypr_local_stt_server::LocalSttServer;

use crate::commands::model::settings;
use crate::error::{CliError, CliResult};

fn default_cactus_model() -> CactusSttModel {
    if cfg!(target_arch = "aarch64") && cfg!(target_os = "macos") {
        CactusSttModel::WhisperSmallInt8Apple
    } else {
        CactusSttModel::WhisperSmallInt8
    }
}

fn resolve_cactus_model(name: Option<&str>) -> CliResult<(CactusSttModel, PathBuf)> {
    let paths = settings::resolve_paths();
    let models_base = paths.models_base;

    let model = match name {
        Some(name) => {
            let canonical = if name.starts_with("cactus-") {
                name.to_string()
            } else {
                format!("cactus-{name}")
            };

            LocalModel::all()
                .into_iter()
                .find_map(|m| match m {
                    LocalModel::Cactus(c) if m.cli_name() == name || m.cli_name() == canonical => {
                        Some(c)
                    }
                    _ => None,
                })
                .ok_or_else(|| {
                    CliError::not_found(
                        format!("cactus model '{name}'"),
                        Some("Run `char model cactus list` to see available models.".to_string()),
                    )
                })?
        }
        None => default_cactus_model(),
    };

    let model_path = LocalModel::Cactus(model.clone()).install_path(&models_base);
    if !model_path.exists() {
        return Err(CliError::not_found(
            format!("cactus model files at '{}'", model_path.display()),
            Some(format!(
                "Download it first: char model cactus download {}",
                model.display_name()
            )),
        ));
    }

    Ok((model, model_path))
}

pub async fn resolve_and_spawn_cactus(
    model_name: Option<&str>,
) -> CliResult<(LocalSttServer, String)> {
    let (_model, model_path) = resolve_cactus_model(model_name)?;

    let server = LocalSttServer::start(model_path)
        .await
        .map_err(|e| CliError::operation_failed("start local cactus server", e.to_string()))?;

    let base_url = server.base_url().to_string();
    Ok((server, base_url))
}
