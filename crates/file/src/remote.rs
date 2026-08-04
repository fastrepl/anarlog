use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use futures_util::{Stream, StreamExt, TryStreamExt};
use reqwest::header::CONTENT_LENGTH;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

const CHUNK_SIZE: u64 = 60 * 1024 * 1024;
const STREAM_BUFFER_SIZE: usize = 64 * 1024;
const MAX_CONCURRENT_UPLOAD_PARTS: usize = 2;
const MAX_ERROR_BODY_BYTES: usize = 64 * 1024;

pub async fn upload(
    presigned_urls: Vec<String>,
    local_path: std::path::PathBuf,
) -> Result<Vec<String>, crate::Error> {
    let file = tokio::fs::File::open(&local_path).await?;
    let file_size = file.metadata().await?.len();

    let client = reqwest::Client::new();
    futures_util::stream::iter(presigned_urls.into_iter().enumerate().map(
        |(chunk_index, presigned_url)| {
            let local_path = local_path.clone();
            let client = client.clone();
            async move {
                let start = (chunk_index as u64)
                    .checked_mul(CHUNK_SIZE)
                    .ok_or_else(|| {
                        crate::Error::OtherError("multipart upload range overflow".to_string())
                    })?;
                if start >= file_size {
                    return Err(crate::Error::OtherError(
                        "multipart upload has more parts than the file".to_string(),
                    ));
                }
                let length = CHUNK_SIZE.min(file_size - start);
                upload_part(&client, &presigned_url, &local_path, start, length).await
            }
        },
    ))
    .buffered(MAX_CONCURRENT_UPLOAD_PARTS)
    .try_collect()
    .await
}

async fn upload_part(
    client: &reqwest::Client,
    presigned_url: &str,
    local_path: &std::path::Path,
    start: u64,
    length: u64,
) -> Result<String, crate::Error> {
    let checksum_b64 = checksum_file_range(local_path, start, length).await?;
    let mut file = tokio::fs::File::open(local_path).await?;
    file.seek(std::io::SeekFrom::Start(start)).await?;
    let body = reqwest::Body::wrap_stream(stream_file_range(file, length));

    let response = client
        .put(presigned_url)
        .header(CONTENT_LENGTH, length.to_string())
        .header("x-amz-checksum-algorithm", "CRC32")
        .header("x-amz-checksum-crc32", checksum_b64)
        .body(body)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(crate::Error::OtherError(read_error_body(response).await?));
    }

    response
        .headers()
        .get("ETag")
        .ok_or_else(|| crate::Error::OtherError("upload response missing ETag".to_string()))?
        .to_str()
        .map(str::to_string)
        .map_err(|error| crate::Error::OtherError(format!("invalid upload ETag: {error}")))
}

async fn read_error_body(response: reqwest::Response) -> Result<String, reqwest::Error> {
    let (body, truncated) =
        collect_body_prefix(response.bytes_stream(), MAX_ERROR_BODY_BYTES).await?;
    let mut message = String::from_utf8_lossy(&body).into_owned();
    if truncated {
        message.push_str(&format!(
            "\n[response body truncated to {MAX_ERROR_BODY_BYTES} bytes]"
        ));
    }
    Ok(message)
}

async fn collect_body_prefix<B, E>(
    stream: impl Stream<Item = Result<B, E>>,
    limit: usize,
) -> Result<(Vec<u8>, bool), E>
where
    B: AsRef<[u8]>,
{
    futures_util::pin_mut!(stream);
    let mut body = Vec::with_capacity(limit);

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let chunk = chunk.as_ref();
        let remaining = limit.saturating_sub(body.len());
        let copied = remaining.min(chunk.len());
        body.extend_from_slice(&chunk[..copied]);
        if copied < chunk.len() {
            return Ok((body, true));
        }
    }

    Ok((body, false))
}

