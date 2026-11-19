//! Where the desktop app keeps the sign-in session: a supabase-js storage map in
//! `<local data dir>/auth.json` on macOS, `auth.dpapi` beside it on Windows
//! (`anlg_storage::windows_auth`), and the secure store on Linux, with the bundled CLI's
//! `auth.cli.json` as the fallback the CLI writes when Secret Service is unavailable.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::Locations;
use crate::paths::STORE_FILENAME;

pub const FILENAME: &str = "auth.json";
pub const CLI_FALLBACK_FILENAME: &str = "auth.cli.json";
/// The secure-store scope and key of the Linux session.
pub const LINUX_SECRET_SCOPE: &str = "auth";
pub const LINUX_SECRET_KEY: &str = "supabase-storage";
// Older builds kept the session map, JSON-encoded, under this key of `store.json`.
const LEGACY_STORE_KEY: &str = "auth";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthPaths {
    pub current: PathBuf,
    /// Older builds kept `auth.json` in the base instead.
    pub legacy: PathBuf,
    pub legacy_store_json: PathBuf,
}

impl AuthPaths {
    pub fn new(locations: &Locations) -> Self {
        Self {
            current: locations.local_data_dir.join(FILENAME),
            legacy: locations.base.join(FILENAME),
            legacy_store_json: locations.base.join(STORE_FILENAME),
        }
    }

    /// Moves a legacy session to `current`, then returns the session file to load. A failed
    /// move keeps the legacy file in use.
    pub fn resolve(&self) -> PathBuf {
        if let Err(error) = migrate_auth_state(&self.legacy, &self.legacy_store_json, &self.current)
        {
            tracing::warn!(
                legacy_auth_path = %self.legacy.display(),
                legacy_store_json_path = %self.legacy_store_json.display(),
                new_auth_path = %self.current.display(),
                error = %error,
                "failed to migrate auth state"
            );
        }

        if self.current.is_file() {
            return self.current.clone();
        }

        if self.legacy.is_file() {
            return self.legacy.clone();
        }

        self.current.clone()
    }
}

pub fn cli_fallback_path(auth_path: &Path) -> PathBuf {
    auth_path.with_file_name(CLI_FALLBACK_FILENAME)
}

/// Truncates a plaintext session before unlinking it, so a hard link can't keep the tokens.
pub fn remove_plaintext(path: &Path) -> std::io::Result<()> {
    if !path.is_file() {
        return Ok(());
    }

    // Removing the file is what retires the session, so a file that can't be truncated, such as
    // a read-only one, is still removed.
    if let Ok(file) = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
    {
        let _ = file.sync_all();
    }
    std::fs::remove_file(path)
}

/// Drops the legacy session from `store.json` once a secure store owns it.
pub fn remove_auth_from_store_json(store_json_path: &Path) -> std::io::Result<()> {
    if !store_json_path.is_file() {
        return Ok(());
    }

    let content = std::fs::read_to_string(store_json_path)?;
    // An unreadable store is left alone, as store2 leaves it: it also holds onboarding, pins and
    // preferences, which a repaired file gets back. No reader takes a session from it meanwhile.
    let mut store: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&content).map_err(invalid_data)?;

    if store.remove(LEGACY_STORE_KEY).is_none() {
        return Ok(());
    }

    anlg_storage::fs::atomic_write(
        store_json_path,
        &serde_json::to_string(&store).map_err(invalid_data)?,
    )
}

fn migrate_auth_state(
    legacy_auth_path: &Path,
    legacy_store_json_path: &Path,
    new_auth_path: &Path,
) -> std::io::Result<()> {
    if let Some(parent) = new_auth_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if legacy_auth_path.is_file() {
        std::fs::rename(legacy_auth_path, new_auth_path)?;
        return Ok(());
    }

    if new_auth_path.is_file() {
        return Ok(());
    }

    migrate_from_store_json(legacy_store_json_path, new_auth_path)
}

