use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::error::Result;

pub fn to_relative_path(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .ok()
        .and_then(|p| p.to_str())
        .map(|s| s.replace(std::path::MAIN_SEPARATOR, "/"))
        .unwrap_or_default()
}

pub fn is_uuid(name: &str) -> bool {
    Uuid::try_parse(name).is_ok()
}

pub fn get_parent_folder_path(path: &str) -> Option<String> {
    path.rsplit_once('/').map(|(parent, _)| parent.to_string())
}

pub fn normalize_folder_path(path: &str) -> Result<String> {
    let path = path.replace('\\', "/");

    if path.starts_with('/') {
        return Err(crate::Error::Path(
            "folder_path_absolute_not_allowed".into(),
        ));
    }

    let path = path.trim_matches('/');
    if path.is_empty() {
        return Ok(String::new());
    }

    let mut normalized = Vec::new();
    for segment in path.split('/') {
        if segment.is_empty() {
            return Err(crate::Error::Path("folder_path_empty_segment".into()));
        }
        if matches!(segment, "." | "..") {
            return Err(crate::Error::Path(
                "folder_path_traversal_not_allowed".into(),
            ));
        }
        normalized.push(segment);
    }

    Ok(normalized.join("/"))
}

pub fn build_session_dir(
    sessions_base: &Path,
    folder_path: &str,
    session_id: &str,
) -> Result<PathBuf> {
    let folder_path = normalize_folder_path(folder_path)?;

    if folder_path.is_empty() {
        return Ok(sessions_base.join(session_id));
    }

    Ok(sessions_base.join(folder_path).join(session_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{UUID_1, UUID_2};
    use std::path::PathBuf;

    #[test]
    fn test_is_uuid() {
        assert!(is_uuid(UUID_1));
        assert!(is_uuid(UUID_2));
        assert!(is_uuid("550E8400-E29B-41D4-A716-446655440000"));
        assert!(!is_uuid("_default"));
        assert!(!is_uuid("work"));
        assert!(!is_uuid("not-a-uuid"));
    }

    #[test]
    fn test_normalize_folder_path() {
        assert_eq!(normalize_folder_path("").unwrap(), "");
        assert_eq!(normalize_folder_path("work").unwrap(), "work");
        assert_eq!(
            normalize_folder_path("work/project-a").unwrap(),
            "work/project-a"
        );
        assert_eq!(normalize_folder_path("work/").unwrap(), "work");
        assert_eq!(
            normalize_folder_path(r"work\project-a").unwrap(),
            "work/project-a"
        );
    }

    #[test]
    fn test_normalize_folder_path_rejects_invalid_values() {
        assert!(normalize_folder_path("/work").is_err());
        assert!(normalize_folder_path("work//project").is_err());
        assert!(normalize_folder_path("./work").is_err());
        assert!(normalize_folder_path("../work").is_err());
    }

    #[test]
    fn test_build_session_dir() {
        let base = PathBuf::from("/tmp/sessions");
        assert_eq!(
            build_session_dir(&base, "", UUID_1).unwrap(),
            base.join(UUID_1)
        );
        assert_eq!(
            build_session_dir(&base, "work/project-a", UUID_1).unwrap(),
            base.join("work").join("project-a").join(UUID_1)
        );
    }
}
