use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::{Args, Error, Result};

pub async fn open(args: &Args) -> Result<anlg_db_core::Db> {
    let path = resolve_path(args)?;
    if !path.is_file() {
        return Err(Error::DatabaseNotFound(path));
    }

    anlg_db_core::Db::connect_local_read_only(&path)
        .await
        .map_err(|error| Error::operation("open database", error.to_string()))
}

pub async fn open_write(args: &Args) -> Result<anlg_db_core::Db> {
    let path = resolve_path(args)?;
    if !path.is_file() {
        return Err(Error::DatabaseNotFound(path));
    }

    anlg_db_core::Db::connect_local_read_write(&path)
        .await
        .map_err(|error| Error::operation("open database for writes", error.to_string()))
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
    let command_name = command_name
        .and_then(OsStr::to_str)
        .and_then(|name| Path::new(name).file_stem())
        .and_then(OsStr::to_str);
    // `anarlog-nightly` falls through: the Nightly desktop app opens the same
    // database as stable.
    // Each channel lists the identifier it ships under today first, then the one
    // it shipped under before the fork renamed it. The desktop app applies the
    // same fallback, so both sides keep opening the one database that exists.
    let channel_identifiers: &[&str] = match command_name {
        Some("anarlog-dev") => &["com.blackmushi.dev", "com.hyprnote.dev"],
        Some("anarlog-staging") => &["com.blackmushi.staging", "com.hyprnote.staging"],
        _ => &[],
    };
    if let Some(first) = channel_identifiers.first() {
        let existing = channel_identifiers
            .iter()
            .map(|identifier| data_dir.join(identifier).join("app.db"))
            .find(|path| path.is_file());
        return existing.unwrap_or_else(|| data_dir.join(first).join("app.db"));
    }

    let current = data_dir.join("anarlog").join("app.db");
    if current.is_file() {
        return current;
    }

    let legacy = data_dir.join("hyprnote").join("app.db");
    if legacy.is_file() {
        return legacy;
    }

    for identifier in ["com.blackmushi.stable", "com.hyprnote.stable"] {
        let candidate = data_dir.join(identifier).join("app.db");
        if candidate.is_file() {
            return candidate;
        }
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
    fn nightly_command_targets_the_stable_database() {
        let dir = tempfile::tempdir().unwrap();
        let stable = dir.path().join("anarlog/app.db");
        std::fs::create_dir_all(stable.parent().unwrap()).unwrap();
        std::fs::write(&stable, "").unwrap();

        for command in ["anarlog-nightly", "anarlog-nightly.exe"] {
            assert_eq!(
                resolve_default_path_for_command(dir.path(), Some(OsStr::new(command))),
                stable
            );
        }
    }

    #[test]
    fn channel_commands_target_their_channel_database() {
        let dir = tempfile::tempdir().unwrap();
        let stable = dir.path().join("anarlog/app.db");
        std::fs::create_dir_all(stable.parent().unwrap()).unwrap();
        std::fs::write(stable, "").unwrap();

        assert_eq!(
            resolve_default_path_for_command(dir.path(), Some(OsStr::new("anarlog-dev"))),
            dir.path().join("com.blackmushi.dev/app.db")
        );
        assert_eq!(
            resolve_default_path_for_command(dir.path(), Some(OsStr::new("anarlog-staging"))),
            dir.path().join("com.blackmushi.staging/app.db")
        );
        assert_eq!(
            resolve_default_path_for_command(dir.path(), Some(OsStr::new("anarlog-dev.exe"))),
            dir.path().join("com.blackmushi.dev/app.db")
        );
        assert_eq!(
            resolve_default_path_for_command(dir.path(), Some(OsStr::new("anarlog-staging.exe"))),
            dir.path().join("com.blackmushi.staging/app.db")
        );
    }

    #[test]
    fn a_channel_still_finds_the_database_left_under_its_previous_identifier() {
        let dir = tempfile::tempdir().unwrap();
        let previous = dir.path().join("com.hyprnote.dev/app.db");
        std::fs::create_dir_all(previous.parent().unwrap()).unwrap();
        std::fs::write(&previous, "").unwrap();

        assert_eq!(
            resolve_default_path_for_command(dir.path(), Some(OsStr::new("anarlog-dev"))),
            previous
        );
    }

    #[test]
    fn a_channel_prefers_its_current_identifier_once_that_database_exists() {
        let dir = tempfile::tempdir().unwrap();
        for identifier in ["com.hyprnote.dev", "com.blackmushi.dev"] {
            let path = dir.path().join(identifier).join("app.db");
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "").unwrap();
        }

        assert_eq!(
            resolve_default_path_for_command(dir.path(), Some(OsStr::new("anarlog-dev"))),
            dir.path().join("com.blackmushi.dev/app.db")
        );
    }
}