fn migrate_from_store_json(store_json_path: &Path, auth_path: &Path) -> std::io::Result<()> {
    if !store_json_path.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(store_json_path)?;
    let mut store: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&content).map_err(invalid_data)?;

    let auth_str = match store
        .remove(LEGACY_STORE_KEY)
        .and_then(|v| v.as_str().map(|s| s.to_owned()))
    {
        Some(s) => s,
        None => return Ok(()),
    };

    let _: HashMap<String, String> = serde_json::from_str(&auth_str).map_err(invalid_data)?;

    anlg_storage::fs::atomic_write(auth_path, &auth_str)?;
    anlg_storage::fs::atomic_write(
        store_json_path,
        &serde_json::to_string(&store).map_err(invalid_data)?,
    )?;

    Ok(())
}

fn invalid_data(e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
}

#[cfg(test)]
mod test {
    use super::*;
    use tempfile::tempdir;

    fn resolve_auth_path_from_paths(
        legacy: &Path,
        legacy_store_json: &Path,
        current: &Path,
    ) -> PathBuf {
        AuthPaths {
            current: current.to_path_buf(),
            legacy: legacy.to_path_buf(),
            legacy_store_json: legacy_store_json.to_path_buf(),
        }
        .resolve()
    }

    #[test]
    fn channel_auth_paths_golden() {
        let data = tempdir().unwrap();
        let local = tempdir().unwrap();
        for (channel, current, legacy) in [
            (
                crate::Channel::Dev,
                "com.hyprnote.dev/auth.json",
                "com.hyprnote.dev/auth.json",
            ),
            (
                crate::Channel::Staging,
                "com.hyprnote.staging/auth.json",
                "com.hyprnote.staging/auth.json",
            ),
            (
                crate::Channel::Nightly,
                "com.hyprnote.nightly/auth.json",
                "com.hyprnote.nightly/auth.json",
            ),
            (
                crate::Channel::Stable,
                "com.hyprnote.stable/auth.json",
                "anarlog/auth.json",
            ),
        ] {
            let locations = Locations::resolve_in(
                data.path(),
                local.path(),
                channel.identifier(),
                channel == crate::Channel::Dev,
            );
            let paths = AuthPaths::new(&locations);
            assert_eq!(paths.current, local.path().join(current), "{channel:?}");
            assert_eq!(paths.legacy, data.path().join(legacy), "{channel:?}");
            assert_eq!(
                paths.legacy_store_json,
                paths.legacy.with_file_name("store.json")
            );
            assert_eq!(
                anlg_storage::windows_auth::secure_path(&paths.current),
                paths.current.with_file_name("auth.dpapi")
            );
            assert_eq!(
                cli_fallback_path(&paths.current),
                paths.current.with_file_name("auth.cli.json")
            );
        }
    }

    #[test]
    fn migration_moves_legacy_auth_file() {
        let temp = tempdir().unwrap();
        let legacy_auth_path = temp.path().join("hyprnote").join(FILENAME);
        let legacy_store_json_path = temp.path().join("hyprnote").join("store.json");
        let new_auth_path = temp
            .path()
            .join("nested")
            .join("com.hyprnote.stable")
            .join(FILENAME);

        std::fs::create_dir_all(legacy_auth_path.parent().unwrap()).unwrap();
        std::fs::write(&legacy_auth_path, auth_json("legacy-token")).unwrap();

        migrate_auth_state(&legacy_auth_path, &legacy_store_json_path, &new_auth_path).unwrap();

        assert!(!legacy_auth_path.exists());
        assert_eq!(
            std::fs::read_to_string(&new_auth_path).unwrap(),
            auth_json("legacy-token")
        );
    }

