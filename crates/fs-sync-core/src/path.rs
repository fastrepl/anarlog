use std::path::Path;

use uuid::Uuid;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{UUID_1, UUID_2};

    #[test]
    fn test_is_uuid() {
        assert!(is_uuid(UUID_1));
        assert!(is_uuid(UUID_2));
        assert!(is_uuid("550E8400-E29B-41D4-A716-446655440000"));
        assert!(!is_uuid("_default"));
        assert!(!is_uuid("work"));
        assert!(!is_uuid("not-a-uuid"));
    }
}
