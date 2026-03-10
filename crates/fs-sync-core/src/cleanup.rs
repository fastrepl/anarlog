use std::collections::HashSet;
use std::path::Path;

use crate::path::{is_uuid, to_relative_path};

pub fn cleanup_files_in_dir(
    dir: &Path,
    extension: &str,
    valid_ids: &HashSet<String>,
) -> std::io::Result<u32> {
    if !dir.exists() {
        return Ok(0);
    }

    let base_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");

    let orphans: Vec<_> = std::fs::read_dir(dir)?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return None;
            }
            let stem = path.file_stem()?.to_str()?;
            if path.extension()?.to_str()? != extension || !is_uuid(stem) {
                return None;
            }
            (!valid_ids.contains(stem)).then_some(path)
        })
        .collect();

    let mut removed = 0;
    for path in orphans {
        let relative_path = format!(
            "{}/{}",
            base_name,
            path.file_name().unwrap().to_str().unwrap()
        );
        if let Err(e) = std::fs::remove_file(&path) {
            tracing::warn!(
                hyprnote.file.path = %relative_path,
                error = %e,
                "failed_to_remove_orphan_file"
            );
        } else {
            tracing::debug!(hyprnote.file.path = %relative_path, "orphan_file_removed");
            removed += 1;
        }
    }

    Ok(removed)
}

fn for_each_entity_dir(
    _base_dir: &Path,
    current_dir: &Path,
    marker_file: &str,
    callback: &mut impl FnMut(&Path, &str),
) {
    let Ok(entries) = std::fs::read_dir(current_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        let has_marker = path.join(marker_file).exists();

        if has_marker && is_uuid(name) {
            callback(&path, name);
        } else if !has_marker && !is_uuid(name) {
            for_each_entity_dir(_base_dir, &path, marker_file, callback);
        }
    }
}

pub fn cleanup_dirs_recursive(
    base_dir: &Path,
    marker_file: &str,
    valid_ids: &HashSet<String>,
) -> std::io::Result<u32> {
    if !base_dir.exists() {
        return Ok(0);
    }

    let mut removed = 0;
    for_each_entity_dir(base_dir, base_dir, marker_file, &mut |path, name| {
        if !valid_ids.contains(name) {
            let relative_path = to_relative_path(path, base_dir);
            if let Err(e) = std::fs::remove_dir_all(path) {
                tracing::warn!(
                    hyprnote.file.path = %relative_path,
                    error = %e,
                    "failed_to_remove_orphan_directory"
                );
            } else {
                tracing::info!(hyprnote.file.path = %relative_path, "orphan_directory_removed");
                removed += 1;
            }
        }
    });
    Ok(removed)
}

pub fn cleanup_files_recursive(
    base_dir: &Path,
    marker_file: &str,
    extension: &str,
    valid_ids: &HashSet<String>,
) -> std::io::Result<u32> {
    if !base_dir.exists() {
        return Ok(0);
    }

    let mut removed = 0;
    for_each_entity_dir(base_dir, base_dir, marker_file, &mut |entity_dir, _| {
        removed += cleanup_files_in_entity_dir(entity_dir, extension, valid_ids);
    });
    Ok(removed)
}