    #[test]
    fn migration_overwrites_new_auth_path_when_legacy_exists() {
        let temp = tempdir().unwrap();
        let legacy_auth_path = temp.path().join("hyprnote").join(FILENAME);
        let legacy_store_json_path = temp.path().join("hyprnote").join("store.json");
        let new_auth_path = temp.path().join("com.hyprnote.stable").join(FILENAME);

        std::fs::create_dir_all(legacy_auth_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(new_auth_path.parent().unwrap()).unwrap();
        std::fs::write(&legacy_auth_path, auth_json("legacy-token")).unwrap();
        std::fs::write(&new_auth_path, "{}").unwrap();

        migrate_auth_state(&legacy_auth_path, &legacy_store_json_path, &new_auth_path).unwrap();

        assert!(!legacy_auth_path.exists());
        assert_eq!(
            std::fs::read_to_string(&new_auth_path).unwrap(),
            auth_json("legacy-token")
        );
    }

    #[test]
    fn migration_is_noop_when_new_auth_path_exists_without_legacy_auth() {
        let temp = tempdir().unwrap();
        let legacy_auth_path = temp.path().join("hyprnote").join(FILENAME);
        let legacy_store_json_path = temp.path().join("hyprnote").join("store.json");
        let new_auth_path = temp.path().join("com.hyprnote.stable").join(FILENAME);

        std::fs::create_dir_all(new_auth_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(legacy_store_json_path.parent().unwrap()).unwrap();
        std::fs::write(&new_auth_path, auth_json("new-token")).unwrap();
        std::fs::write(&legacy_store_json_path, "{ invalid json").unwrap();

        migrate_auth_state(&legacy_auth_path, &legacy_store_json_path, &new_auth_path).unwrap();

        assert_eq!(
            std::fs::read_to_string(&new_auth_path).unwrap(),
            auth_json("new-token")
        );
    }

    #[test]
    fn migration_moves_auth_out_of_legacy_store_json() {
        let temp = tempdir().unwrap();
        let legacy_auth_path = temp.path().join("hyprnote").join(FILENAME);
        let legacy_store_json_path = temp.path().join("hyprnote").join("store.json");
        let new_auth_path = temp.path().join("com.hyprnote.stable").join(FILENAME);

        std::fs::create_dir_all(legacy_store_json_path.parent().unwrap()).unwrap();
        std::fs::write(
            &legacy_store_json_path,
            legacy_store_json(
                &auth_json("legacy-token"),
                Some(("other", serde_json::json!("value"))),
            ),
        )
        .unwrap();

        migrate_auth_state(&legacy_auth_path, &legacy_store_json_path, &new_auth_path).unwrap();

        assert_eq!(
            std::fs::read_to_string(&new_auth_path).unwrap(),
            auth_json("legacy-token")
        );
        let migrated_store: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&legacy_store_json_path).unwrap())
                .unwrap();
        assert!(migrated_store.get(LEGACY_STORE_KEY).is_none());
        assert_eq!(
            migrated_store.get("other").unwrap(),
            &serde_json::json!("value")
        );
    }

    #[test]
    fn migration_does_not_clone_shared_auth_into_second_bundle() {
        let temp = tempdir().unwrap();
        let legacy_auth_path = temp.path().join("hyprnote").join(FILENAME);
        let legacy_store_json_path = temp.path().join("hyprnote").join("store.json");
        let stable_auth_path = temp.path().join("com.hyprnote.stable").join(FILENAME);
        let nightly_auth_path = temp.path().join("com.hyprnote.nightly").join(FILENAME);

        std::fs::create_dir_all(legacy_auth_path.parent().unwrap()).unwrap();
        std::fs::write(&legacy_auth_path, auth_json("legacy-token")).unwrap();

        migrate_auth_state(
            &legacy_auth_path,
            &legacy_store_json_path,
            &stable_auth_path,
        )
        .unwrap();
        migrate_auth_state(
            &legacy_auth_path,
            &legacy_store_json_path,
            &nightly_auth_path,
        )
        .unwrap();

        assert!(stable_auth_path.exists());
        assert!(!nightly_auth_path.exists());
    }

    #[test]
    fn resolve_auth_path_ignores_invalid_legacy_store_json() {
        let temp = tempdir().unwrap();
        let legacy_auth_path = temp.path().join("hyprnote").join(FILENAME);
        let legacy_store_json_path = temp.path().join("hyprnote").join("store.json");
        let new_auth_path = temp.path().join("com.hyprnote.stable").join(FILENAME);

        std::fs::create_dir_all(legacy_store_json_path.parent().unwrap()).unwrap();
        std::fs::write(&legacy_store_json_path, "{ invalid json").unwrap();

        let resolved = resolve_auth_path_from_paths(
            &legacy_auth_path,
            &legacy_store_json_path,
            &new_auth_path,
        );

        assert_eq!(resolved, new_auth_path);
        assert!(!legacy_auth_path.exists());
        assert!(!new_auth_path.exists());
    }

