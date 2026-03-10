#[cfg(test)]
mod test_fixtures;

pub mod audio;
pub mod cleanup;
pub mod error;
pub mod folder;
pub mod frontmatter;
pub mod json;
pub mod path;
pub mod scan;
pub mod session;
pub mod session_content;
pub mod types;

pub use error::{Error, Result};
pub use path::{build_session_dir, is_uuid, normalize_folder_path};
pub use session::find_session_dir;
pub use types::*;

use std::collections::HashSet;
use std::path::PathBuf;

pub struct FsSyncCore {
    base_dir: PathBuf,
    sessions_dir: PathBuf,
}

impl FsSyncCore {
    pub fn new(base_dir: PathBuf) -> Self {
        let sessions_dir = base_dir.join("sessions");
        Self {
            base_dir,
            sessions_dir,
        }
    }

    pub fn list_folders(&self) -> Result<ListFoldersResult> {
        let mut result = ListFoldersResult {
            folders: std::collections::HashMap::new(),
            session_folder_map: std::collections::HashMap::new(),
        };

        if !self.sessions_dir.exists() {
            return Ok(result);
        }

        folder::scan_directory_recursive(&self.sessions_dir, "", &mut result);

        Ok(result)
    }

    pub fn move_session(
        &self,
        session_id: &str,
        from_folder_path: &str,
        target_folder_path: &str,
    ) -> Result<MoveSessionResult> {
        let from_folder_path = normalize_folder_path(from_folder_path)?;
        let target_folder_path = normalize_folder_path(target_folder_path)?;

        if from_folder_path == target_folder_path {
            return Err(Error::Path("session_move_noop".into()));
        }

        let source = build_session_dir(&self.sessions_dir, &from_folder_path, session_id)?;
        if !source.exists() {
            return Err(Error::Path("session_source_missing".into()));
        }

        let target = build_session_dir(&self.sessions_dir, &target_folder_path, session_id)?;
        if target.exists() {
            return Err(Error::Path("session_target_exists".into()));
        }

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&source, &target)?;

        tracing::info!(
            "Moved session {} from {:?} to {:?}",
            session_id,
            source,
            target
        );

