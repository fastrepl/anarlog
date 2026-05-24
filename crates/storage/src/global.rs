use std::path::{Path, PathBuf};

pub const VAULT_CONFIG_FILENAME: &str = "global.json";
const STAGING_BUNDLE_ID: &str = "com.hyprnote.staging";
const RELEASE_APP_FOLDER: &str = "anarlog";
const LEGACY_RELEASE_APP_FOLDER: &str = "hyprnote";

pub fn compute_vault_config_path(base: &Path) -> PathBuf {
    base.join(VAULT_CONFIG_FILENAME)
}

pub fn compute_default_base(bundle_id: &str) -> Option<PathBuf> {
    let data_dir = dirs::data_dir()?;
    let app_folder = resolve_app_folder(&data_dir, bundle_id, cfg!(debug_assertions));
    Some(data_dir.join(app_folder))
}

fn resolve_app_folder<'a>(data_dir: &Path, bundle_id: &'a str, is_debug: bool) -> &'a str {
    if is_debug || bundle_id == STAGING_BUNDLE_ID {
        bundle_id
    } else if has_app_data(&data_dir.join(LEGACY_RELEASE_APP_FOLDER))
        && !has_app_data(&data_dir.join(RELEASE_APP_FOLDER))
    {
        LEGACY_RELEASE_APP_FOLDER
    } else {
        RELEASE_APP_FOLDER
    }
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