    #[test]
    fn resolve_auth_path_falls_back_to_legacy_auth_when_rename_fails() {
        let temp = tempdir().unwrap();
        let legacy_auth_path = temp.path().join("hyprnote").join(FILENAME);
        let legacy_store_json_path = temp.path().join("hyprnote").join("store.json");
        let new_auth_path = temp.path().join("com.hyprnote.stable").join(FILENAME);

        std::fs::create_dir_all(legacy_auth_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(&new_auth_path).unwrap();
        std::fs::write(&legacy_auth_path, auth_json("legacy-token")).unwrap();

        let resolved = resolve_auth_path_from_paths(
            &legacy_auth_path,
            &legacy_store_json_path,
            &new_auth_path,
        );

        assert_eq!(resolved, legacy_auth_path);
        assert_eq!(
            std::fs::read_to_string(&legacy_auth_path).unwrap(),
            auth_json("legacy-token")
        );
        assert!(new_auth_path.is_dir());
    }

    #[test]
    fn plaintext_removal_truncates_contents_before_unlinking() {
        let temp = tempdir().unwrap();
        let auth_path = temp.path().join(FILENAME);
        let surviving_link = temp.path().join("auth-backup.json");
        std::fs::write(&auth_path, auth_json("secret-token")).unwrap();
        std::fs::hard_link(&auth_path, &surviving_link).unwrap();

        remove_plaintext(&auth_path).unwrap();

        assert!(!auth_path.exists());
        assert_eq!(std::fs::read(&surviving_link).unwrap(), b"");
    }

    #[test]
    fn cli_fallback_is_distinct_from_legacy_plaintext_auth() {
        let auth_path = Path::new("/data/com.hyprnote.stable/auth.json");

        assert_eq!(
            cli_fallback_path(auth_path),
            Path::new("/data/com.hyprnote.stable/auth.cli.json")
        );
    }

    #[test]
    fn plaintext_cleanup_removes_auth_from_legacy_store() {
        let temp = tempdir().unwrap();
        let store_path = temp.path().join("store.json");
        std::fs::write(
            &store_path,
            legacy_store_json(
                &auth_json("secret-token"),
                Some(("other", serde_json::json!("value"))),
            ),
        )
        .unwrap();

        remove_auth_from_store_json(&store_path).unwrap();

        let store: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(store_path).unwrap()).unwrap();
        assert!(store.get(LEGACY_STORE_KEY).is_none());
        assert_eq!(store.get("other"), Some(&serde_json::json!("value")));
    }

    #[test]
    fn plaintext_cleanup_leaves_an_unreadable_store_and_its_settings_alone() {
        let temp = tempdir().unwrap();
        let store_path = temp.path().join("store.json");
        let truncated = r#"{"desktop":"{\"OnboardingNeeded2\":false}""#;
        std::fs::write(&store_path, truncated).unwrap();

        assert!(remove_auth_from_store_json(&store_path).is_err());

        assert_eq!(std::fs::read_to_string(store_path).unwrap(), truncated);
    }

    fn auth_json(token: &str) -> String {
        serde_json::to_string(&serde_json::json!({
            "sb-project-auth-token": token,
        }))
        .unwrap()
    }

    fn legacy_store_json(auth_json: &str, extra: Option<(&str, serde_json::Value)>) -> String {
        let mut store = serde_json::Map::new();
        store.insert(
            LEGACY_STORE_KEY.to_string(),
            serde_json::Value::String(auth_json.to_string()),
        );
        if let Some((key, value)) = extra {
            store.insert(key.to_string(), value);
        }
        serde_json::to_string(&store).unwrap()
    }
}
