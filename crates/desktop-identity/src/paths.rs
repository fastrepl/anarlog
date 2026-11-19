use std::path::{Path, PathBuf};

use anlg_storage::global::compute_default_base_in;

use crate::channel::{NIGHTLY_IDENTIFIER, STABLE_IDENTIFIER};

pub const DB_FILENAME: &str = "app.db";
pub const DB_RESET_MARKER: &str = "app.db.reset-requested";
pub const STORE_FILENAME: &str = "store.json";
/// Held in the database directory from process start until startup finishes.
pub const LAUNCH_LOCK_FILENAME: &str = "launch.lock";

/// Held in the database directory for the process lifetime by channels that share it.
pub fn channel_lock_filename(identifier: &str) -> String {
    format!("{identifier}.running.lock")
}

/// Every place one channel of the desktop app keeps its data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Locations {
    pub identifier: String,
    /// Settings, `store.json`, notes and recordings (the vault), and models.
    pub base: PathBuf,
    /// `app.db`, its reset marker and the launch locks.
    pub database_dir: PathBuf,
    /// Tauri's `app_local_data_dir`: the sign-in session.
    pub local_data_dir: PathBuf,
}

impl Locations {
    pub fn resolve(identifier: &str) -> Option<Self> {
        Some(Self::resolve_in(
            &dirs::data_dir()?,
            &dirs::data_local_dir()?,
            identifier,
            cfg!(debug_assertions),
        ))
    }

    pub fn resolve_in(
        data_dir: &Path,
        data_local_dir: &Path,
        identifier: &str,
        is_debug: bool,
    ) -> Self {
        Self {
            identifier: identifier.to_string(),
            base: compute_default_base_in(data_dir, identifier, is_debug),
            database_dir: desktop_db_dir_in(data_dir, identifier, is_debug),
            local_data_dir: data_local_dir.join(identifier),
        }
    }

    pub fn database(&self) -> PathBuf {
        self.database_dir.join(DB_FILENAME)
    }
}

// Nightly previews run against the user's real notes, so it opens stable's
// database while keeping its own settings, store, and sign-in. db-migrate keeps
// the two builds compatible: stable tolerates the additive migrations a newer
// Nightly applies and refuses the file after a "-- breaking" one until a stable
// release includes that migration.
pub fn shared_database_peer(identifier: &str) -> Option<&'static str> {
    match identifier {
        NIGHTLY_IDENTIFIER => Some(STABLE_IDENTIFIER),
        STABLE_IDENTIFIER => Some(NIGHTLY_IDENTIFIER),
        _ => None,
    }
}

pub fn database_identifier(identifier: &str) -> &str {
    if identifier == NIGHTLY_IDENTIFIER {
        STABLE_IDENTIFIER
    } else {
        identifier
    }
}

pub fn desktop_db_dir(identifier: &str) -> Option<PathBuf> {
    Some(desktop_db_dir_in(
        &dirs::data_dir()?,
        identifier,
        cfg!(debug_assertions),
    ))
}

fn desktop_db_dir_in(data_dir: &Path, identifier: &str, is_debug: bool) -> PathBuf {
    let identifier = database_identifier(identifier);
    let default_dir = compute_default_base_in(data_dir, identifier, is_debug);
    let identifier_dir = data_dir.join(identifier);

    if identifier_dir.join(DB_FILENAME).is_file() && !default_dir.join(DB_FILENAME).is_file() {
        identifier_dir
    } else {
        default_dir
    }
}

