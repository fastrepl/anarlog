use std::path::{Path, PathBuf};

pub const VAULT_CONFIG_FILENAME: &str = "global.json";
const STAGING_BUNDLE_ID: &str = "com.blackmushi.staging";
const NIGHTLY_BUNDLE_ID: &str = "com.blackmushi.nightly";
const RELEASE_APP_FOLDER: &str = "anarlog";
const LEGACY_RELEASE_APP_FOLDER: &str = "hyprnote";

// Debug, staging and nightly name their folder after the bundle id, so renaming
// the bundle would point a working install at an empty directory. Each entry
// maps a current id to the id it used to ship under; the older folder keeps
// being used until the new one has data of its own.
const PREVIOUS_BUNDLE_IDS: &[(&str, &str)] = &[
    ("com.blackmushi.dev", "com.hyprnote.dev"),
    ("com.blackmushi.staging", "com.hyprnote.staging"),
    ("com.blackmushi.nightly", "com.hyprnote.nightly"),
    ("com.blackmushi.stable", "com.hyprnote.stable"),
];

pub fn compute_vault_config_path(base: &Path) -> PathBuf {
    base.join(VAULT_CONFIG_FILENAME)
}

pub fn compute_default_base(bundle_id: &str) -> Option<PathBuf> {
    let data_dir = dirs::data_dir()?;
    let app_folder = resolve_app_folder(&data_dir, bundle_id, cfg!(debug_assertions));
    Some(data_dir.join(app_folder))
}

// This base holds settings, the store, and the vault config. Nightly keeps its
// own; the desktop app maps only the database itself onto stable's folder.
fn resolve_app_folder<'a>(data_dir: &Path, bundle_id: &'a str, is_debug: bool) -> &'a str {
    if is_debug || matches!(bundle_id, STAGING_BUNDLE_ID | NIGHTLY_BUNDLE_ID) {
        per_bundle_folder(data_dir, bundle_id)
    } else if has_app_data(&data_dir.join(LEGACY_RELEASE_APP_FOLDER))
        && !has_app_data(&data_dir.join(RELEASE_APP_FOLDER))
    {
        LEGACY_RELEASE_APP_FOLDER
    } else {
        RELEASE_APP_FOLDER
    }
}

// Prefer this id's own folder once it holds anything, so a fresh install that
// has already written data never jumps back to the folder it was renamed from.
fn per_bundle_folder<'a>(data_dir: &Path, bundle_id: &'a str) -> &'a str {
    if has_app_data(&data_dir.join(bundle_id)) {
        return bundle_id;
    }

    PREVIOUS_BUNDLE_IDS
        .iter()
        .find(|(current, _)| *current == bundle_id)
        .map(|(_, previous)| *previous)
        .filter(|previous| has_app_data(&data_dir.join(previous)))
        .unwrap_or(bundle_id)
}

fn has_app_data(path: &Path) -> bool {
    std::fs::read_dir(path)
        .map(|mut entries| entries.next().is_some())
        .unwrap_or_else(|_| path.exists())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn renamed_bundle_keeps_reading_the_folder_it_shipped_under() {
        let temp = tempdir().unwrap();
        let previous = temp.path().join("com.hyprnote.dev");
        std::fs::create_dir_all(&previous).unwrap();
        std::fs::write(previous.join("app.db"), "").unwrap();

        assert_eq!(
            resolve_app_folder(temp.path(), "com.blackmushi.dev", true),
            "com.hyprnote.dev"
        );
    }

    #[test]
    fn renamed_bundle_prefers_its_own_folder_once_it_has_data() {
        let temp = tempdir().unwrap();
        for id in ["com.hyprnote.dev", "com.blackmushi.dev"] {
            let dir = temp.path().join(id);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("app.db"), "").unwrap();
        }

        assert_eq!(
            resolve_app_folder(temp.path(), "com.blackmushi.dev", true),
            "com.blackmushi.dev"
        );
    }

    #[test]
    fn an_unmapped_bundle_id_stays_on_its_own_folder() {
        let temp = tempdir().unwrap();

        assert_eq!(
            resolve_app_folder(temp.path(), "com.example.other", true),
            "com.example.other"
        );
    }

    #[test]
    fn resolve_app_folder_uses_anarlog_for_new_stable_installs() {
        let temp = tempdir().unwrap();

        assert_eq!(
            resolve_app_folder(temp.path(), "com.hyprnote.stable", false),
            RELEASE_APP_FOLDER
        );
    }

    #[test]
    fn resolve_app_folder_keeps_legacy_stable_folder_when_it_has_data() {
        let temp = tempdir().unwrap();
        let legacy_base = temp.path().join(LEGACY_RELEASE_APP_FOLDER);
        std::fs::create_dir_all(&legacy_base).unwrap();
        std::fs::write(legacy_base.join("store.json"), "{}").unwrap();

        assert_eq!(
            resolve_app_folder(temp.path(), "com.hyprnote.stable", false),
            LEGACY_RELEASE_APP_FOLDER
        );
    }

    #[test]
    fn resolve_app_folder_prefers_anarlog_when_new_folder_has_data() {
        let temp = tempdir().unwrap();
        let legacy_base = temp.path().join(LEGACY_RELEASE_APP_FOLDER);
        let new_base = temp.path().join(RELEASE_APP_FOLDER);
        std::fs::create_dir_all(&legacy_base).unwrap();
        std::fs::create_dir_all(&new_base).unwrap();
        std::fs::write(legacy_base.join("store.json"), "{}").unwrap();
        std::fs::write(new_base.join("app.db"), "").unwrap();

        assert_eq!(
            resolve_app_folder(temp.path(), "com.hyprnote.stable", false),
            RELEASE_APP_FOLDER
        );
    }

    #[test]
    fn resolve_app_folder_ignores_empty_legacy_stable_folder() {
        let temp = tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join(LEGACY_RELEASE_APP_FOLDER)).unwrap();

        assert_eq!(
            resolve_app_folder(temp.path(), "com.hyprnote.stable", false),
            RELEASE_APP_FOLDER
        );
    }

    #[test]
    fn resolve_app_folder_uses_anarlog_for_other_release_bundle_ids() {
        let temp = tempdir().unwrap();

        assert_eq!(
            resolve_app_folder(temp.path(), "com.hyprnote.Hyprnote", false),
            RELEASE_APP_FOLDER
        );
    }

    #[test]
    fn nightly_keeps_its_own_settings_base() {
        let temp = tempfile::tempdir().unwrap();
        for folder in [RELEASE_APP_FOLDER, LEGACY_RELEASE_APP_FOLDER] {
            std::fs::create_dir(temp.path().join(folder)).unwrap();
            std::fs::write(temp.path().join(folder).join("app.db"), "stable data").unwrap();
        }
        assert_eq!(
            resolve_app_folder(temp.path(), NIGHTLY_BUNDLE_ID, false),
            NIGHTLY_BUNDLE_ID
        );
    }

    #[test]
    fn resolve_app_folder_returns_bundle_id_for_staging() {
        assert_eq!(
            resolve_app_folder(Path::new("/tmp"), STAGING_BUNDLE_ID, false),
            STAGING_BUNDLE_ID
        );
    }

    #[test]
    fn resolve_app_folder_returns_bundle_id_in_debug_builds() {
        assert_eq!(
            resolve_app_folder(Path::new("/tmp"), "com.hyprnote.stable", true),
            "com.hyprnote.stable"
        );
    }
}
