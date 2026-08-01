use std::io::Write;
use std::path::Path;
use std::time::Duration;

use anlg_e2ee::{AttachmentBlobContext, AttachmentBlobMetadata, WorkspaceKey};
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use super::cache::{hex_digest, sync_destination_directory};
use crate::error::{Error, Result};

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60);

pub(super) enum DownloadObject<'a> {
    Private(&'a str),
    Shared(&'a str),
}

pub(super) fn require_configured_supabase_url(value: Option<&str>) -> Result<&str> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or(Error::InvalidDownloadUrl)
}

pub(super) fn validate_signed_download_url(
    signed_url: &str,
    supabase_url: &str,
    object: DownloadObject<'_>,
) -> Result<url::Url> {
    let base = url::Url::parse(supabase_url).map_err(|_| Error::InvalidDownloadUrl)?;
    let signed = url::Url::parse(signed_url).map_err(|_| Error::InvalidDownloadUrl)?;
    if !base.username().is_empty()
        || base.password().is_some()
        || base.query().is_some()
        || base.fragment().is_some()
        || !matches!(base.path(), "" | "/")
        || !signed.username().is_empty()
        || signed.password().is_some()
        || signed.fragment().is_some()
        || base.scheme() != signed.scheme()
        || base.host_str() != signed.host_str()
        || base.port_or_known_default() != signed.port_or_known_default()
        || !secure_or_local(&base)
    {
        return Err(Error::InvalidDownloadUrl);
    }
    let mut query = signed.query_pairs();
    if !query
        .next()
        .is_some_and(|(name, value)| name == "token" && !value.is_empty())
        || query.next().is_some()
    {
        return Err(Error::InvalidDownloadUrl);
    }
    let valid_path = match object {
        DownloadObject::Private(object_key) => {
            signed.path() == format!("/storage/v1/object/sign/attachment-backups/{object_key}")
        }
        DownloadObject::Shared(attachment_id) => {
            valid_shared_attachment_download_path(signed.path(), attachment_id)
        }
    };
    if !valid_path {
        return Err(Error::InvalidDownloadUrl);
    }
    Ok(signed)
}

fn valid_shared_attachment_download_path(path: &str, attachment_id: &str) -> bool {
    let Some(object_key) = path.strip_prefix("/storage/v1/object/sign/shared-note-attachments/")
    else {
        return false;
    };
    let mut parts = object_key.split('/');
    let owner = parts.next();
    let share = parts.next();
    let filename = parts.next();
    parts.next().is_none()
        && owner.is_some_and(valid_canonical_uuid)
        && share.is_some_and(valid_canonical_uuid)
        && filename.is_some_and(|filename| filename == format!("{attachment_id}.sna1"))
}

fn valid_canonical_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok_and(|uuid| uuid.to_string() == value)
}

fn secure_or_local(url: &url::Url) -> bool {
    if url.scheme() == "https" {
        return true;
    }
    if url.scheme() != "http" {
        return false;
    }
    match url.host() {
        Some(url::Host::Domain("localhost")) => true,
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        _ => false,
    }
}

pub(super) async fn download_to_path(
    url: url::Url,
    path: &Path,
    expected_size: u64,
    expected_sha256: &str,
    create_new: bool,
    cancellation: &tokio_util::sync::CancellationToken,
) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(DOWNLOAD_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(Error::Download)?;
    let response = tokio::select! {
        _ = cancellation.cancelled() => return Err(Error::Cancelled),
        response = client
            .get(url)
            .header(reqwest::header::ACCEPT_ENCODING, "identity")
            .send() => response.map_err(Error::Download)?,
    };
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|size| size != expected_size)
        || response
            .headers()
            .get(reqwest::header::CONTENT_ENCODING)
            .is_some_and(|value| value != "identity")
    {
        return Err(Error::IncompleteDownload);
    }

    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).truncate(!create_new);
    if create_new {
        options.create_new(true);
    }
    let mut file = options.open(path).await?;
    let mut stream = response.bytes_stream();
    let mut size = 0_u64;
    let mut hasher = Sha256::new();
    loop {
        let next = tokio::select! {
            _ = cancellation.cancelled() => return Err(Error::Cancelled),
            next = stream.next() => next,
        };
        let Some(chunk) = next else {
            break;
        };
        let chunk = chunk.map_err(Error::Download)?;
        size = size
            .checked_add(chunk.len() as u64)
            .ok_or(Error::IncompleteDownload)?;
        if size > expected_size {
            return Err(Error::IncompleteDownload);
        }
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
    }
    tokio::select! {
        _ = cancellation.cancelled() => return Err(Error::Cancelled),
        result = file.sync_all() => result?,
    }
    if size != expected_size || hex_digest(hasher.finalize().as_slice()) != expected_sha256 {
        return Err(Error::ChecksumMismatch);
    }
    Ok(())
}

pub(super) fn stage_attachment_restore(
    key: &WorkspaceKey,
    context: &AttachmentBlobContext,
    cache_path: &Path,
    destination_parent: &Path,
    expected: &AttachmentBlobMetadata,
) -> Result<tempfile::NamedTempFile> {
    std::fs::create_dir_all(destination_parent)?;
    let mut source = std::fs::File::open(cache_path)?;
    let mut temp = tempfile::NamedTempFile::new_in(destination_parent)?;
    key.open_attachment_blob(context, &mut source, &mut temp, expected)?;
    temp.flush()?;
    temp.as_file().sync_all()?;
    Ok(temp)
}

pub(super) fn persist_staged_attachment(
    staged: tempfile::NamedTempFile,
    destination: &Path,
) -> Result<()> {
    let parent = destination
        .parent()
        .ok_or(Error::LocalAttachmentUnavailable)?;
    staged
        .persist(destination)
        .map(|_| ())
        .map_err(|error| Error::Io(error.error))?;
    sync_destination_directory(parent)
}
