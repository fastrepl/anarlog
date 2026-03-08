use std::path::Path;

use crate::path::{get_parent_folder_path, is_uuid};
use crate::types::{FolderInfo, ListFoldersResult};

pub fn scan_directory_recursive(
    sessions_dir: &Path,
    current_path: &str,
    result: &mut ListFoldersResult,
) {
    let full_path = if current_path.is_empty() {
        sessions_dir.to_path_buf()
    } else {
        sessions_dir.join(current_path)
    };

    let entries = match std::fs::read_dir(&full_path) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        let entry_path = if current_path.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", current_path, name)
        };

        let has_meta_json = sessions_dir.join(&entry_path).join("_meta.json").exists();

        if has_meta_json {
            result
                .session_folder_map
                .insert(name, current_path.to_string());
        } else if !is_uuid(&name) {
            let prev_session_count = result.session_folder_map.len();
            scan_directory_recursive(sessions_dir, &entry_path, result);
            let has_sessions = result.session_folder_map.len() > prev_session_count;

            if has_sessions {
                result.folders.insert(
                    entry_path.clone(),
                    FolderInfo {
                        name,
                        parent_folder_id: get_parent_folder_path(&entry_path),
                    },
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{TestEnv, UUID_1, UUID_2};
    use std::collections::HashMap;

    #[test]
    fn scan_directory_detects_sessions_with_meta() {
        let env = TestEnv::new()
            .session(UUID_1)
            .done()
            .session(UUID_2)
            .no_meta()
            .done()
            .build();

        let mut result = ListFoldersResult {
            folders: HashMap::new(),
            session_folder_map: HashMap::new(),
        };
        scan_directory_recursive(env.path(), "", &mut result);

        assert_eq!(result.session_folder_map.len(), 1);
        assert!(result.session_folder_map.contains_key(UUID_1));
        assert!(!result.session_folder_map.contains_key(UUID_2));
    }

    #[test]
    fn scan_directory_tracks_folders_with_sessions() {
        let env = TestEnv::new()
            .folder("work")
            .session(UUID_1)
            .done_folder()
            .done()
            .build();

        let mut result = ListFoldersResult {
            folders: HashMap::new(),
            session_folder_map: HashMap::new(),
        };
        scan_directory_recursive(env.path(), "", &mut result);

        assert!(result.folders.contains_key("work"));
        assert_eq!(result.folders["work"].name, "work");
    }
}
