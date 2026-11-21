//! Where the shipping app keeps a channel's notes, recordings and attachments: its data folder,
//! [`crate::Locations::base`].
//!
//! Older versions let users keep them in another folder, often an Obsidian vault, recorded as
//! `vault_path` in the base's `global.json`. Each shipping start moves that folder's contents
//! into the base under the launch lock, before anything opens the vault
//! (`anlg_storage::vault::consolidate_custom_vault`); a failed copy keeps the setting and is
//! retried at the next start. A recorded folder that is missing, such as an unmounted drive, or
//! is the base itself holds nothing to move, and shipping only drops the setting.

use std::{
    fmt,
    path::{Path, PathBuf},
};

/// Notes and recordings that shipping's next start would move from a legacy custom folder into
/// the base. Until it has, the library is split between the two folders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnfinishedMove {
    pub from: PathBuf,
    pub to: PathBuf,
}

impl fmt::Display for UnfinishedMove {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "notes and recordings in {} have not finished moving into {}",
            self.from.display(),
            self.to.display()
        )
    }
}

/// The move shipping's next start would make into `base`, if any. Only reads.
pub fn unfinished_move(base: &Path) -> Option<UnfinishedMove> {
    anlg_storage::vault::pending_vault_path(base, base).map(|from| UnfinishedMove {
        from,
        to: base.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(root: &Path) -> Vec<(String, Vec<u8>)> {
        let mut files = Vec::new();
        let mut dirs = vec![root.to_path_buf()];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    dirs.push(path);
                } else {
                    let relative = path.strip_prefix(root).unwrap().display().to_string();
                    files.push((relative, std::fs::read(&path).unwrap()));
                }
            }
        }
        files.sort();
        files
    }

    fn seed_notes(folder: &Path) {
        std::fs::create_dir_all(folder.join("sessions/s1")).unwrap();
        std::fs::write(folder.join("sessions/s1/audio.wav"), "custom").unwrap();
        std::fs::write(folder.join("settings.json"), r#"{"theme":"dark"}"#).unwrap();
    }

    /// Every `vault_path` shipping can find at startup; an interrupted copy leaves the setting,
    /// so it is the separate-folder case. The native app must refuse exactly when shipping's
    /// consolidation would move notes into the base, and change nothing itself.
    #[test]
    fn refuses_exactly_when_shipping_would_still_move_notes() {
        for case in [
            "no config",
            "no override",
            "missing folder",
            "the base itself",
            "the base through $DEFAULT",
            "a separate folder",
        ] {
            let temp = tempfile::tempdir().unwrap();
            let base = temp.path().join("base");
            let custom = temp.path().join("vault");
            std::fs::create_dir_all(&base).unwrap();
            std::fs::write(base.join("store.json"), "{}").unwrap();
            let recorded = match case {
                "no config" => None,
                "no override" => Some(serde_json::json!({ "theme": "dark" })),
                "missing folder" => Some(serde_json::json!({ "vault_path": custom })),
                "the base itself" => Some(serde_json::json!({ "vault_path": base })),
                "the base through $DEFAULT" => {
                    seed_notes(&base);
                    Some(serde_json::json!({ "vault_path": "$DEFAULT" }))
                }
                "a separate folder" => {
                    seed_notes(&custom);
                    Some(serde_json::json!({ "vault_path": custom }))
                }
                _ => unreachable!(),
            };
            if let Some(config) = &recorded {
                std::fs::write(base.join("global.json"), config.to_string()).unwrap();
            }
            let before = tree(temp.path());

            let refused = unfinished_move(&base);
            assert_eq!(tree(temp.path()), before, "{case}: only reads");

            let library = || {
                let mut files = tree(&base);
                files.retain(|(path, _)| path != "global.json");
                files
            };
            let library_before = library();
            anlg_storage::vault::consolidate_custom_vault(&base, &base).unwrap();
            assert_eq!(refused.is_some(), library() != library_before, "{case}");
            if let Some(refused) = refused {
                assert_eq!(
                    refused,
                    UnfinishedMove {
                        from: custom,
                        to: base.clone()
                    }
                );
            }
            // Once shipping has run, the library is in the base and nothing is pending.
            assert_eq!(unfinished_move(&base), None, "{case}");
        }
    }
}
