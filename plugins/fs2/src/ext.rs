use std::path::{Path, PathBuf};

use tauri_plugin_settings::SettingsPluginExt;

pub struct Fs2<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Fs2<'a, R, M> {
    fn base(&self) -> Result<PathBuf, crate::Error> {
        self.manager
            .settings()
            .vault_base()
            .map(|p| p.into_std_path_buf())
            .map_err(|e| crate::Error::Path(e.to_string()))
    }

    fn validate_path(&self, path: &Path) -> Result<PathBuf, crate::Error> {
        let base = self.base()?;

        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            base.join(path)
        };

        let canonical_base = base.canonicalize().unwrap_or_else(|_| base.clone());

        let canonical_path =
            if resolved.exists() {
                resolved.canonicalize()?
            } else {
                let parent = resolved.parent().ok_or_else(|| {
                    crate::Error::Path("invalid path: no parent directory".to_string())
                })?;

                if parent.exists() {
                    let canonical_parent = parent.canonicalize()?;
                    canonical_parent.join(resolved.file_name().ok_or_else(|| {
                        crate::Error::Path("invalid path: no file name".to_string())
                    })?)
                } else {
                    resolved.clone()
                }
            };

        if canonical_path.starts_with(&canonical_base) {
            Ok(resolved)
        } else {
            Err(crate::Error::PathForbidden(resolved))
        }
    }

    pub fn read_text_file(&self, path: &Path) -> Result<String, crate::Error> {
        let validated_path = self.validate_path(path)?;
        let content = std::fs::read_to_string(&validated_path)?;
        Ok(content)
    }

    pub fn remove(&self, path: &Path) -> Result<(), crate::Error> {
        let validated_path = self.validate_path(path)?;

        if !validated_path.exists() {
            return Ok(());
        }

        let metadata = std::fs::symlink_metadata(&validated_path)?;

        if metadata.is_dir() {
            std::fs::remove_dir_all(&validated_path)?;
        } else {
            std::fs::remove_file(&validated_path)?;
        }

        Ok(())
    }
}

pub trait Fs2PluginExt<R: tauri::Runtime> {
    fn fs2(&self) -> Fs2<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> Fs2PluginExt<R> for T {
    fn fs2(&self) -> Fs2<'_, R, Self>
    where
        Self: Sized,
    {
        Fs2 {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
