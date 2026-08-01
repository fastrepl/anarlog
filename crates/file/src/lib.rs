mod local;
mod remote;
mod types;

pub use local::*;
pub use remote::*;
pub use types::*;

use {
    anlg_download_interface::DownloadProgress,
    futures_util::{StreamExt, TryStreamExt, stream::FuturesUnordered},
    reqwest::StatusCode,
    std::{
        cmp::min,
        fs::File,
        fs::OpenOptions,
        io::{BufReader, Read, Seek, SeekFrom, Write},
        path::Path,
        sync::{Arc, Mutex, OnceLock},
    },
    tokio_util::sync::CancellationToken,
};

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn get_client() -> &'static reqwest::Client {
    CLIENT.get_or_init(reqwest::Client::new)
}

/// Makes a request with optional range header and returns the response.
/// This function can be used to test range request behavior.
pub async fn request_with_range(
    url: impl reqwest::IntoUrl,
    start_byte: Option<u64>,
) -> Result<reqwest::Response, Error> {
    let client = get_client();
    let url = url.into_url()?;

    let mut request = client.get(url);
    if let Some(start) = start_byte {
        request = request.header("Range", format!("bytes={}-", start));
    }

    let response = request.send().await?;
    Ok(response)
}

/// Validates if a partial file is suitable for resuming by checking:
/// 1. File size is aligned to a reasonable boundary (to detect incomplete writes)
/// 2. Optionally: Last few bytes can be read successfully
fn validate_partial_file(path: impl AsRef<Path>, size: u64) -> bool {
    // For empty files, don't try to resume
    if size == 0 {
        return false;
    }

    // Try to read the last few bytes to ensure the file isn't corrupted
    if let Ok(mut file) = File::open(path.as_ref()) {
        // Try to seek to near the end and read
        let test_size = min(512, size);
        let test_offset = size.saturating_sub(test_size);
        if file.seek(SeekFrom::Start(test_offset)).is_ok() {
            let mut buffer = vec![0u8; test_size as usize];
            if file.read_exact(&mut buffer).is_err() {
                // Can't read the end of the file properly, might be corrupted
                return false;
            }
        }
        true
    } else {
        false
    }
}

/// Downloads a file with resume capability. If the file already exists,
/// it will resume from where it left off using HTTP Range requests.
/// This is the preferred method for downloading large files that might
/// be interrupted.
pub async fn download_file_with_callback<F: Fn(DownloadProgress)>(
    url: impl reqwest::IntoUrl,
    output_path: impl AsRef<Path>,
    progress_callback: F,
) -> Result<(), crate::Error> {
    download_file_with_callback_cancellable(url, output_path, progress_callback, None).await
}

