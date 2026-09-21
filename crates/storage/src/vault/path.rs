use std::path::{Path, PathBuf};

use crate::global::compute_vault_config_path;

pub const VAULT_PATH_KEY: &str = "vault_path";
pub const SETTINGS_FILENAME: &str = "settings.json";

pub fn compute_settings_path(base: &Path) -> PathBuf {
    base.join(SETTINGS_FILENAME)
}
const VAULT_BASE_ENV_VAR: &str = "CHAR_VAULT_BASE";

fn expand_path(path: &str, default_base: Option<&Path>) -> PathBuf {
    let home_dir = || dirs::home_dir().map(|p| p.to_string_lossy().into_owned());
    let context = |var: &str| -> Option<String> {
        if var == "DEFAULT" {
            return default_base.map(|p| p.to_string_lossy().into_owned());
        }
        std::env::var(var).ok()
    };
    let expanded = shellexpand::full_with_context_no_errors(path, home_dir, context);
    PathBuf::from(expanded.into_owned())
}

pub fn validate_vault_path(path: &Path) -> Result<(), crate::Error> {
    if !path.is_absolute() {
        return Err(crate::Error::PathNotAbsolute);
    }

    if path.to_str().is_none() {
        return Err(crate::Error::PathNotValidUtf8);
    }

    if path.exists() && !path.is_dir() {
        return Err(crate::Error::PathIsNotDirectory);
    }

    Ok(())
}

pub fn ensure_vault_dir(path: &Path) -> Result<(), crate::Error> {
    validate_vault_path(path)?;

    if !path.exists() {
        std::fs::create_dir_all(path)?;
    }

    Ok(())
}

pub fn resolve_base(global_base: &Path, default_base: &Path) -> PathBuf {
    resolve_custom(global_base, default_base).unwrap_or_else(|| default_base.to_path_buf())
}

pub fn resolve_custom(global_base: &Path, default_base: &Path) -> Option<PathBuf> {
    if let Ok(path) = std::env::var(VAULT_BASE_ENV_VAR) {
        let path = expand_path(&path, Some(default_base));
        if ensure_vault_dir(&path).is_ok() {
            return Some(path);
        }
    }

    if let Some(custom_base) = load_vault_path(global_base) {
        let custom_path = expand_path(&custom_base, Some(default_base));
        if ensure_vault_dir(&custom_path).is_ok() {
            return Some(custom_path);
        }
    }

    None
}

/// Moves the notes and recordings of a custom storage location into
/// `default_base` and clears the override that pointed at it. Returns the
/// folder that was consolidated, if there was one.
///
/// The copy finishes before the override is cleared, so an interrupted or
/// failed launch keeps using the custom folder and retries next time. Only
/// artifacts the app owns are removed from the old folder afterwards.
pub fn consolidate_custom_vault(
    global_base: &Path,
    default_base: &Path,
) -> Result<Option<PathBuf>, crate::Error> {
    let Some(mut config) = load_config(global_base) else {
        return Ok(None);
    };
    let Some(custom_path) = config
        .get(VAULT_PATH_KEY)
        .and_then(|v| v.as_str())
        .map(|path| expand_path(path, Some(default_base)))
    else {
        return Ok(None);
    };

    let has_separate_data = custom_path.is_dir() && !is_same_dir(&custom_path, default_base);
    if has_separate_data {
        std::fs::create_dir_all(default_base)?;
        super::fs::copy_vault_items(&custom_path, default_base)?;
        super::fs::remove_derived_items(default_base)?;
    }

    if let Some(obj) = config.as_object_mut() {
        obj.remove(VAULT_PATH_KEY);
    }
    crate::fs::atomic_write(
        &compute_vault_config_path(global_base),
        &serde_json::to_string_pretty(&config)?,
    )?;

    if has_separate_data {
        // Best-effort: the data is already safe at the default base.
        let _ = super::fs::remove_owned_items(&custom_path);
    }

    Ok(Some(custom_path))
}

fn is_same_dir(a: &Path, b: &Path) -> bool {
    a == b
        || matches!(
            (a.canonicalize(), b.canonicalize()),
            (Ok(a), Ok(b)) if a == b
        )
}

fn load_vault_path(global_base: &Path) -> Option<String> {
    load_config(global_base)?
        .get(VAULT_PATH_KEY)
        .and_then(|v| v.as_str())
        .map(ToOwned::to_owned)
}

