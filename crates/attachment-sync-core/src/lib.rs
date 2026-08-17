mod cache;
mod control;
mod download;
mod error;
mod layout;
mod private_transfer;

pub use cache::{
    CacheFileGuard, MAX_RANGE_BYTES, SharedUploadCacheFileGuard, cached_shared_attachment_path,
    cleanup_shared_upload_path, clear_attachment_cache_directory, commit_shared_cache_entry,
    create_shared_upload_cache_root, ensure_file_size, file_matches, file_matches_async,
    file_matches_cancellable, file_matches_cancellable_async, hash_identifier, hex_digest,
    private_cache_path, read_range, read_shared_cache_metadata, shared_cache_id,
    shared_cache_matches, shared_cache_matches_async, shared_cache_metadata_path,
    shared_upload_cache_path, snapshot_verified_file, sync_destination_directory, valid_cache_id,
    valid_sha256, validate_range, write_shared_cache_metadata,
};
pub use control::{DownloadControl, DownloadOperation, SharedScopeClear, SharedScopePrefixClear};
pub use download::{
    DownloadObject, download_to_path, persist_staged_attachment, require_configured_supabase_url,
    stage_attachment_restore, validate_signed_download_url,
};
pub use error::{Error, Result};
pub use layout::{SHARED_PREVIEW_SCOPE_PREFIX, TransferPaths};
pub use private_transfer::seal_attachment_to_cache;

pub const MAX_PLAINTEXT_BYTES: u64 = anlg_e2ee::ATTACHMENT_BLOB_MAX_PLAINTEXT_BYTES;
