use std::path::Path;

use futures_util::StreamExt;
use reqwest::{StatusCode, header};
use tokio::io::AsyncWriteExt;

use crate::{Error, ResolvedRelease, Result, UpdateChecker, verify_signature};

impl UpdateChecker {
    /// Saves partial bytes across process exits and only publishes a verified installer.
    /// The caller must serialize downloads and cleanup for this destination.
    pub async fn download_to_path(
        &self,
        release: &ResolvedRelease,
        destination: &Path,
        mut on_chunk: impl FnMut(u64, Option<u64>),
    ) -> Result<()> {
        let partial = destination.with_extension("part");
        let identity = destination.with_extension("signature");
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // A version can be republished with different bytes. Truncate before updating
        // the identity so a process exit cannot associate old bytes with a new signature.
        if tokio::fs::read_to_string(&identity).await.ok().as_deref()
            != Some(release.signature.as_str())
        {
            tokio::fs::File::create(&partial).await?;
            tokio::fs::write(&identity, &release.signature).await?;
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&partial)
            .await?;
        let mut offset = file.metadata().await?.len();
        let mut headers = self.config.headers.clone();
        headers
            .entry(header::ACCEPT)
            .or_insert(header::HeaderValue::from_static("application/octet-stream"));
        headers.insert(
            header::ACCEPT_ENCODING,
            header::HeaderValue::from_static("identity"),
        );
        headers.remove(header::RANGE);
        headers.remove(header::IF_RANGE);
        let client = self
            .client_builder()?
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .build()?;

        let response = loop {
            let mut request = client
                .get(release.download_url.clone())
                .headers(headers.clone());
            if offset > 0 {
                request = request.header(header::RANGE, format!("bytes={offset}-"));
            }
            let response = request.send().await?;
            if response.status() != StatusCode::RANGE_NOT_SATISFIABLE || offset == 0 {
                break response;
            }

            // The app may have quit after the last write but before verification.
            if verify_file(&partial, release, &self.config.pubkey)
                .await
                .is_ok()
            {
                on_chunk(offset, Some(offset));
                file.sync_all().await?;
                drop(file);
                tokio::fs::rename(&partial, destination).await?;
                let _ = tokio::fs::remove_file(&identity).await;
                return Ok(());
            }
            file.set_len(0).await?;
            offset = 0;
        };

        let total = match response.status() {
            StatusCode::OK => {
                file.set_len(0).await?;
                offset = 0;
                response.content_length()
            }
            StatusCode::PARTIAL_CONTENT => {
                let (start, end, total) = response
                    .headers()
                    .get(header::CONTENT_RANGE)
                    .and_then(|value| value.to_str().ok())
                    .and_then(parse_range)
                    .ok_or(Error::InvalidDownloadResponse)?;
                if start != offset
                    || end.checked_add(1) != Some(total)
                    || start > end
                    || response
                        .content_length()
                        .is_some_and(|length| length != total - start)
                {
                    return Err(Error::InvalidDownloadResponse);
                }
                Some(total)
            }
            status => return Err(Error::BadStatus(status)),
        };
        if response
            .headers()
            .get(header::CONTENT_ENCODING)
            .is_some_and(|encoding| encoding != "identity")
        {
            return Err(Error::InvalidDownloadResponse);
        }

        on_chunk(offset, total);
        let mut downloaded = offset;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            // Tokio writes can outlive write_all; report only bytes visible to a new process.
            file.flush().await?;
            downloaded += chunk.len() as u64;
            on_chunk(chunk.len() as u64, total);
        }
        file.sync_all().await?;
        drop(file);
        if total.is_some_and(|total| total != downloaded) {
            return Err(Error::InvalidDownloadResponse);
        }
        if let Err(error) = verify_file(&partial, release, &self.config.pubkey).await {
            tokio::fs::remove_file(&partial).await?;
            return Err(error);
        }
        tokio::fs::rename(&partial, destination).await?;
        let _ = tokio::fs::remove_file(&identity).await;
        Ok(())
    }
}

async fn verify_file(path: &Path, release: &ResolvedRelease, pubkey: &str) -> Result<()> {
    let path = path.to_owned();
    let signature = release.signature.clone();
    let pubkey = pubkey.to_owned();
    tokio::task::spawn_blocking(move || {
        verify_signature(&std::fs::read(path)?, &signature, &pubkey)
    })
    .await?
}

fn parse_range(value: &str) -> Option<(u64, u64, u64)> {
    let (range, total) = value.strip_prefix("bytes ")?.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    Some((start.parse().ok()?, end.parse().ok()?, total.parse().ok()?))
}