fn load_config(global_base: &Path) -> Option<serde_json::Value> {
    let content = std::fs::read_to_string(compute_vault_config_path(global_base)).ok()?;
    serde_json::from_str::<serde_json::Value>(&content).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;
    use tempfile::tempdir;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn with_env<F, R>(key: &str, value: Option<&str>, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let _guard = ENV_MUTEX.lock().unwrap();
        let prev = std::env::var(key).ok();

        match value {
            Some(v) => unsafe { std::env::set_var(key, v) },
            None => unsafe { std::env::remove_var(key) },
        }

        let result = f();

        match prev {
            Some(v) => unsafe { std::env::set_var(key, v) },
            None => unsafe { std::env::remove_var(key) },
        }

        result
    }

    mod validate_vault_path_tests {
        use super::*;

        #[test]
        fn accepts_valid_absolute_path() {
            let temp = tempdir().unwrap();
            let path = temp.path().join("vault");
            assert!(validate_vault_path(&path).is_ok());
        }

        #[test]
        fn rejects_relative_path() {
            let path = PathBuf::from("relative/path/vault");
            let result = validate_vault_path(&path);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("absolute"));
        }

        #[test]
        fn rejects_dot_relative_path() {
            let path = PathBuf::from("./vault");
            let result = validate_vault_path(&path);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("absolute"));
        }

        #[test]
        fn accepts_existing_directory() {
            let temp = tempdir().unwrap();
            let path = temp.path().join("vault");
            fs::create_dir_all(&path).unwrap();
            assert!(validate_vault_path(&path).is_ok());
        }

        #[test]
        fn rejects_existing_file() {
            let temp = tempdir().unwrap();
            let path = temp.path().join("not_a_dir");
            fs::write(&path, "content").unwrap();
            let result = validate_vault_path(&path);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("not a directory"));
        }
    }

    mod ensure_vault_dir_tests {
        use super::*;

        #[test]
        fn creates_directory_if_not_exists() {
            let temp = tempdir().unwrap();
            let path = temp.path().join("new_vault");
            assert!(!path.exists());
            assert!(ensure_vault_dir(&path).is_ok());
            assert!(path.exists());
            assert!(path.is_dir());
        }

        #[test]
        fn succeeds_for_existing_directory() {
            let temp = tempdir().unwrap();
            let path = temp.path().join("existing");
            fs::create_dir_all(&path).unwrap();
            assert!(ensure_vault_dir(&path).is_ok());
        }

        #[test]
        fn rejects_existing_file() {
            let temp = tempdir().unwrap();
            let path = temp.path().join("file");
            fs::write(&path, "content").unwrap();
            let result = ensure_vault_dir(&path);
            assert!(result.is_err());
        }

        #[test]
        fn creates_nested_directories() {
            let temp = tempdir().unwrap();
            let path = temp.path().join("a").join("b").join("c");
            assert!(ensure_vault_dir(&path).is_ok());
            assert!(path.is_dir());
        }
    }

    mod resolve_custom_tests {
        use super::*;

        #[test]
        fn returns_none_when_no_sources() {
            let temp = tempdir().unwrap();
            let global_base = temp.path().to_path_buf();
            let default_base = temp.path().join("default");

            with_env(VAULT_BASE_ENV_VAR, None, || {
                assert!(resolve_custom(&global_base, &default_base).is_none());
            });
        }

        #[test]
        fn returns_env_var_path_when_exists() {
            let temp = tempdir().unwrap();
            let global_base = temp.path().to_path_buf();
            let default_base = temp.path().join("default");
            let env_path = temp.path().join("env_content");
            fs::create_dir_all(&env_path).unwrap();

            with_env(VAULT_BASE_ENV_VAR, Some(env_path.to_str().unwrap()), || {
                let result = resolve_custom(&global_base, &default_base);
                assert_eq!(result, Some(env_path.clone()));
            });
        }

        #[test]
        fn creates_env_var_path_if_missing() {
            let temp = tempdir().unwrap();
            let global_base = temp.path().to_path_buf();
            let default_base = temp.path().join("default");
            let env_path = temp.path().join("new_env_vault");

            with_env(VAULT_BASE_ENV_VAR, Some(env_path.to_str().unwrap()), || {
                let result = resolve_custom(&global_base, &default_base);
                assert_eq!(result, Some(env_path.clone()));
                assert!(env_path.exists());
            });
        }

        #[test]
        fn reads_from_vault_config() {
            let temp = tempdir().unwrap();
            let global_base = temp.path().to_path_buf();
            let default_base = temp.path().join("default");
            let custom_path = temp.path().join("custom_vault");
            fs::create_dir_all(&custom_path).unwrap();

            let config = serde_json::json!({ VAULT_PATH_KEY: custom_path.to_string_lossy() });
            fs::write(compute_vault_config_path(&global_base), config.to_string()).unwrap();

            with_env(VAULT_BASE_ENV_VAR, None, || {
                let result = resolve_custom(&global_base, &default_base);
                assert_eq!(result, Some(custom_path.clone()));
            });
        }

        #[test]
        fn env_var_takes_precedence() {
            let temp = tempdir().unwrap();
            let global_base = temp.path().to_path_buf();
            let default_base = temp.path().join("default");
            let env_path = temp.path().join("env_content");
            let file_path = temp.path().join("file_vault");
            fs::create_dir_all(&env_path).unwrap();
            fs::create_dir_all(&file_path).unwrap();

            let config = serde_json::json!({ VAULT_PATH_KEY: file_path.to_string_lossy() });
            fs::write(compute_vault_config_path(&global_base), config.to_string()).unwrap();

            with_env(VAULT_BASE_ENV_VAR, Some(env_path.to_str().unwrap()), || {
                let result = resolve_custom(&global_base, &default_base);
                assert_eq!(result, Some(env_path.clone()));
            });
        }

        #[test]
        fn creates_vault_path_if_missing() {
            let temp = tempdir().unwrap();
            let global_base = temp.path().to_path_buf();
            let default_base = temp.path().join("default");
            let custom_path = temp.path().join("custom_vault");

            let config = serde_json::json!({ VAULT_PATH_KEY: custom_path.to_string_lossy() });
            fs::write(compute_vault_config_path(&global_base), config.to_string()).unwrap();

            with_env(VAULT_BASE_ENV_VAR, None, || {
                let result = resolve_custom(&global_base, &default_base);
                assert_eq!(result, Some(custom_path.clone()));
                assert!(custom_path.exists());
            });
        }
    }

    mod resolve_base_tests {
        use super::*;

        #[test]
        fn falls_back_to_default_base() {
            let temp = tempdir().unwrap();
            let global_base = temp.path().to_path_buf();
            let default_base = temp.path().join("default");

            with_env(VAULT_BASE_ENV_VAR, None, || {
                let result = resolve_base(&global_base, &default_base);
                assert_eq!(result, default_base);
            });
        }
    }

    mod consolidate_custom_vault_tests {
        use super::*;

        fn write_config(global_base: &Path, config: serde_json::Value) {
            fs::write(compute_vault_config_path(global_base), config.to_string()).unwrap();
        }

        fn read_config(global_base: &Path) -> serde_json::Value {
            let content = fs::read_to_string(compute_vault_config_path(global_base)).unwrap();
            serde_json::from_str(&content).unwrap()
        }

        fn seed_vault(vault: &Path) {
            fs::create_dir_all(vault.join("sessions").join("s1")).unwrap();
            fs::write(
                vault.join("sessions").join("s1").join("audio.wav"),
                "custom",
            )
            .unwrap();
            fs::create_dir_all(vault.join("search_index")).unwrap();
            fs::write(vault.join("search_index").join("meta.json"), "{}").unwrap();
            fs::write(vault.join("settings.json"), r#"{"theme":"dark"}"#).unwrap();
            fs::write(vault.join("AGENTS.md"), "# Anarlog Desktop\n").unwrap();
            fs::create_dir_all(vault.join(".obsidian")).unwrap();
            fs::write(vault.join("Daily note.md"), "note").unwrap();
        }

        #[test]
        fn does_nothing_without_an_override() {
            let temp = tempdir().unwrap();
            let global_base = temp.path().to_path_buf();
            let default_base = temp.path().join("default");
            fs::create_dir_all(&default_base).unwrap();
            write_config(&global_base, serde_json::json!({"theme": "dark"}));

            assert_eq!(
                consolidate_custom_vault(&global_base, &default_base).unwrap(),
                None
            );
            assert_eq!(
                read_config(&global_base),
                serde_json::json!({"theme": "dark"})
            );
        }

        #[test]
        fn does_nothing_without_a_config_file() {
            let temp = tempdir().unwrap();
            let global_base = temp.path().to_path_buf();
            let default_base = temp.path().join("default");

            assert_eq!(
                consolidate_custom_vault(&global_base, &default_base).unwrap(),
                None
            );
            assert!(!compute_vault_config_path(&global_base).exists());
        }

        #[test]
        fn moves_owned_items_into_the_default_base_and_clears_the_override() {
            let temp = tempdir().unwrap();
            let global_base = temp.path().join("global");
            let default_base = global_base.clone();
            let vault = temp.path().join("obsidian-vault");
            fs::create_dir_all(&default_base).unwrap();
            seed_vault(&vault);
            fs::create_dir_all(default_base.join("sessions").join("s0")).unwrap();
            fs::write(
                default_base.join("sessions").join("s0").join("audio.wav"),
                "older",
            )
            .unwrap();
            fs::create_dir_all(default_base.join("search_index")).unwrap();
            fs::write(default_base.join("search_index").join("meta.json"), "stale").unwrap();
            write_config(
                &global_base,
                serde_json::json!({
                    "theme": "dark",
                    VAULT_PATH_KEY: vault.to_string_lossy(),
                }),
            );

            let result = consolidate_custom_vault(&global_base, &default_base).unwrap();

            assert_eq!(result, Some(vault.clone()));
            assert_eq!(
                fs::read_to_string(default_base.join("sessions").join("s1").join("audio.wav"))
                    .unwrap(),
                "custom"
            );
            assert!(
                default_base
                    .join("sessions")
                    .join("s0")
                    .join("audio.wav")
                    .exists()
            );
            assert_eq!(
                fs::read_to_string(default_base.join("settings.json")).unwrap(),
                r#"{"theme":"dark"}"#
            );
            assert!(
                !default_base.join("search_index").exists(),
                "a stale index must not survive; the app rebuilds it from the database"
            );
            assert_eq!(
                read_config(&global_base),
                serde_json::json!({"theme": "dark"})
            );

            assert!(!vault.join("sessions").exists());
            assert!(!vault.join("search_index").exists());
            assert!(!vault.join("settings.json").exists());
            assert!(!vault.join("AGENTS.md").exists());
            assert!(vault.join(".obsidian").exists());
            assert!(vault.join("Daily note.md").exists());
        }

        #[test]
        fn resolves_to_the_default_base_after_consolidating() {
            let temp = tempdir().unwrap();
            let global_base = temp.path().join("global");
            let vault = temp.path().join("vault");
            fs::create_dir_all(&global_base).unwrap();
            seed_vault(&vault);
            write_config(
                &global_base,
                serde_json::json!({ VAULT_PATH_KEY: vault.to_string_lossy() }),
            );

            with_env(VAULT_BASE_ENV_VAR, None, || {
                assert_eq!(resolve_base(&global_base, &global_base), vault);
                consolidate_custom_vault(&global_base, &global_base).unwrap();
                assert_eq!(resolve_base(&global_base, &global_base), global_base);
            });
        }

        #[test]
        fn clears_an_override_that_points_at_the_default_base() {
            let temp = tempdir().unwrap();
            let global_base = temp.path().join("global");
            fs::create_dir_all(&global_base).unwrap();
            seed_vault(&global_base);
            write_config(
                &global_base,
                serde_json::json!({ VAULT_PATH_KEY: global_base.to_string_lossy() }),
            );

            let result = consolidate_custom_vault(&global_base, &global_base).unwrap();

            assert_eq!(result, Some(global_base.clone()));
            assert!(read_config(&global_base).get(VAULT_PATH_KEY).is_none());
            assert!(global_base.join("sessions").join("s1").exists());
            assert!(global_base.join("search_index").exists());
        }

        #[test]
        fn clears_an_override_whose_folder_is_gone() {
            let temp = tempdir().unwrap();
            let global_base = temp.path().join("global");
            fs::create_dir_all(&global_base).unwrap();
            let missing = temp.path().join("missing");
            write_config(
                &global_base,
                serde_json::json!({ VAULT_PATH_KEY: missing.to_string_lossy() }),
            );

            let result = consolidate_custom_vault(&global_base, &global_base).unwrap();

            assert_eq!(result, Some(missing));
            assert!(read_config(&global_base).get(VAULT_PATH_KEY).is_none());
        }

        #[test]
        fn keeps_the_override_when_the_copy_fails() {
            let temp = tempdir().unwrap();
            let global_base = temp.path().join("global");
            let vault = temp.path().join("vault");
            fs::create_dir_all(&global_base).unwrap();
            seed_vault(&vault);
            // A file where the sessions directory must go makes the copy fail.
            fs::write(global_base.join("sessions"), "not a directory").unwrap();
            write_config(
                &global_base,
                serde_json::json!({ VAULT_PATH_KEY: vault.to_string_lossy() }),
            );

            assert!(consolidate_custom_vault(&global_base, &global_base).is_err());

            assert_eq!(
                read_config(&global_base)
                    .get(VAULT_PATH_KEY)
                    .and_then(|v| v.as_str()),
                Some(vault.to_string_lossy().as_ref())
            );
            assert!(vault.join("sessions").join("s1").join("audio.wav").exists());
            assert!(vault.join("settings.json").exists());
        }
    }
}
