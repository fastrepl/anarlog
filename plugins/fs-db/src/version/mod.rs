#[macro_use]
pub(crate) mod r#macro;

pub(crate) mod known;

use std::path::Path;

pub use hypr_version::Version;
pub(crate) use r#macro::version_from_name;

pub use known::write as write_version;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectedVersion {
    Fresh,
    FromFile(Version),
    Inferred(InferredVersion),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InferredVersion {
    V0_0_84,
    V1_0_1,
    V1_0_2NightlyEarly,
    V1_0_2NightlyLate,
}

pub async fn detect_version(base_dir: &Path) -> DetectedVersion {
    if let Some(version) = known::read(base_dir) {
        return DetectedVersion::FromFile(version);
    }

    infer_version(base_dir).await
}

async fn infer_version(base_dir: &Path) -> DetectedVersion {
    let sqlite_file = base_dir.join("db.sqlite");
    if !sqlite_file.exists() {
        return DetectedVersion::Fresh;
    }

    if hypr_db_parser::v0::validate(&sqlite_file).await.is_ok() {
        return DetectedVersion::Inferred(InferredVersion::V0_0_84);
    }

    if hypr_db_parser::v1::validate(&sqlite_file).await.is_err() {
        return DetectedVersion::Fresh;
    }

    let sessions_dir = base_dir.join("sessions");
    if !sessions_dir.exists() {
        return DetectedVersion::Inferred(InferredVersion::V1_0_1);
    }

    let entries = match std::fs::read_dir(&sessions_dir) {
        Ok(entries) => entries,
        Err(_) => {
            return DetectedVersion::Inferred(InferredVersion::V1_0_1);
        }
    };

    let session_dirs: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .collect();

    if session_dirs.is_empty() {
        return DetectedVersion::Inferred(InferredVersion::V1_0_1);
    }

    let has_session_without_meta = session_dirs
        .iter()
        .any(|session_dir| !session_dir.path().join("_meta.json").exists());

    if has_session_without_meta {
        return DetectedVersion::Inferred(InferredVersion::V1_0_1);
    }

    let has_old_transcript = session_dirs
        .iter()
        .any(|session_dir| session_dir.path().join("_transcript.json").exists());

    if has_old_transcript {
        return DetectedVersion::Inferred(InferredVersion::V1_0_2NightlyEarly);
    }

    DetectedVersion::Inferred(InferredVersion::V1_0_2NightlyLate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_fresh_vault() {
        let temp = tempdir().unwrap();
        assert_eq!(detect_version(temp.path()).await, DetectedVersion::Fresh);
    }

    #[tokio::test]
    async fn test_from_version_file() {
        let temp = tempdir().unwrap();
        let version: Version = "1.0.2-nightly.15".parse().unwrap();
        write_version(temp.path(), &version).unwrap();

        assert_eq!(
            detect_version(temp.path()).await,
            DetectedVersion::FromFile(version)
        );
    }

    #[tokio::test]
    async fn test_v1_0_1_real_data() {
        let path = std::path::Path::new("/tmp/char-data/v1.0.1/simple");
        if !path.exists() {
            return;
        }

        assert_eq!(
            detect_version(path).await,
            DetectedVersion::Inferred(InferredVersion::V1_0_1)
        );
    }

    #[tokio::test]
    async fn test_empty_sessions_dir_with_v1_sqlite() {
        let temp = tempdir().unwrap();
        copy_db("/tmp/char-data/v1.0.1/simple/db.sqlite", temp.path());
        std::fs::create_dir_all(temp.path().join("sessions")).unwrap();

        assert_eq!(
            detect_version(temp.path()).await,
            DetectedVersion::Inferred(InferredVersion::V1_0_1)
        );
    }

    fn copy_db(src: &str, dest_dir: &Path) {
        let src_path = std::path::Path::new(src);
        if src_path.exists() {
            std::fs::copy(src_path, dest_dir.join("db.sqlite")).unwrap();
        }
    }
}