        Ok(MoveSessionResult {
            session_id: session_id.to_string(),
            folder_id: target_folder_path,
        })
    }

    pub fn create_folder(&self, folder_path: &str) -> Result<()> {
        let folder = self.sessions_dir.join(folder_path);

        if folder.exists() {
            return Ok(());
        }

        std::fs::create_dir_all(&folder)?;
        tracing::info!("Created folder: {:?}", folder);
        Ok(())
    }

    pub fn rename_folder(&self, old_path: &str, new_path: &str) -> Result<RenameFolderResult> {
        let old_path = normalize_folder_path(old_path)?;
        let new_path = normalize_folder_path(new_path)?;

        if old_path.is_empty() || new_path.is_empty() {
            return Err(Error::Path("folder_rename_root_not_allowed".into()));
        }
        if old_path == new_path {
            return Err(Error::Path("folder_rename_noop".into()));
        }

        let source = self.sessions_dir.join(&old_path);
        let target = self.sessions_dir.join(&new_path);

        if !source.exists() {
            return Err(Error::Path("folder_source_missing".into()));
        }

        if target.exists() {
            return Err(Error::Path("folder_target_exists".into()));
        }

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::rename(&source, &target)?;
        tracing::info!("Renamed folder from {:?} to {:?}", source, target);

        let mut updates = Vec::new();
        folder::collect_session_updates(&self.sessions_dir, &new_path, &mut updates);
        updates.sort_by(|a, b| a.session_id.cmp(&b.session_id));

        Ok(RenameFolderResult { updates })
    }

    pub fn delete_folder(&self, folder_path: &str) -> Result<()> {
        let folder = self.sessions_dir.join(folder_path);

        if !folder.exists() {
            return Ok(());
        }

        if self.folder_contains_sessions(&folder)? {
            return Err(Error::Path(
                "Cannot delete folder containing sessions. Move or delete sessions first."
                    .to_string(),
            ));
        }

        std::fs::remove_dir_all(&folder)?;
        tracing::info!("Deleted folder: {:?}", folder);
        Ok(())
    }

    fn folder_contains_sessions(&self, folder: &PathBuf) -> Result<bool> {
        let entries = std::fs::read_dir(folder)?;

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };

            if is_uuid(name) && path.join("_meta.json").exists() {
                return Ok(true);
            }

            if !is_uuid(name) && self.folder_contains_sessions(&path)? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn cleanup_orphan(&self, target: CleanupTarget, valid_ids: Vec<String>) -> Result<u32> {
        let valid_set: HashSet<String> = valid_ids.into_iter().collect();

        match target {
            CleanupTarget::Files { subdir, extension } => {
                let dir = self.base_dir.join(&subdir);
                Ok(cleanup::cleanup_files_in_dir(&dir, &extension, &valid_set)?)
            }
            CleanupTarget::Dirs {
                subdir,
                marker_file,
            } => {
                let dir = self.base_dir.join(&subdir);
                Ok(cleanup::cleanup_dirs_recursive(
                    &dir,
                    &marker_file,
                    &valid_set,
                )?)
            }
            CleanupTarget::FilesRecursive {
                subdir,
                marker_file,
                extension,
            } => {
                let dir = self.base_dir.join(&subdir);
                Ok(cleanup::cleanup_files_recursive(
                    &dir,
                    &marker_file,
                    &extension,
                    &valid_set,
                )?)
            }
        }
    }

    pub fn attachment_save(
        &self,
        session_id: &str,
        data: &[u8],
        filename: &str,
    ) -> Result<AttachmentSaveResult> {
        let session_dir = self.resolve_session_dir(session_id);
        let attachments_dir = session_dir.join("attachments");

        std::fs::create_dir_all(&attachments_dir)?;

        let safe_filename = sanitize_filename(filename)?;
        let (file_path, final_filename) =
            write_unique_file(&attachments_dir, &safe_filename, data)?;

        Ok(AttachmentSaveResult {
            path: file_path.to_string_lossy().to_string(),
            attachment_id: final_filename,
        })
    }

    pub fn attachment_list(&self, session_id: &str) -> Result<Vec<AttachmentInfo>> {
        let session_dir = self.resolve_session_dir(session_id);
        let attachments_dir = session_dir.join("attachments");

        let mut attachments = Vec::new();

        let entries = match std::fs::read_dir(&attachments_dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(attachments),
            Err(e) => return Err(e.into()),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let filename = match path.file_name().and_then(|s| s.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            let extension = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_string();

            let modified_at = entry
                .metadata()
                .and_then(|m| m.modified())
                .map(|t| {
                    chrono::DateTime::<chrono::Utc>::from(t)
                        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                })
                .unwrap_or_default();

            attachments.push(AttachmentInfo {
                attachment_id: filename,
                path: path.to_string_lossy().to_string(),
                extension,
                modified_at,
            });
        }

        Ok(attachments)
    }

    pub fn attachment_remove(&self, session_id: &str, attachment_id: &str) -> Result<()> {
        let session_dir = self.resolve_session_dir(session_id);
        let attachments_dir = session_dir.join("attachments");

        let entries = match std::fs::read_dir(&attachments_dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let filename = match path.file_name().and_then(|s| s.to_str()) {
                Some(name) => name,
                None => continue,
            };

            if filename == attachment_id {
                std::fs::remove_file(&path)?;
                return Ok(());
            }
        }

        Ok(())
    }

    pub fn resolve_session_dir(&self, session_id: &str) -> PathBuf {
        find_session_dir(&self.sessions_dir, session_id)
    }
}

fn sanitize_filename(filename: &str) -> Result<String> {
    let path = std::path::Path::new(filename);

    let clean_name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
        Error::from(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid filename",
        ))
    })?;

    if clean_name.is_empty() || clean_name.contains(['/', '\\', '\0']) {
        return Err(Error::from(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid filename characters",
        )));
    }

    Ok(clean_name.to_string())
}

