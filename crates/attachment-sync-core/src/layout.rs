use std::path::PathBuf;

use crate::{Error, Result, hash_identifier, valid_cache_id};

pub const SHARED_PREVIEW_SCOPE_PREFIX: &str = "preview:";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferPaths {
    cache_base: PathBuf,
    data_base: PathBuf,
}

impl TransferPaths {
    pub fn new(cache_base: impl Into<PathBuf>, data_base: impl Into<PathBuf>) -> Self {
        Self {
            cache_base: cache_base.into(),
            data_base: data_base.into(),
        }
    }

    pub fn private_cache_root(&self) -> PathBuf {
        self.cache_root().join("private")
    }

    pub fn shared_upload_cache_root(&self) -> PathBuf {
        self.cache_root().join("shared-upload")
    }

    pub fn shared_cache_root(&self) -> PathBuf {
        self.cache_root().join("shared")
    }

    pub fn shared_preview_cache_root(&self) -> PathBuf {
        self.cache_root().join("shared-preview")
    }

    pub fn delete_guard_root(&self) -> PathBuf {
        self.data_base.join("attachment-sync").join("delete-guards")
    }

    pub fn shared_scope_path(&self, scope_id: &str) -> Result<PathBuf> {
        let root = if let Some(view_id) = scope_id.strip_prefix(SHARED_PREVIEW_SCOPE_PREFIX) {
            if !valid_cache_id(view_id) {
                return Err(Error::InvalidMetadata);
            }
            self.shared_preview_cache_root()
        } else {
            self.shared_cache_root()
        };
        Ok(root.join(hash_identifier(scope_id)))
    }

    fn cache_root(&self) -> PathBuf {
        self.cache_base.join("attachment-sync")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use uuid::Uuid;

    #[test]
    fn derives_platform_neutral_transfer_roots() {
        let paths = TransferPaths::new(Path::new("/cache"), Path::new("/data"));

        assert_eq!(
            paths.private_cache_root(),
            Path::new("/cache/attachment-sync/private")
        );
        assert_eq!(
            paths.shared_upload_cache_root(),
            Path::new("/cache/attachment-sync/shared-upload")
        );
        assert_eq!(
            paths.delete_guard_root(),
            Path::new("/data/attachment-sync/delete-guards")
        );
    }

    #[test]
    fn preview_scopes_are_isolated_from_durable_shared_cache() {
        let paths = TransferPaths::new("/cache", "/data");
        let preview_id = Uuid::new_v4();
        let preview_scope = format!("{SHARED_PREVIEW_SCOPE_PREFIX}{preview_id}");

        assert!(
            paths
                .shared_scope_path(&preview_scope)
                .unwrap()
                .starts_with(paths.shared_preview_cache_root())
        );
        assert!(
            paths
                .shared_scope_path("durable-viewer")
                .unwrap()
                .starts_with(paths.shared_cache_root())
        );
        assert!(paths.shared_scope_path("preview:not-a-uuid").is_err());
    }
}
