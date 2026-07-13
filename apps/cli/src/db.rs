use std::path::{Path, PathBuf};

use crate::{Args, Error, Result};

pub async fn open(args: &Args) -> Result<hypr_db_core::Db> {
    let path = resolve_path(args)?;
    if !path.is_file() {
        return Err(Error::DatabaseNotFound(path));
    }

    hypr_db_core::Db::connect_local_read_only(&path)
        .await
        .map_err(|error| Error::operation("open database", error.to_string()))
}

pub(crate) fn resolve_path(args: &Args) -> Result<PathBuf> {
    if let Some(path) = &args.db_path {
        return Ok(path.clone());
    }
    if let Some(base) = &args.base {
        return Ok(base.join("app.db"));
    }

    let data_dir = dirs::data_dir().ok_or_else(|| {
        Error::operation("resolve database path", "data directory is unavailable")
    })?;
    Ok(resolve_default_path(&data_dir))
}

fn resolve_default_path(data_dir: &Path) -> PathBuf {
    let current = data_dir.join("anarlog").join("app.db");
    if current.is_file() {
        return current;
    }

    let legacy = data_dir.join("hyprnote").join("app.db");
    if legacy.is_file() {
        return legacy;
    }

    let identifier = data_dir.join("com.hyprnote.stable").join("app.db");
    if identifier.is_file() {
        return identifier;
    }

    current
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_path_prefers_current_then_legacy_then_identifier() {
        let dir = tempfile::tempdir().unwrap();
        let current = dir.path().join("anarlog/app.db");
        let legacy = dir.path().join("hyprnote/app.db");
        let identifier = dir.path().join("com.hyprnote.stable/app.db");

        std::fs::create_dir_all(identifier.parent().unwrap()).unwrap();
        std::fs::write(&identifier, "").unwrap();
        assert_eq!(resolve_default_path(dir.path()), identifier);

        std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        std::fs::write(&legacy, "").unwrap();
        assert_eq!(resolve_default_path(dir.path()), legacy);

        std::fs::create_dir_all(current.parent().unwrap()).unwrap();
        std::fs::write(&current, "").unwrap();
        assert_eq!(resolve_default_path(dir.path()), current);
    }

    #[test]
    fn default_path_targets_current_location_for_new_installs() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            resolve_default_path(dir.path()),
            dir.path().join("anarlog/app.db")
        );
    }
}