/// Moves `app.db` and its WAL files aside once when the reset marker is present.
pub fn apply_pending_database_reset(dir: &Path) -> Result<Option<PathBuf>, String> {
    let marker = dir.join(DB_RESET_MARKER);
    if !marker.is_file() {
        return Ok(None);
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let backup = dir.join(format!("{DB_FILENAME}.{stamp}.bak"));
    for suffix in ["", "-wal", "-shm"] {
        let source = dir.join(format!("{DB_FILENAME}{suffix}"));
        if !source.exists() {
            continue;
        }
        let target = dir.join(format!("{DB_FILENAME}.{stamp}.bak{suffix}"));
        std::fs::rename(&source, &target)
            .map_err(|error| format!("failed to move {} aside: {error}", source.display()))?;
    }
    std::fs::remove_file(&marker)
        .map_err(|error| format!("failed to clear the database reset marker: {error}"))?;
    Ok(Some(backup))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Channel;

    #[test]
    fn a_reset_marker_moves_the_database_aside_once() {
        let dir = tempfile::tempdir().unwrap();
        for suffix in ["", "-wal", "-shm"] {
            std::fs::write(dir.path().join(format!("{DB_FILENAME}{suffix}")), b"data").unwrap();
        }
        assert_eq!(apply_pending_database_reset(dir.path()).unwrap(), None);
        std::fs::write(dir.path().join(DB_RESET_MARKER), b"").unwrap();

        let backup = apply_pending_database_reset(dir.path())
            .unwrap()
            .expect("database moved aside");

        assert!(backup.is_file());
        assert!(
            backup.with_extension("bak-wal").is_file() || {
                let name = format!("{}-wal", backup.file_name().unwrap().to_string_lossy());
                dir.path().join(name).is_file()
            }
        );
        assert!(!dir.path().join(DB_FILENAME).exists());
        assert!(!dir.path().join(DB_RESET_MARKER).exists());
        assert_eq!(apply_pending_database_reset(dir.path()).unwrap(), None);
    }

    #[test]
    fn only_nightly_and_stable_share_a_database_directory() {
        let db_dir = desktop_db_dir("com.hyprnote.dev").unwrap();

        assert!(db_dir.ends_with("com.hyprnote.dev"));

        assert_eq!(
            desktop_db_dir(NIGHTLY_IDENTIFIER),
            desktop_db_dir(STABLE_IDENTIFIER)
        );
        assert!(
            !desktop_db_dir(NIGHTLY_IDENTIFIER)
                .unwrap()
                .ends_with(NIGHTLY_IDENTIFIER)
        );

        assert_eq!(
            shared_database_peer(NIGHTLY_IDENTIFIER),
            Some(STABLE_IDENTIFIER)
        );
        assert_eq!(
            shared_database_peer(STABLE_IDENTIFIER),
            Some(NIGHTLY_IDENTIFIER)
        );
        assert_eq!(shared_database_peer("com.hyprnote.staging"), None);
        assert_eq!(shared_database_peer("com.hyprnote.dev"), None);
    }

    fn relative(root: &Path, path: &Path) -> String {
        path.strip_prefix(root).unwrap().display().to_string()
    }

    /// Each channel as its shipping build resolves it: dev in debug, the others in release.
    #[test]
    fn channel_locations_golden() {
        let data = tempfile::tempdir().unwrap();
        let local = tempfile::tempdir().unwrap();
        for (channel, base, database_dir, local_data_dir) in [
            (
                Channel::Dev,
                "com.hyprnote.dev",
                "com.hyprnote.dev",
                "com.hyprnote.dev",
            ),
            (
                Channel::Staging,
                "com.hyprnote.staging",
                "com.hyprnote.staging",
                "com.hyprnote.staging",
            ),
            (
                Channel::Nightly,
                "com.hyprnote.nightly",
                "anarlog",
                "com.hyprnote.nightly",
            ),
            (Channel::Stable, "anarlog", "anarlog", "com.hyprnote.stable"),
        ] {
            let locations = Locations::resolve_in(
                data.path(),
                local.path(),
                channel.identifier(),
                channel == Channel::Dev,
            );
            assert_eq!(locations.identifier, channel.identifier());
            assert_eq!(relative(data.path(), &locations.base), base, "{channel:?}");
            assert_eq!(
                relative(data.path(), &locations.database_dir),
                database_dir,
                "{channel:?}"
            );
            assert_eq!(
                relative(local.path(), &locations.local_data_dir),
                local_data_dir,
                "{channel:?}"
            );
            assert_eq!(locations.database(), locations.database_dir.join("app.db"));
        }
        assert_eq!(LAUNCH_LOCK_FILENAME, "launch.lock");
        assert_eq!(
            channel_lock_filename(STABLE_IDENTIFIER),
            "com.hyprnote.stable.running.lock"
        );
        assert_eq!(DB_RESET_MARKER, "app.db.reset-requested");
        assert_eq!(STORE_FILENAME, "store.json");
    }

    #[test]
    fn stable_keeps_the_legacy_hyprnote_base_and_database() {
        let data = tempfile::tempdir().unwrap();
        let local = tempfile::tempdir().unwrap();
        std::fs::create_dir(data.path().join("hyprnote")).unwrap();
        std::fs::write(data.path().join("hyprnote/app.db"), "").unwrap();

        for identifier in [STABLE_IDENTIFIER, NIGHTLY_IDENTIFIER] {
            let locations = Locations::resolve_in(data.path(), local.path(), identifier, false);
            assert_eq!(relative(data.path(), &locations.database_dir), "hyprnote");
        }
        let stable = Locations::resolve_in(data.path(), local.path(), STABLE_IDENTIFIER, false);
        assert_eq!(relative(data.path(), &stable.base), "hyprnote");
    }

    #[test]
    fn a_database_left_in_the_bundle_folder_is_used_until_the_base_has_one() {
        let data = tempfile::tempdir().unwrap();
        let local = tempfile::tempdir().unwrap();
        let bundle_dir = data.path().join(STABLE_IDENTIFIER);
        std::fs::create_dir(&bundle_dir).unwrap();
        std::fs::write(bundle_dir.join(DB_FILENAME), "").unwrap();

        for identifier in [STABLE_IDENTIFIER, NIGHTLY_IDENTIFIER] {
            let locations = Locations::resolve_in(data.path(), local.path(), identifier, false);
            assert_eq!(locations.database_dir, bundle_dir, "{identifier}");
        }

        std::fs::create_dir(data.path().join("anarlog")).unwrap();
        std::fs::write(data.path().join("anarlog").join(DB_FILENAME), "").unwrap();
        let stable = Locations::resolve_in(data.path(), local.path(), STABLE_IDENTIFIER, false);
        assert_eq!(relative(data.path(), &stable.database_dir), "anarlog");
    }
}
