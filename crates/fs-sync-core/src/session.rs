use std::path::{Path, PathBuf};

use crate::path::is_uuid;

pub fn find_session_dir(sessions_base: &Path, session_id: &str) -> PathBuf {
    if let Some(found) = find_session_dir_recursive(sessions_base, session_id) {
        return found;
    }
    sessions_base.join(session_id)
}

fn find_session_dir_recursive(dir: &Path, session_id: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = path.file_name()?.to_str()?;

        if name == session_id {
            return Some(path);
        }

        if !is_uuid(name)
            && let Some(found) = find_session_dir_recursive(&path, session_id)
        {
            return Some(found);
        }
    }

    None
}

pub fn delete_session_dir(session_dir: &Path) -> std::io::Result<()> {
    if session_dir.exists() {
        std::fs::remove_dir_all(session_dir)?;
    }
    Ok(())
}

pub fn list_uuid_files(dir: &Path, ext: &str) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return None;
            }
            if path.extension().and_then(|e| e.to_str()) != Some(ext) {
                return None;
            }
            let stem = path.file_stem()?.to_str()?;
            if !is_uuid(stem) {
                return None;
            }
            Some((stem.to_string(), path))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{TestEnv, UUID_1, UUID_2};
    use assert_fs::TempDir;
    use assert_fs::prelude::*;
    use predicates::prelude::*;

    #[test]
    fn find_session_at_root() {
        let env = TestEnv::new()
            .folder("sessions")
            .session(UUID_1)
            .done_folder()
            .done()
            .build();

        let result = find_session_dir(&env.path().join("sessions"), UUID_1);
        assert_eq!(result, env.folder_session_path("sessions", UUID_1));
    }

    #[test]
    fn find_session_in_nested_folder() {
        let env = TestEnv::new()
            .folder("sessions")
            .done()
            .folder("sessions/work")
            .done()
            .folder("sessions/work/project")
            .session(UUID_1)
            .done_folder()
            .done()
            .build();

        let result = find_session_dir(&env.path().join("sessions"), UUID_1);
        assert_eq!(
            result,
            env.path().join("sessions/work/project").join(UUID_1)
        );
    }

    #[test]
    fn find_session_fallback_when_not_found() {
        let temp = TempDir::new().unwrap();
        let sessions = temp.child("sessions");
        sessions.create_dir_all().unwrap();

        let result = find_session_dir(sessions.path(), UUID_1);
        assert_eq!(result, sessions.path().join(UUID_1));
    }

    #[test]
    fn delete_session_dir_removes_directory() {
        let env = TestEnv::new().session(UUID_1).done().build();

        delete_session_dir(&env.session_path(UUID_1)).unwrap();
        env.child(UUID_1).assert(predicate::path::missing());
    }

    #[test]
    fn delete_session_dir_noop_if_missing() {
        let temp = TempDir::new().unwrap();
        let missing = temp.path().join(UUID_1);

        let result = delete_session_dir(&missing);
        assert!(result.is_ok());
    }

    #[test]
    fn list_uuid_files_nonexistent_dir_returns_empty() {
        let temp = TempDir::new().unwrap();
        let nonexistent = temp.path().join("does_not_exist");

        let result = list_uuid_files(&nonexistent, "md");

        assert!(result.is_empty());
    }

    #[test]
    fn list_uuid_files_empty_dir_returns_empty() {
        let env = TestEnv::new().build();

        let result = list_uuid_files(env.path(), "md");

        assert!(result.is_empty());
    }

    #[test]
    fn list_uuid_files_finds_uuid_files() {
        let env = TestEnv::new()
            .file(&format!("{UUID_1}.md"), "content1")
            .file(&format!("{UUID_2}.md"), "content2")
            .build();

        let result = list_uuid_files(env.path(), "md");

        assert_eq!(result.len(), 2);
        let ids: Vec<_> = result.iter().map(|(id, _)| id.as_str()).collect();
        assert!(ids.contains(&UUID_1));
        assert!(ids.contains(&UUID_2));
    }

    #[test]
    fn list_uuid_files_skips_non_uuid_filenames() {
        let env = TestEnv::new()
            .file(&format!("{UUID_1}.md"), "valid")
            .file("not-a-uuid.md", "skip")
            .file("readme.md", "skip")
            .build();

        let result = list_uuid_files(env.path(), "md");

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, UUID_1);
    }

    #[test]
    fn list_uuid_files_skips_wrong_extension() {
        let env = TestEnv::new()
            .file(&format!("{UUID_1}.md"), "valid")
            .file(&format!("{UUID_1}.txt"), "skip")
            .file(&format!("{UUID_1}.json"), "skip")
            .build();

        let result = list_uuid_files(env.path(), "md");

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, UUID_1);
    }

    #[test]
    fn list_uuid_files_skips_directories() {
        let env = TestEnv::new()
            .file(&format!("{UUID_1}.md"), "valid")
            .folder(UUID_2)
            .done()
            .build();

        let result = list_uuid_files(env.path(), "md");

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, UUID_1);
    }
}
