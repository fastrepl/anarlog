use std::path::PathBuf;

use anlg_e2ee::{
    AttachmentBlobContext, AttachmentBlobMetadata, AttachmentBlobPlaintextMetadata, WorkspaceKey,
};

use crate::{CacheFileGuard, Error, Result};

pub async fn seal_attachment_to_cache(
    key: WorkspaceKey,
    context: AttachmentBlobContext,
    source_path: PathBuf,
    cache_path: PathBuf,
    expected: AttachmentBlobPlaintextMetadata,
) -> Result<(AttachmentBlobMetadata, CacheFileGuard)> {
    tokio::task::spawn_blocking(move || {
        let mut source = std::fs::File::open(source_path)?;
        let mut destination = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&cache_path)?;
        let cache_guard = CacheFileGuard::new(cache_path);
        let metadata =
            key.seal_attachment_blob(&context, &mut source, &mut destination, &expected)?;
        destination.sync_all()?;
        Ok((metadata, cache_guard))
    })
    .await
    .map_err(|_| Error::CacheUnavailable)?
}
