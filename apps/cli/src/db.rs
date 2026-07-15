use std::ffi::OsStr;
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
    let command_name = std::env::args_os()
        .next()
        .and_then(|path| Path::new(&path).file_name().map(|name| name.to_owned()));
    resolve_default_path_for_command(data_dir, command_name.as_deref())
}

fn resolve_default_path_for_command(data_dir: &Path, command_name: Option<&OsStr>) -> PathBuf {
    let channel_identifier = match command_name.and_then(OsStr::to_str) {
        Some("anarlog-dev") => Some("com.hyprnote.dev"),
        Some("anarlog-staging") => Some("com.hyprnote.staging"),
        _ => None,
    };
    if let Some(identifier) = channel_identifier {
        return data_dir.join(identifier).join("app.db");
    }

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
        assert_eq!(
            resolve_default_path_for_command(dir.path(), Some(OsStr::new("anarlog"))),
            identifier
        );

        std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        std::fs::write(&legacy, "").unwrap();
        assert_eq!(
            resolve_default_path_for_command(dir.path(), Some(OsStr::new("anarlog"))),
            legacy
        );

        std::fs::create_dir_all(current.parent().unwrap()).unwrap();
        std::fs::write(&current, "").unwrap();
        assert_eq!(
            resolve_default_path_for_command(dir.path(), Some(OsStr::new("anarlog"))),
            current
        );
    }

    #[test]
    fn default_path_targets_current_location_for_new_installs() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            resolve_default_path_for_command(dir.path(), Some(OsStr::new("anarlog"))),
            dir.path().join("anarlog/app.db")
        );
    }

    #[test]
    fn channel_commands_target_their_channel_database() {
        let dir = tempfile::tempdir().unwrap();
        let stable = dir.path().join("anarlog/app.db");
        std::fs::create_dir_all(stable.parent().unwrap()).unwrap();
        std::fs::write(stable, "").unwrap();

        assert_eq!(
            resolve_default_path_for_command(dir.path(), Some(OsStr::new("anarlog-dev"))),
            dir.path().join("com.hyprnote.dev/app.db")
        );
        assert_eq!(
            resolve_default_path_for_command(dir.path(), Some(OsStr::new("anarlog-staging"))),
            dir.path().join("com.hyprnote.staging/app.db")
        );
    }
}