/// Downloads a file with resume capability and cancellation support.
/// If the file already exists, it will resume from where it left off using HTTP Range requests.
/// When cancelled, ensures any buffered data is written to disk before returning.
pub async fn download_file_with_callback_cancellable<F: Fn(DownloadProgress)>(
    url: impl reqwest::IntoUrl,
    output_path: impl AsRef<Path>,
    progress_callback: F,
    cancellation_token: Option<CancellationToken>,
) -> Result<(), crate::Error> {
    let url = url.into_url()?;

    if let Some(parent) = output_path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut existing_size = if output_path.as_ref().exists() {
        let size = file_size(&output_path)?;
        if validate_partial_file(&output_path, size) {
            size
        } else {
            std::fs::remove_file(output_path.as_ref())?;
            0
        }
    } else {
        0
    };

    let mut res = request_with_range(
        url.clone(),
        if existing_size > 0 {
            Some(existing_size)
        } else {
            None
        },
    )
    .await?;

    if !res.status().is_success() && res.status() != StatusCode::PARTIAL_CONTENT {
        return Err(crate::Error::OtherError(format!(
            "Download failed with status {}: {}",
            res.status(),
            url
        )));
    }

    // If we tried to resume but server doesn't support it, start fresh
    if existing_size > 0 && res.status() != StatusCode::PARTIAL_CONTENT {
        tracing::info!("Server doesn't support resume, starting fresh download");
        std::fs::remove_file(output_path.as_ref()).ok();
        existing_size = 0;
        res = request_with_range(url.clone(), None).await?;

        if !res.status().is_success() {
            return Err(crate::Error::OtherError(format!(
                "Download failed with status {}: {}",
                res.status(),
                url
            )));
        }
    }

    let total_size = get_content_length_from_headers(&res).map(|content_length| {
        if existing_size > 0 {
            existing_size + content_length
        } else {
            content_length
        }
    });

    // Use read+write mode for resuming (consistent with parallel download)
    let mut file = if existing_size > 0 {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(output_path.as_ref())?
    } else {
        std::fs::File::create(output_path.as_ref())?
    };

    // Seek to the end for resuming
    if existing_size > 0 {
        file.seek(SeekFrom::End(0))?;
    }

    let mut downloaded: u64 = existing_size;
    let mut stream = res.bytes_stream();

    progress_callback(DownloadProgress::Started);

    // Buffer writes to reduce syscalls
    let mut write_buffer = Vec::with_capacity(1024 * 1024); // 1MB buffer

    loop {
        // Check for cancellation
        if let Some(ref token) = cancellation_token
            && token.is_cancelled()
        {
            // Flush any buffered data before exiting
            if !write_buffer.is_empty() {
                file.write_all(&write_buffer)?;
                write_buffer.clear();
            }
            file.flush()?;
            file.sync_all()?;
            tracing::info!(
                "Download cancelled, partial file saved at: {:?}",
                output_path.as_ref()
            );
            return Err(crate::Error::Cancelled);
        }

        match stream.next().await {
            Some(Ok(chunk)) => {
                write_buffer.extend_from_slice(&chunk);

                // Write when buffer is large enough
                if write_buffer.len() >= 1024 * 1024 {
                    file.write_all(&write_buffer)?;
                    write_buffer.clear();
                }

                downloaded += chunk.len() as u64;
                progress_callback(DownloadProgress::Progress(
                    downloaded,
                    total_size.unwrap_or(downloaded),
                ));
            }
            Some(Err(e)) => {
                // On error, flush any buffered data
                if !write_buffer.is_empty() {
                    file.write_all(&write_buffer)?;
                }
                file.flush()?;
                file.sync_all()?;
                return Err(e.into());
            }
            None => break,
        }
    }

    // Write any remaining buffered data
    if !write_buffer.is_empty() {
        file.write_all(&write_buffer)?;
    }

    // Ensure all data is written to disk
    file.flush()?;
    file.sync_all()?;

    progress_callback(DownloadProgress::Finished);

    Ok(())
}

/// Process a chunk write with proper error handling and ordering
fn process_task_result(
    result: Result<(u64, Vec<u8>), Error>,
    file: &Arc<Mutex<File>>,
    pending_writes: &Arc<Mutex<std::collections::BTreeMap<u64, Vec<u8>>>>,
    next_write_offset: &Arc<Mutex<u64>>,
) -> Result<(), Error> {
    match result {
        Ok((offset, data)) => {
            let mut pending = pending_writes.lock().unwrap();
            pending.insert(offset, data);

            // Try to write consecutive chunks
            let mut next_offset = next_write_offset.lock().unwrap();
            let mut file = file.lock().unwrap();

            while let Some(data) = pending.remove(&*next_offset) {
                file.seek(SeekFrom::Start(*next_offset))?;
                file.write_all(&data)?;
                *next_offset += data.len() as u64;
            }

            // Only flush periodically, not after every write
            if pending.is_empty() {
                file.flush()?;
            }

            Ok(())
        }
        Err(e) => Err(e),
    }
}

