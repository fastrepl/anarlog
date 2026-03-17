mod error;
mod model;
mod server;
mod store;

pub use error::*;
pub use model::*;
pub use server::*;
pub use store::*;

use std::path::Path;

pub fn is_model_downloaded(model: &SupportedModel, models_dir: &Path) -> Result<bool, Error> {
    let path = models_dir.join(model.file_name());

    if !path.exists() {
        return Ok(false);
    }

    let actual = hypr_file::file_size(&path)?;
    if actual != model.model_size() {
        return Ok(false);
    }

    Ok(true)
}

pub fn list_downloaded_models(models_dir: &Path) -> Result<Vec<SupportedModel>, Error> {
    if !models_dir.exists() {
        return Ok(vec![]);
    }

    let mut models = Vec::new();

    for entry in models_dir.read_dir()? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => {
                continue;
            }
        };

        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        if let Some(model) = SUPPORTED_MODELS
            .iter()
            .find(|model| model.file_name() == file_name_str)
            && entry.path().is_file()
        {
            models.push(model.clone());
        }
    }

    Ok(models)
}

pub fn list_custom_models() -> Result<Vec<CustomModelInfo>, Error> {
    #[cfg(target_os = "macos")]
    {
        let app_data_dir = dirs::data_dir().unwrap();
        let gguf_files = hypr_lmstudio::list_models(app_data_dir)?;

        let mut custom_models = Vec::new();
        for path_str in gguf_files {
            let path = std::path::Path::new(&path_str);
            if path.exists() {
                let name = {
                    use hypr_gguf::GgufExt;
                    path.model_name()
                };

                if let Ok(Some(name)) = name {
                    custom_models.push(CustomModelInfo {
                        path: path_str,
                        name,
                    });
                }
            }
        }
        Ok(custom_models)
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(Vec::new())
    }
}

pub fn get_current_model(
    store: &dyn ModelStore,
    models_dir: &Path,
) -> Result<SupportedModel, Error> {
    let model = store.get_model()?;

    match model {
        Some(existing_model) => Ok(existing_model),
        None => {
            let is_migrated = store.is_default_model_migrated()?;

            if is_migrated {
                Ok(SupportedModel::HyprLLM)
            } else {
                let old_model_path = models_dir.join(SupportedModel::Llama3p2_3bQ4.file_name());

                if old_model_path.exists() {
                    let _ = store.set_model(&SupportedModel::Llama3p2_3bQ4);
                    let _ = store.set_default_model_migrated(true);
                    Ok(SupportedModel::Llama3p2_3bQ4)
                } else {
                    let _ = store.set_default_model_migrated(true);
                    Ok(SupportedModel::HyprLLM)
                }
            }
        }
    }
}

pub fn set_current_model(store: &dyn ModelStore, model: SupportedModel) -> Result<(), Error> {
    store.set_model(&model)
}

pub fn get_current_model_selection(
    store: &dyn ModelStore,
    models_dir: &Path,
) -> Result<ModelSelection, Error> {
    if let Ok(Some(selection)) = store.get_model_selection() {
        return Ok(selection);
    }

    let current_model = get_current_model(store, models_dir)?;
    let selection = ModelSelection::Predefined { key: current_model };

    let _ = store.set_model_selection(&selection);
    Ok(selection)
}

pub fn set_current_model_selection(
    store: &dyn ModelStore,
    model: ModelSelection,
) -> Result<(), Error> {
    if let ModelSelection::Predefined { key } = &model {
        let _ = store.set_model(key);
    }

    store.set_model_selection(&model)
}