async fn checksum_file_range(
    local_path: &std::path::Path,
    start: u64,
    length: u64,
) -> Result<String, std::io::Error> {
    let mut file = tokio::fs::File::open(local_path).await?;
    file.seek(std::io::SeekFrom::Start(start)).await?;

    let mut hasher = crc32fast::Hasher::new();
    let mut remaining = length;
    let mut buffer = vec![0; STREAM_BUFFER_SIZE];
    while remaining > 0 {
        let read_len = (remaining as usize).min(buffer.len());
        file.read_exact(&mut buffer[..read_len]).await?;
        hasher.update(&buffer[..read_len]);
        remaining -= read_len as u64;
    }

    Ok(BASE64.encode(hasher.finalize().to_be_bytes()))
}

fn stream_file_range(
    file: tokio::fs::File,
    remaining: u64,
) -> impl Stream<Item = Result<Vec<u8>, std::io::Error>> + Send {
    futures_util::stream::try_unfold((file, remaining), |(mut file, remaining)| async move {
        if remaining == 0 {
            return Ok(None);
        }

        let read_len = (remaining as usize).min(STREAM_BUFFER_SIZE);
        let mut buffer = vec![0; read_len];
        file.read_exact(&mut buffer).await?;
        Ok(Some((buffer, (file, remaining - read_len as u64))))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use futures_util::{StreamExt, pin_mut};
    use std::io::Write;
    use testcontainers_modules::{minio, testcontainers::runners::AsyncRunner};

    #[tokio::test]
    async fn file_range_stream_reads_exact_bounded_chunks() {
        let mut temp_file = tempfile::NamedTempFile::new().unwrap();
        let data = (0..STREAM_BUFFER_SIZE * 2 + 7)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        temp_file.write_all(&data).unwrap();

        let start = 13_u64;
        let length = (STREAM_BUFFER_SIZE + 19) as u64;
        let mut file = tokio::fs::File::open(temp_file.path()).await.unwrap();
        file.seek(std::io::SeekFrom::Start(start)).await.unwrap();
        let stream = stream_file_range(file, length);
        pin_mut!(stream);

        let mut actual = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.unwrap();
            assert!(chunk.len() <= STREAM_BUFFER_SIZE);
            actual.extend(chunk);
        }

        assert_eq!(
            actual,
            data[start as usize..start as usize + length as usize]
        );
    }

    #[tokio::test]
    async fn error_body_prefix_is_capped_and_marks_truncation() {
        let stream = futures_util::stream::iter([
            Ok::<_, std::convert::Infallible>(vec![b'a'; MAX_ERROR_BODY_BYTES - 1]),
            Ok(vec![b'b'; 2]),
        ]);

        let (body, truncated) = collect_body_prefix(stream, MAX_ERROR_BODY_BYTES)
            .await
            .unwrap();

        assert_eq!(body.len(), MAX_ERROR_BODY_BYTES);
        assert_eq!(body.last(), Some(&b'b'));
        assert!(truncated);
    }

    #[tokio::test]
    #[ignore]
    async fn test_upload() {
        let container = minio::MinIO::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(9000).await.unwrap();

        let admin_s3 = anlg_s3::Client::builder()
            .endpoint_url(format!("http://127.0.0.1:{}", port))
            .bucket("test")
            .credentials("minioadmin", "minioadmin")
            .build()
            .await;

        let _ = admin_s3.create_bucket().await.unwrap();

        let user_s3 = admin_s3.for_user("test-user");

        let file_key = "audio.wav";
        let mut temp_file = tempfile::NamedTempFile::new().unwrap();
        let test_data = vec![0u8; 120 * 1024 * 1024];
        temp_file.write_all(&test_data).unwrap();

        let upload_id = user_s3.create_multipart_upload(file_key).await.unwrap();
        let presigned_urls = user_s3
            .presigned_url_for_multipart_upload(file_key, &upload_id, 2)
            .await
            .unwrap();
        assert!(presigned_urls.len() == 2);

        let etags = upload(presigned_urls, temp_file.into_temp_path().to_path_buf())
            .await
            .unwrap();
        assert!(etags.len() == 2);

        // let _ = user_s3
        //     .complete_multipart_upload(file_key, &upload_id, etags)
        //     .await
        //     .unwrap();
    }
}