fn cleanup_files_in_entity_dir(
    entity_dir: &Path,
    extension: &str,
    valid_ids: &HashSet<String>,
) -> u32 {
    let Ok(entries) = std::fs::read_dir(entity_dir) else {
        return 0;
    };

    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if ext != extension {
            continue;
        }

        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if !is_uuid(stem) {
            continue;
        }

        if !valid_ids.contains(stem) && std::fs::remove_file(&path).is_ok() {
            tracing::debug!(hyprnote.file.path = %path.display(), "orphan_file_removed");
            removed += 1;
        }
    }

    removed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{TestEnv, UUID_1, UUID_2, UUID_3};
    use assert_fs::TempDir;
    use assert_fs::assert::PathAssert;
    use assert_fs::fixture::PathChild;
    use predicates::prelude::*;

    #[test]
    fn cleanup_files_removes_orphan_uuid_files() {
        let env = TestEnv::new()
            .file(&format!("{UUID_1}.json"), "{}")
            .file(&format!("{UUID_2}.json"), "{}")
            .file("not-uuid.json", "{}")
            .build();

        let valid: HashSet<String> = [UUID_1.to_string()].into();
        let removed = cleanup_files_in_dir(env.path(), "json", &valid).unwrap();

        assert_eq!(removed, 1);
        env.child(&format!("{UUID_1}.json"))
            .assert(predicate::path::exists());
        env.child(&format!("{UUID_2}.json"))
            .assert(predicate::path::missing());
        env.child("not-uuid.json").assert(predicate::path::exists());
    }

    #[test]
    fn cleanup_files_nonexistent_dir_returns_zero() {
        let temp = TempDir::new().unwrap();
        let nonexistent = temp.path().join("nope");

        let removed = cleanup_files_in_dir(&nonexistent, "json", &HashSet::new()).unwrap();
        assert_eq!(removed, 0);
    }

    #[test]
    fn cleanup_dirs_removes_orphan_session_dirs() {
        let env = TestEnv::new()
            .session(UUID_1)
            .done()
            .session(UUID_2)
            .done()
            .build();

        let valid: HashSet<String> = [UUID_1.to_string()].into();
        let removed = cleanup_dirs_recursive(env.path(), "_meta.json", &valid).unwrap();

        assert_eq!(removed, 1);
        env.child(UUID_1).assert(predicate::path::exists());
        env.child(UUID_2).assert(predicate::path::missing());
    }

    #[test]
    fn cleanup_dirs_in_nested_folders() {
        let env = TestEnv::new()
            .folder("work")
            .session(UUID_1)
            .done_folder()
            .session(UUID_2)
            .done_folder()
            .done()
            .build();

        let valid: HashSet<String> = [UUID_1.to_string()].into();
        let removed = cleanup_dirs_recursive(env.path(), "_meta.json", &valid).unwrap();

        assert_eq!(removed, 1);
        env.child("work")
            .child(UUID_1)
            .assert(predicate::path::exists());
        env.child("work")
            .child(UUID_2)
            .assert(predicate::path::missing());
    }

    #[test]
    fn cleanup_files_recursive_removes_orphan_notes() {
        let env = TestEnv::new()
            .session(UUID_1)
            .note(UUID_2, "valid")
            .note(UUID_3, "orphan")
            .done()
            .build();

        let valid: HashSet<String> = [UUID_2.to_string()].into();
        let removed = cleanup_files_recursive(env.path(), "_meta.json", "md", &valid).unwrap();

        assert_eq!(removed, 1);
        env.child(UUID_1)
            .child(&format!("{UUID_2}.md"))
            .assert(predicate::path::exists());
        env.child(UUID_1)
            .child(&format!("{UUID_3}.md"))
            .assert(predicate::path::missing());
    }

    #[test]
    fn cleanup_files_recursive_in_nested_folders() {
        let env = TestEnv::new()
            .folder("work")
            .session(UUID_1)
            .note(UUID_2, "valid")
            .note(UUID_3, "orphan")
            .done_folder()
            .done()
            .build();

        let valid: HashSet<String> = [UUID_2.to_string()].into();
        let removed = cleanup_files_recursive(env.path(), "_meta.json", "md", &valid).unwrap();

        assert_eq!(removed, 1);
        env.child("work")
            .child(UUID_1)
            .child(&format!("{UUID_2}.md"))
            .assert(predicate::path::exists());
        env.child("work")
            .child(UUID_1)
            .child(&format!("{UUID_3}.md"))
            .assert(predicate::path::missing());
    }

    #[test]
    fn cleanup_files_recursive_ignores_non_uuid_files() {
        let env = TestEnv::new()
            .session(UUID_1)
            .note(UUID_2, "valid")
            .memo("memo content")
            .done()
            .build();

        let valid: HashSet<String> = [UUID_2.to_string()].into();
        let removed = cleanup_files_recursive(env.path(), "_meta.json", "md", &valid).unwrap();

        assert_eq!(removed, 0);
        env.child(UUID_1)
            .child(&format!("{UUID_2}.md"))
            .assert(predicate::path::exists());
        env.child(UUID_1)
            .child("_memo.md")
            .assert(predicate::path::exists());
    }
}
