use std::collections::HashMap;
use std::path::{Path, PathBuf};

use glob::Pattern;
use rayon::prelude::*;

use crate::path::{is_uuid, to_relative_path};
use crate::types::ScanResult;

pub fn scan_and_read(
    scan_dir: &Path,
    relative_to: &Path,
    file_patterns: &[String],
    recursive: bool,
    path_filter: Option<&str>,
) -> ScanResult {
    if !scan_dir.exists() {
        return ScanResult {
            files: HashMap::new(),
            dirs: Vec::new(),
        };
    }

    let patterns: Vec<Pattern> = file_patterns
        .iter()
        .filter_map(|p| Pattern::new(p).ok())
        .collect();

    let mut files = HashMap::new();
    let mut dirs = Vec::new();

    scan_directory_for_files(
        relative_to,
        scan_dir,
        &patterns,
        recursive,
        &mut files,
        &mut dirs,
    );

    let files: HashMap<String, String> = files
        .into_par_iter()
        .filter(|(rel_path, _)| {
            path_filter
                .map(|filter| rel_path.contains(filter))
                .unwrap_or(true)
        })
        .filter_map(|(rel_path, abs_path)| {
            std::fs::read_to_string(&abs_path)
                .ok()
                .map(|content| (rel_path, content))
        })
        .collect();

    ScanResult { files, dirs }
}

fn scan_directory_for_files(
    base_path: &Path,
    current_path: &Path,
    patterns: &[Pattern],
    recursive: bool,
    files: &mut HashMap<String, PathBuf>,
    dirs: &mut Vec<String>,
) {
    let entries = match std::fs::read_dir(current_path) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        if path.is_dir() {
            let rel_path = to_relative_path(&path, base_path);

            if !is_uuid(name) {
                dirs.push(rel_path);
            }

            if recursive && !is_uuid(name) {
                scan_directory_for_files(base_path, &path, patterns, recursive, files, dirs);
            } else if is_uuid(name) {
                scan_directory_for_files(base_path, &path, patterns, false, files, dirs);
            }
        } else if path.is_file() && patterns.iter().any(|p| p.matches(name)) {
            files.insert(to_relative_path(&path, base_path), path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{TestEnv, UUID_1};
    use assert_fs::TempDir;

    #[test]
    fn nonexistent_dir_returns_empty() {
        let temp = TempDir::new().unwrap();
        let nonexistent = temp.path().join("does_not_exist");

        let result = scan_and_read(&nonexistent, &nonexistent, &["*.txt".into()], true, None);

        assert!(result.files.is_empty());
        assert!(result.dirs.is_empty());
    }

    #[test]
    fn matches_files_by_pattern() {
        let env = TestEnv::new()
            .file("note.txt", "hello")
            .file("data.json", "{}")
            .build();

        let result = scan_and_read(env.path(), env.path(), &["*.txt".into()], false, None);

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files.get("note.txt"), Some(&"hello".into()));
    }

    #[test]
    fn recursive_finds_nested_files() {
        let env = TestEnv::new()
            .file("root.txt", "root")
            .folder("sub")
            .file("nested.txt", "nested")
            .done()
            .build();

        let result = scan_and_read(env.path(), env.path(), &["*.txt".into()], true, None);

        assert_eq!(result.files.len(), 2);
        assert_eq!(result.files.get("root.txt"), Some(&"root".into()));
        assert_eq!(result.files.get("sub/nested.txt"), Some(&"nested".into()));
    }

    #[test]
    fn non_recursive_skips_nested_files() {
        let env = TestEnv::new()
            .file("root.txt", "root")
            .folder("sub")
            .file("nested.txt", "nested")
            .done()
            .build();

        let result = scan_and_read(env.path(), env.path(), &["*.txt".into()], false, None);

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files.get("root.txt"), Some(&"root".into()));
    }

    #[test]
    fn collects_non_uuid_directories() {
        let env = TestEnv::new()
            .folder("work")
            .done()
            .folder("personal")
            .done()
            .build();

        let result = scan_and_read(env.path(), env.path(), &["*.txt".into()], true, None);

        assert!(result.dirs.contains(&"work".into()));
        assert!(result.dirs.contains(&"personal".into()));
    }

    #[test]
    fn uuid_dirs_not_in_dirs_list_but_files_are_scanned() {
        let env = TestEnv::new()
            .folder(UUID_1)
            .file("note.txt", "inside uuid")
            .done()
            .build();

        let result = scan_and_read(env.path(), env.path(), &["*.txt".into()], false, None);

        assert!(!result.dirs.iter().any(|d| d.contains(UUID_1)));
        assert_eq!(
            result.files.get(&format!("{UUID_1}/note.txt")),
            Some(&"inside uuid".into())
        );
    }

    #[test]
    fn paths_relative_to_different_base() {
        let env = TestEnv::new()
            .folder(&format!("sessions/{UUID_1}"))
            .file("_meta.json", "{}")
            .done()
            .build();

        let scan_dir = env.path().join("sessions").join(UUID_1);
        let result = scan_and_read(&scan_dir, env.path(), &["*.json".into()], false, None);

        assert_eq!(result.files.len(), 1);
        assert_eq!(
            result.files.get(&format!("sessions/{UUID_1}/_meta.json")),
            Some(&"{}".into())
        );
    }
}