const DEFAULT_CHUNK_SIZE: u64 = 8 * 1024 * 1024;
const MAX_CONCURRENT_CHUNKS: usize = 8;

pub async fn download_file_parallel<F: Fn(DownloadProgress) + Send + Sync>(
    url: impl reqwest::IntoUrl,
    output_path: impl AsRef<Path>,
    progress_callback: F,
) -> Result<(), Error> {
    download_file_parallel_cancellable(url, output_path, progress_callback, None).await
}

/// Downloads a file in parallel chunks with cancellation support.
/// When cancelled, ensures all downloaded data is properly written to disk.
pub async fn download_file_parallel_cancellable<F: Fn(DownloadProgress) + Send + Sync>(
    url: impl reqwest::IntoUrl,
    output_path: impl AsRef<Path>,
    progress_callback: F,
    cancellation_token: Option<CancellationToken>,
) -> Result<(), Error> {
    let url = url.into_url()?;
    let progress_callback = Arc::new(progress_callback);

    if let Some(parent) = output_path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }

    let head_response = get_client().head(url.clone()).send().await?;

    // Check if the resource exists before attempting download
    if !head_response.status().is_success() {
        return Err(crate::Error::OtherError(format!(
            "Resource not found or inaccessible (status {}): {}",
            head_response.status(),
            url
        )));
    }

    let total_size = get_content_length_from_headers(&head_response);

    let supports_ranges = head_response
        .headers()
        .get("accept-ranges")
        .map(|v| v.to_str().unwrap_or(""))
        .unwrap_or("")
        == "bytes";

    // Fall back to sequential download if ranges not supported or file is small
    if !supports_ranges || total_size.unwrap_or(0) <= DEFAULT_CHUNK_SIZE {
        return download_file_with_callback_cancellable(
            url,
            output_path,
            move |progress| progress_callback(progress),
            cancellation_token,
        )
        .await;
    }

    let total_size = total_size.unwrap();

    let existing_size = if output_path.as_ref().exists() {
        let size = file_size(&output_path)?;
        // Validate existing file and truncate to last complete chunk boundary
        if validate_partial_file(&output_path, size) {
            // Round down to nearest chunk boundary to ensure we re-download any partial chunk
            let chunk_boundary = (size / (1024 * 1024)) * (1024 * 1024);
            if chunk_boundary < size {
                // Truncate file to last complete chunk
                let file = OpenOptions::new().write(true).open(output_path.as_ref())?;
                file.set_len(chunk_boundary)?;
                tracing::info!("Truncated file from {} to {} bytes", size, chunk_boundary);
                chunk_boundary
            } else {
                size
            }
        } else {
            tracing::warn!("Existing file appears corrupted, starting fresh download");
            std::fs::remove_file(output_path.as_ref())?;
            0
        }
    } else {
        0
    };

    if existing_size >= total_size {
        progress_callback(DownloadProgress::Finished);
        return Ok(());
    }

    let remaining_size = total_size - existing_size;
    let chunk_size = min(
        DEFAULT_CHUNK_SIZE,
        remaining_size / MAX_CONCURRENT_CHUNKS as u64,
    )
    .max(1024 * 1024);
    let num_chunks = remaining_size.div_ceil(chunk_size);

    let file = if existing_size > 0 {
        Arc::new(Mutex::new(
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(output_path.as_ref())?,
        ))
    } else {
        Arc::new(Mutex::new(File::create(output_path.as_ref())?))
    };

    let downloaded = Arc::new(Mutex::new(existing_size));
    let pending_writes = Arc::new(Mutex::new(std::collections::BTreeMap::new()));
    let next_write_offset = Arc::new(Mutex::new(existing_size));
    let mut tasks = FuturesUnordered::new();

    progress_callback(DownloadProgress::Started);

    for chunk_idx in 0..num_chunks {
        // Check for cancellation before starting new chunks
        if let Some(ref token) = cancellation_token
            && token.is_cancelled()
        {
            // Process any remaining tasks and flush data
            while let Some(result) = tasks.next().await {
                let _ = process_task_result(result, &file, &pending_writes, &next_write_offset);
            }

            // Ensure all pending writes are flushed
            {
                let mut file_guard = file.lock().unwrap();
                file_guard.flush()?;
                file_guard.sync_all()?;
            }

            tracing::info!(
                "Download cancelled, partial file saved at: {:?}",
                output_path.as_ref()
            );
            return Err(crate::Error::Cancelled);
        }

        let start = existing_size + chunk_idx * chunk_size;
        let end = min(start + chunk_size - 1, total_size - 1);

        let url_clone = url.clone();
        let downloaded_clone = Arc::clone(&downloaded);
        let progress_callback_clone = Arc::clone(&progress_callback);
        let cancellation_token_clone = cancellation_token.clone();

        let task = async move {
            // Check cancellation at chunk level
            if let Some(ref token) = cancellation_token_clone
                && token.is_cancelled()
            {
                return Err(crate::Error::Cancelled);
            }

            let client = get_client();
            let range_header = format!("bytes={}-{}", start, end);

            let response = client
                .get(url_clone)
                .header("Range", range_header)
                .send()
                .await?;

            if response.status() != StatusCode::PARTIAL_CONTENT {
                return Err(crate::Error::OtherError(format!(
                    "Server didn't return partial content (status: {})",
                    response.status()
                )));
            }

            let mut bytes = Vec::new();
            let mut stream = response.bytes_stream();

            while let Some(chunk) = stream.try_next().await? {
                // Check cancellation during chunk download
                if let Some(ref token) = cancellation_token_clone
                    && token.is_cancelled()
                {
                    return Ok((start, bytes)); // Return what we have so far
                }

                bytes.extend_from_slice(&chunk);

                let mut downloaded_guard = downloaded_clone.lock().unwrap();
                *downloaded_guard += chunk.len() as u64;
                let current_downloaded = *downloaded_guard;
                drop(downloaded_guard);

                progress_callback_clone(DownloadProgress::Progress(current_downloaded, total_size));
            }

            Ok((start, bytes))
        };

        tasks.push(task);

        if tasks.len() >= MAX_CONCURRENT_CHUNKS
            && let Some(result) = tasks.next().await
        {
            process_task_result(result, &file, &pending_writes, &next_write_offset)?;
        }
    }

    while let Some(result) = tasks.next().await {
        // If we get a cancellation error, still try to process the result
        // as it might contain partial data
        if let Err(Error::Cancelled) = &result {
            // Process any data that was downloaded before cancellation
            if let Ok((offset, data)) = result {
                let _ = process_task_result(
                    Ok((offset, data)),
                    &file,
                    &pending_writes,
                    &next_write_offset,
                );
            }
        } else {
            process_task_result(result, &file, &pending_writes, &next_write_offset)?;
        }
    }

    // Final sync to ensure all data is on disk
    {
        let mut file_guard = file.lock().unwrap();
        file_guard.flush()?;
        file_guard.sync_all()?;
    }

    progress_callback(DownloadProgress::Finished);

    Ok(())
}

pub fn file_size(path: impl AsRef<Path>) -> Result<u64, Error> {
    let metadata = std::fs::metadata(path.as_ref())?;
    Ok(metadata.len())
}

/// Manually parse content-length header from HTTP response
/// This is a workaround for cases where reqwest's content_length() method returns incorrect values
fn get_content_length_from_headers(response: &reqwest::Response) -> Option<u64> {
    response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .or_else(|| response.content_length())
}

pub fn calculate_file_checksum(path: impl AsRef<Path>) -> Result<u32, Error> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = crc32fast::Hasher::new();

    let mut buffer = [0; 65536]; // 64KB buffer

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            // eof
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hasher.finalize())
}

#[cfg(test)]
mod tests;
