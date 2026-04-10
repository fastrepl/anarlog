use std::path::Path;

use hypr_version::Version;

use crate::Result;

const CHAR_DIR: &str = ".char";
const VERSION_FILE: &str = "version";

pub fn exists(base_dir: &Path) -> bool {
    base_dir.join(CHAR_DIR).join(VERSION_FILE).exists()
}

pub fn read(base_dir: &Path) -> Option<Version> {
    let version_file = base_dir.join(CHAR_DIR).join(VERSION_FILE);
    if version_file.exists() {
        let content = std::fs::read_to_string(&version_file).ok()?;
        return content.trim().parse().ok();
    }
    None
}

pub fn write(base_dir: &Path, version: &Version) -> Result<()> {
    let char_dir = base_dir.join(CHAR_DIR);
    std::fs::create_dir_all(&char_dir)?;
    std::fs::write(char_dir.join(VERSION_FILE), version.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_exists_returns_false_when_no_version_file() {
        let temp = tempdir().unwrap();
        assert!(!exists(temp.path()));
    }

    #[test]
    fn test_exists_returns_true_when_version_file_exists() {
        let temp = tempdir().unwrap();
        let char_dir = temp.path().join(CHAR_DIR);
        std::fs::create_dir_all(&char_dir).unwrap();
        std::fs::write(char_dir.join(VERSION_FILE), "1.0.0").unwrap();
        assert!(exists(temp.path()));
    }

    #[test]
    fn test_read_returns_none_when_no_version_file() {
        let temp = tempdir().unwrap();
        assert_eq!(read(temp.path()), None);
    }

    #[test]
    fn test_read_returns_version_when_file_exists() {
        let temp = tempdir().unwrap();
        let char_dir = temp.path().join(CHAR_DIR);
        std::fs::create_dir_all(&char_dir).unwrap();
        std::fs::write(char_dir.join(VERSION_FILE), "1.0.2-nightly.14").unwrap();

        let result = read(temp.path());
        assert_eq!(result, Some("1.0.2-nightly.14".parse().unwrap()));
    }

    #[test]
    fn test_read_returns_none_for_malformed_version() {
        let temp = tempdir().unwrap();
        let char_dir = temp.path().join(CHAR_DIR);
        std::fs::create_dir_all(&char_dir).unwrap();
        std::fs::write(char_dir.join(VERSION_FILE), "not-a-version").unwrap();

        assert_eq!(read(temp.path()), None);
    }

    #[test]
    fn test_read_trims_whitespace() {
        let temp = tempdir().unwrap();
        let char_dir = temp.path().join(CHAR_DIR);
        std::fs::create_dir_all(&char_dir).unwrap();
        std::fs::write(char_dir.join(VERSION_FILE), "  1.0.1  \n").unwrap();

        let result = read(temp.path());
        assert_eq!(result, Some(Version::new(1, 0, 1)));
    }

    #[test]
    fn test_write_creates_char_dir_and_version_file() {
        let temp = tempdir().unwrap();
        let version = Version::new(1, 0, 2);

        write(temp.path(), &version).unwrap();

        assert!(temp.path().join(CHAR_DIR).exists());
        let content =
            std::fs::read_to_string(temp.path().join(CHAR_DIR).join(VERSION_FILE)).unwrap();
        assert_eq!(content, "1.0.2");
    }

    #[test]
    fn test_write_and_read_roundtrip() {
        let temp = tempdir().unwrap();
        let version: Version = "1.0.2-nightly.14".parse().unwrap();

        write(temp.path(), &version).unwrap();
        let result = read(temp.path());

        assert_eq!(result, Some(version));
    }
}
