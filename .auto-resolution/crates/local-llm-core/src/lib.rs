mod error;
mod model;
mod server;

pub use error::*;
pub use model::*;
pub use server::*;

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
        let app_data_dir = dirs::data_dir()
            .ok_or_else(|| Error::Other("application data directory is unavailable".to_string()))?;
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
