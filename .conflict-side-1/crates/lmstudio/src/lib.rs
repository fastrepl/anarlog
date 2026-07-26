use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

pub fn list_models(app_data_dir: PathBuf) -> Result<Vec<String>, Error> {
    let config_path = app_data_dir.join("LM Studio").join("settings.json");

    let config = match std::fs::read_to_string(config_path) {
        Ok(content) => content,
        Err(_) => return Ok(Vec::new()),
    };

    let config: serde_json::Value = match serde_json::from_str(&config) {
        Ok(json) => json,
        Err(_) => return Ok(Vec::new()),
    };

    let models_path = match config["downloadsFolder"].as_str() {
        Some(path) => path,
        None => return Ok(Vec::new()),
    };

    let files = {
        let mut files = walk_directory_for_gguf(models_path)?
            .into_iter()
            .map(|path| {
                let path_str = path.to_str().unwrap_or_default();
                let file_size = fs::metadata(&path).map(|v| v.len()).unwrap_or(0);
                (path_str.to_string(), file_size)
            })
            .filter(|(f, _)| !f.contains("mmproj") && !f.contains("embedding"))
            .collect::<Vec<_>>();
        files.sort_by(|a, b| b.1.cmp(&a.1));
        files.into_iter().map(|(path, _)| path).collect()
    };

    Ok(files)
}

fn walk_directory_for_gguf(path: impl AsRef<Path>) -> Result<Vec<PathBuf>, Error> {
    let dir = path.as_ref();
    let mut gguf_files = Vec::new();

    if dir.is_dir() {
        let entries = std::fs::read_dir(dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let mut sub_files = walk_directory_for_gguf(path)?;
                gguf_files.append(&mut sub_files);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("gguf") {
                gguf_files.push(path);
            }
        }
    }
    Ok(gguf_files)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_config() {
        let app_data_dir = dirs::data_dir().unwrap();
        let config_path = app_data_dir.join("LM Studio").join("settings.json");
        match std::fs::read_to_string(config_path) {
            Ok(config) => println!("config: {:#?}", config),
            Err(_) => println!("LM Studio config not found, skipping"),
        }
    }

    #[test]
    fn test_list_models() {
        let app_data_dir = dirs::data_dir().unwrap();
        let gguf_files = list_models(app_data_dir).unwrap();
        println!("gguf_files: {:#?}", gguf_files);
    }
}
