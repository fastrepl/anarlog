use std::path::PathBuf;

use hypr_local_model::{CactusSttModel, LocalModel};

use crate::config::desktop;
use crate::error::{CliError, CliResult, did_you_mean};

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
pub const CACTUS_ENABLED: bool = true;
#[cfg(not(any(target_arch = "arm", target_arch = "aarch64")))]
pub const CACTUS_ENABLED: bool = false;

pub fn is_cactus_local_model(model: &LocalModel) -> bool {
    matches!(model, LocalModel::Cactus(_) | LocalModel::CactusLlm(_))
}

pub fn unsupported_cactus_error() -> CliError {
    CliError::msg("cactus local models are only available on ARM devices")
}

pub fn canonical_cactus_name(name: &str) -> String {
    if name.starts_with("cactus-") {
        name.to_string()
    } else {
        format!("cactus-{name}")
    }
}

pub fn all_cactus_models() -> Vec<LocalModel> {
    if !CACTUS_ENABLED {
        return Vec::new();
    }

    LocalModel::all()
        .into_iter()
        .filter(|model| model.cli_name().starts_with("cactus-"))
        .collect()
}

pub fn find_cactus_model(name: &str) -> Option<LocalModel> {
    let canonical = canonical_cactus_name(name);
    all_cactus_models()
        .into_iter()
        .find(|model| model.cli_name() == name || model.cli_name() == canonical)
}

pub fn default_cactus_model() -> CactusSttModel {
    if cfg!(target_arch = "aarch64") && cfg!(target_os = "macos") {
        CactusSttModel::WhisperSmallInt8Apple
    } else {
        CactusSttModel::WhisperSmallInt8
    }
}

pub fn not_found_cactus_model(name: &str, include_downloaded_hint: bool) -> CliError {
    if !CACTUS_ENABLED {
        return unsupported_cactus_error();
    }

    let names: Vec<&str> = LocalModel::all()
        .iter()
        .filter_map(|model| {
            if matches!(model, LocalModel::Cactus(_)) {
                Some(model.cli_name())
            } else {
                None
            }
        })
        .collect();

    let mut hint = String::new();
    if let Some(suggestion) = did_you_mean(name, &names) {
        hint.push_str(&format!("Did you mean '{suggestion}'?\n\n"));
    }
    if include_downloaded_hint {
        hint.push_str(&suggest_cactus_models());
    } else {
        hint.push_str("Run `char model cactus list` to see available models.");
    }

    CliError::not_found(format!("cactus model '{name}'"), Some(hint))
}

pub fn suggest_cactus_models() -> String {
    if !CACTUS_ENABLED {
        return "Cactus local models are only available on ARM devices.".to_string();
    }

    let models_base = desktop::resolve_paths().models_base;
    let mut downloaded = Vec::new();
    let mut available = Vec::new();

    for model in LocalModel::all() {
        let LocalModel::Cactus(_) = &model else {
            continue;
        };

        if model.install_path(&models_base).exists() {
            downloaded.push(model.cli_name());
        } else {
            available.push(model.cli_name());
        }
    }

    let mut hint = String::new();
    if !downloaded.is_empty() {
        hint.push_str("Downloaded models:\n");
        for name in &downloaded {
            hint.push_str(&format!("  {name}\n"));
        }
    }
    if !available.is_empty() {
        if !downloaded.is_empty() {
            hint.push_str("Other models (not downloaded):\n");
        } else {
            hint.push_str("No models downloaded. Available models:\n");
        }
        for name in &available {
            hint.push_str(&format!("  {name}\n"));
        }
        hint.push_str("Download with: char model cactus download <name>");
    }
    if hint.is_empty() {
        hint.push_str("No cactus models found. Run `char model cactus list` to check.");
    }
    hint
}

pub fn resolve_cactus_model(name: Option<&str>) -> CliResult<(CactusSttModel, PathBuf)> {
    if !CACTUS_ENABLED {
        return Err(unsupported_cactus_error());
    }

    let models_base = desktop::resolve_paths().models_base;

    let model = match name {
        Some(name) => {
            let canonical = canonical_cactus_name(name);
            LocalModel::all()
                .into_iter()
                .find_map(|model| match model {
                    LocalModel::Cactus(cactus)
                        if model.cli_name() == name || model.cli_name() == canonical =>
                    {
                        Some(cactus)
                    }
                    _ => None,
                })
                .ok_or_else(|| not_found_cactus_model(name, false))?
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
