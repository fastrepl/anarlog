use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};
use uuid::{Uuid, Version};

use super::MAX_PLAINTEXT_BYTES;
use crate::error::{Error, Result};

pub const MAX_RANGE_BYTES: u64 = 6 * 1024 * 1024;

pub struct CacheFileGuard {
    path: Option<PathBuf>,
}

impl CacheFileGuard {
    pub fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    pub fn disarm(mut self) {
        self.path = None;
    }
}

impl Drop for CacheFileGuard {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub struct SharedUploadCacheFileGuard {
    path: Option<PathBuf>,
}

impl SharedUploadCacheFileGuard {
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    pub fn disarm(mut self) {
        self.path = None;
    }
}

impl Drop for SharedUploadCacheFileGuard {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = cleanup_shared_upload_path(&path);
        }
    }
}

pub fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

pub fn valid_cache_id(value: &str) -> bool {
    Uuid::parse_str(value)
        .is_ok_and(|uuid| uuid.to_string() == value && uuid.get_version() == Some(Version::Random))
}

pub fn validate_range(start: u64, end: u64) -> Result<()> {
    if end <= start || end - start > MAX_RANGE_BYTES {
        return Err(Error::InvalidRange);
    }
    Ok(())
}

pub async fn read_range(
    path: PathBuf,
    start: u64,
    end: u64,
    expected_size: u64,
) -> Result<Vec<u8>> {
    tokio::task::spawn_blocking(move || {
        let mut file = std::fs::File::open(path)?;
        if file.metadata()?.len() != expected_size {
            return Err(Error::CacheUnavailable);
        }
        file.seek(SeekFrom::Start(start))?;
        let length = usize::try_from(end - start).map_err(|_| Error::InvalidRange)?;
        let mut bytes = vec![0_u8; length];
        file.read_exact(&mut bytes)?;
        Ok(bytes)
    })
    .await
    .map_err(|_| Error::CacheUnavailable)?
}

pub fn ensure_file_size(path: &Path, expected_size: u64) -> Result<()> {
    if std::fs::metadata(path)?.len() != expected_size {
        return Err(Error::LocalAttachmentUnavailable);
    }
    Ok(())
}

pub fn file_matches(path: &Path, expected_size: u64, expected_sha256: &str) -> Result<bool> {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() || metadata.len() != expected_size {
        return Ok(false);
    }
    Ok(hash_file(path)? == expected_sha256)
}

pub async fn file_matches_async(
    path: PathBuf,
    expected_size: u64,
    expected_sha256: String,
) -> Result<bool> {
    tokio::task::spawn_blocking(move || file_matches(&path, expected_size, &expected_sha256))
        .await
        .map_err(|_| Error::CacheUnavailable)?
}

pub async fn file_matches_cancellable_async(
    path: PathBuf,
    expected_size: u64,
    expected_sha256: String,
    cancellation: tokio_util::sync::CancellationToken,
) -> Result<bool> {
    tokio::task::spawn_blocking(move || {
        file_matches_cancellable(&path, expected_size, &expected_sha256, &cancellation)
    })
    .await
    .map_err(|_| Error::CacheUnavailable)?
}

pub fn file_matches_cancellable(
    path: &Path,
    expected_size: u64,
    expected_sha256: &str,
    cancellation: &tokio_util::sync::CancellationToken,
) -> Result<bool> {
    if cancellation.is_cancelled() {
        return Err(Error::Cancelled);
    }
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() || metadata.len() != expected_size {
        return Ok(false);
    }
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    if cancellation.is_cancelled() {
        return Err(Error::Cancelled);
    }
    Ok(hex_digest(hasher.finalize().as_slice()) == expected_sha256)
}

fn hash_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_digest(hasher.finalize().as_slice()))
}

pub fn snapshot_verified_file(
    source_path: &Path,
    cache_path: &Path,
    expected_size: u64,
    expected_sha256: &str,
    cancellation: &tokio_util::sync::CancellationToken,
) -> Result<SharedUploadCacheFileGuard> {
    if cancellation.is_cancelled() {
        return Err(Error::Cancelled);
    }
    let mut source = std::fs::File::open(source_path)?;
    let source_metadata = source.metadata()?;
    if !source_metadata.is_file() || source_metadata.len() != expected_size {
        return Err(Error::LocalAttachmentUnavailable);
    }
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let cache_guard = SharedUploadCacheFileGuard::new(cache_path.to_path_buf());
    let mut cache = options.open(cache_path)?;
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let read = source.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        total = total
            .checked_add(read as u64)
            .ok_or(Error::ChecksumMismatch)?;
        if total > expected_size {
            return Err(Error::ChecksumMismatch);
        }
        hasher.update(&buffer[..read]);
        cache.write_all(&buffer[..read])?;
    }
    if cancellation.is_cancelled() {
        return Err(Error::Cancelled);
    }
    cache.sync_all()?;
    if total != expected_size || hex_digest(hasher.finalize().as_slice()) != expected_sha256 {
        return Err(Error::ChecksumMismatch);
    }
    Ok(cache_guard)
}