fn write_unique_file(
    dir: &std::path::Path,
    filename: &str,
    data: &[u8],
) -> Result<(PathBuf, String)> {
    use std::fs::OpenOptions;
    use std::io::Write;

    let path = std::path::Path::new(filename);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);
    let extension = path.extension().and_then(|e| e.to_str());

    let mut counter = 0;
    loop {
        let candidate_filename = if counter == 0 {
            filename.to_string()
        } else {
            match extension {
                Some(ext) => format!("{} {}.{}", stem, counter, ext),
                None => format!("{} {}", stem, counter),
            }
        };

        let candidate_path = dir.join(&candidate_filename);

        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate_path)
        {
            Ok(mut file) => {
                file.write_all(data)?;
                return Ok((candidate_path, candidate_filename));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                counter += 1;
                continue;
            }
            Err(e) => return Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use assert_fs::TempDir;
    use assert_fs::fixture::PathChild;
    use assert_fs::prelude::*;

    use super::*;
    use crate::test_fixtures::{UUID_1, UUID_2};

    #[test]
    fn move_session_to_folder() {
        let temp = TempDir::new().unwrap();
        temp.child("sessions")
            .child(UUID_1)
            .create_dir_all()
            .unwrap();
        temp.child("sessions")
            .child(UUID_1)
            .child("_meta.json")
            .write_str("{}")
            .unwrap();

        let core = FsSyncCore::new(temp.path().to_path_buf());
        let result = core.move_session(UUID_1, "", "work").unwrap();

        temp.child("sessions")
            .child("work")
            .child(UUID_1)
            .assert(predicates::path::exists());
        assert_eq!(
            result,
            MoveSessionResult {
                session_id: UUID_1.into(),
                folder_id: "work".into(),
            }
        );
    }

    #[test]
    fn move_session_to_root() {
        let temp = TempDir::new().unwrap();
        temp.child("sessions")
            .child("work")
            .child(UUID_1)
            .create_dir_all()
            .unwrap();
        temp.child("sessions")
            .child("work")
            .child(UUID_1)
            .child("_meta.json")
            .write_str("{}")
            .unwrap();

        let core = FsSyncCore::new(temp.path().to_path_buf());
        let result = core.move_session(UUID_1, "work", "").unwrap();

        temp.child("sessions")
            .child(UUID_1)
            .assert(predicates::path::exists());
        assert_eq!(
            result,
            MoveSessionResult {
                session_id: UUID_1.into(),
                folder_id: "".into(),
            }
        );
    }

    #[test]
    fn move_session_missing_source_errors() {
        let temp = TempDir::new().unwrap();
        temp.child("sessions").create_dir_all().unwrap();

        let core = FsSyncCore::new(temp.path().to_path_buf());
        let result = core.move_session(UUID_1, "", "work");

        assert!(matches!(result, Err(Error::Path(message)) if message == "session_source_missing"));
    }

    #[test]
    fn move_session_existing_target_errors() {
        let temp = TempDir::new().unwrap();
        temp.child("sessions")
            .child(UUID_1)
            .create_dir_all()
            .unwrap();
        temp.child("sessions")
            .child(UUID_1)
            .child("_meta.json")
            .write_str("{}")
            .unwrap();
        temp.child("sessions")
            .child("work")
            .child(UUID_1)
            .create_dir_all()
            .unwrap();
        temp.child("sessions")
            .child("work")
            .child(UUID_1)
            .child("_meta.json")
            .write_str("{}")
            .unwrap();

        let core = FsSyncCore::new(temp.path().to_path_buf());
        let result = core.move_session(UUID_1, "", "work");

        assert!(matches!(result, Err(Error::Path(message)) if message == "session_target_exists"));
    }

    #[test]
    fn rename_folder_target_exists_errors() {
        let temp = TempDir::new().unwrap();
        temp.child("sessions")
            .child("old")
            .create_dir_all()
            .unwrap();
        temp.child("sessions")
            .child("new")
            .create_dir_all()
            .unwrap();

        let core = FsSyncCore::new(temp.path().to_path_buf());
        let result = core.rename_folder("old", "new");

        assert!(result.is_err());
    }

    #[test]
    fn rename_folder_returns_updated_sessions() {
        let temp = TempDir::new().unwrap();
        temp.child("sessions")
            .child("old")
            .child(UUID_1)
            .create_dir_all()
            .unwrap();
        temp.child("sessions")
            .child("old")
            .child(UUID_1)
            .child("_meta.json")
            .write_str("{}")
            .unwrap();
        temp.child("sessions")
            .child("old")
            .child("nested")
            .child(UUID_2)
            .create_dir_all()
            .unwrap();
        temp.child("sessions")
            .child("old")
            .child("nested")
            .child(UUID_2)
            .child("_meta.json")
            .write_str("{}")
            .unwrap();

        let core = FsSyncCore::new(temp.path().to_path_buf());
        let result = core.rename_folder("old", "new").unwrap();

        assert_eq!(
            result,
            RenameFolderResult {
                updates: vec![
                    FolderSessionUpdate {
                        session_id: UUID_1.into(),
                        folder_id: "new".into(),
                    },
                    FolderSessionUpdate {
                        session_id: UUID_2.into(),
                        folder_id: "new/nested".into(),
                    },
                ],
            }
        );
    }

    #[test]
    fn delete_folder_with_sessions_errors() {
        let temp = TempDir::new().unwrap();
        temp.child("sessions")
            .child("work")
            .child(UUID_1)
            .create_dir_all()
            .unwrap();
        temp.child("sessions")
            .child("work")
            .child(UUID_1)
            .child("_meta.json")
            .write_str("{}")
            .unwrap();

        let core = FsSyncCore::new(temp.path().to_path_buf());
        let result = core.delete_folder("work");

        assert!(result.is_err());
        temp.child("sessions")
            .child("work")
            .assert(predicates::path::exists());
    }

    #[test]
    fn attachment_save_dedup_naming() {
        let temp = TempDir::new().unwrap();
        temp.child("sessions")
            .child(UUID_1)
            .create_dir_all()
            .unwrap();

        let core = FsSyncCore::new(temp.path().to_path_buf());
        let first = core.attachment_save(UUID_1, b"hello", "file.txt").unwrap();
        let second = core.attachment_save(UUID_1, b"world", "file.txt").unwrap();

        assert_eq!(first.attachment_id, "file.txt");
        assert_eq!(second.attachment_id, "file 1.txt");
        temp.child("sessions")
            .child(UUID_1)
            .child("attachments")
            .child("file.txt")
            .assert(predicates::path::exists());
        temp.child("sessions")
            .child(UUID_1)
            .child("attachments")
            .child("file 1.txt")
            .assert(predicates::path::exists());
    }

    #[test]
    fn attachment_remove_missing_noop() {
        let temp = TempDir::new().unwrap();
        temp.child("sessions")
            .child(UUID_1)
            .create_dir_all()
            .unwrap();

        let core = FsSyncCore::new(temp.path().to_path_buf());
        core.attachment_remove(UUID_1, "missing.txt").unwrap();
    }

    #[test]
    fn cleanup_orphan_files_removes_invalid_ids() {
        let temp = TempDir::new().unwrap();
        temp.child("notes").create_dir_all().unwrap();
        temp.child("notes")
            .child(format!("{UUID_1}.json"))
            .write_str("{}")
            .unwrap();
        temp.child("notes")
            .child(format!("{UUID_2}.json"))
            .write_str("{}")
            .unwrap();

        let core = FsSyncCore::new(temp.path().to_path_buf());
        let removed = core
            .cleanup_orphan(
                CleanupTarget::Files {
                    subdir: "notes".to_string(),
                    extension: "json".to_string(),
                },
                vec![UUID_1.to_string()],
            )
            .unwrap();

        assert_eq!(removed, 1);
    }

    #[test]
    fn sanitize_filename_rejects_empty() {
        assert!(sanitize_filename("").is_err());
    }

    #[test]
    fn sanitize_filename_strips_directories() {
        let result = sanitize_filename("nested/path/file.txt").unwrap();
        assert_eq!(result, "file.txt");
    }
}
