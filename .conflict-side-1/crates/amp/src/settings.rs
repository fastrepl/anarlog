use std::path::{Path, PathBuf};

use tempfile::TempDir;

use crate::error::Error;

#[derive(Debug)]
pub(crate) struct SettingsFile {
    _temp_dir: TempDir,
    path: PathBuf,
}

impl SettingsFile {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

pub(crate) fn create_settings_file(
    settings: Option<&serde_json::Value>,
) -> Result<Option<SettingsFile>, Error> {
    let Some(settings) = settings else {
        return Ok(None);
    };

    let temp_dir = tempfile::tempdir().map_err(Error::SettingsIo)?;
    let path = temp_dir.path().join("settings.json");
    let contents = serde_json::to_vec(settings)?;
    std::fs::write(&path, contents).map_err(Error::SettingsIo)?;

    Ok(Some(SettingsFile {
        _temp_dir: temp_dir,
        path,
    }))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::create_settings_file;

    #[test]
    fn creates_and_removes_temp_settings_file() {
        let settings_file = create_settings_file(Some(&json!({
            "theme": "dark"
        })))
        .expect("settings file should be created")
        .expect("settings file should exist");

        let settings_path = settings_file.path().to_path_buf();
        let settings_dir = settings_path
            .parent()
            .expect("settings file should have a parent directory")
            .to_path_buf();

        assert!(settings_path.is_file());
        drop(settings_file);
        assert!(!settings_dir.exists());
    }
}