pub fn private_cache_path(root: &Path, cache_id: &str) -> Result<PathBuf> {
    if !valid_cache_id(cache_id) {
        return Err(Error::InvalidTransferState);
    }
    Ok(root.join(format!("{cache_id}.anb1")))
}

pub async fn create_shared_upload_cache_root(path: &Path) -> Result<()> {
    tokio::fs::create_dir_all(path).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).await?;
    }
    Ok(())
}

pub fn shared_upload_cache_path(root: &Path, cache_id: &str) -> Result<PathBuf> {
    if !valid_cache_id(cache_id) {
        return Err(Error::InvalidTransferState);
    }
    Ok(root.join(format!("{cache_id}.bin")))
}

pub fn cleanup_shared_upload_path(path: &Path) -> Result<bool> {
    let mut last_error = None;
    for delay_ms in [0, 50, 250, 1_000] {
        if delay_ms > 0 {
            std::thread::sleep(Duration::from_millis(delay_ms));
        }
        match std::fs::remove_file(path) {
            Ok(()) => return Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.ok_or(Error::CacheUnavailable)?.into())
}

pub fn clear_attachment_cache_directory(path: &Path) -> Result<()> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub fn cached_shared_attachment_path(scope_path: &Path, cache_id: &str) -> PathBuf {
    scope_path.join(format!("{cache_id}.bin"))
}

pub fn shared_cache_metadata_path(scope_path: &Path, cache_id: &str) -> PathBuf {
    scope_path.join(format!("{cache_id}.meta"))
}

pub fn write_shared_cache_metadata(
    scope_path: &Path,
    cache_id: &str,
    size_bytes: u64,
    sha256: &str,
) -> Result<()> {
    let path = shared_cache_metadata_path(scope_path, cache_id);
    let mut temp = tempfile::NamedTempFile::new_in(scope_path)?;
    write!(temp, "{size_bytes}\n{sha256}\n")?;
    temp.flush()?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|error| Error::Io(error.error))?;
    Ok(())
}

pub fn commit_shared_cache_entry(
    scope_path: &Path,
    cache_id: &str,
    data_path: &Path,
    size_bytes: u64,
    sha256: &str,
) -> Result<()> {
    let data_guard = CacheFileGuard::new(data_path.to_path_buf());
    write_shared_cache_metadata(scope_path, cache_id, size_bytes, sha256)?;
    data_guard.disarm();
    Ok(())
}

pub fn read_shared_cache_metadata(
    scope_path: &Path,
    cache_id: &str,
) -> Result<Option<(u64, String)>> {
    let path = shared_cache_metadata_path(scope_path, cache_id);
    let metadata = match std::fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() || metadata.len() > 128 {
        return Ok(None);
    }
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut lines = content.lines();
    let size = lines
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value <= MAX_PLAINTEXT_BYTES);
    let sha256 = lines.next().filter(|value| valid_sha256(value));
    if lines.next().is_some() || size.is_none() || sha256.is_none() {
        return Ok(None);
    }
    Ok(Some((size.unwrap(), sha256.unwrap().to_string())))
}

pub fn shared_cache_matches(
    scope_path: &Path,
    cache_id: &str,
    expected_size: u64,
    expected_sha256: &str,
) -> Result<bool> {
    let Some((recorded_size, recorded_sha256)) = read_shared_cache_metadata(scope_path, cache_id)?
    else {
        return Ok(false);
    };
    if recorded_size != expected_size || recorded_sha256 != expected_sha256 {
        return Ok(false);
    }
    file_matches(
        &cached_shared_attachment_path(scope_path, cache_id),
        expected_size,
        expected_sha256,
    )
}

pub async fn shared_cache_matches_async(
    scope_path: PathBuf,
    cache_id: String,
    expected_size: u64,
    expected_sha256: String,
) -> Result<bool> {
    tokio::task::spawn_blocking(move || {
        shared_cache_matches(&scope_path, &cache_id, expected_size, &expected_sha256)
    })
    .await
    .map_err(|_| Error::CacheUnavailable)?
}

pub fn hash_identifier(value: &str) -> String {
    hex_digest(Sha256::digest(value.as_bytes()).as_slice())
}

pub fn shared_cache_id(scope_id: &str, attachment_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update((scope_id.len() as u64).to_be_bytes());
    hasher.update(scope_id.as_bytes());
    hasher.update((attachment_id.len() as u64).to_be_bytes());
    hasher.update(attachment_id.as_bytes());
    hex_digest(hasher.finalize().as_slice())
}

pub fn hex_digest(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut value, byte| {
            write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
            value
        })
}

#[cfg(unix)]
pub fn sync_destination_directory(path: &Path) -> Result<()> {
    std::fs::File::open(path)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
pub fn sync_destination_directory(_path: &Path) -> Result<()> {
    Ok(())
}
