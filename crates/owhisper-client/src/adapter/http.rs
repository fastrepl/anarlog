use std::path::Path;

use reqwest::multipart::Part;
use reqwest::{Body, Response};
use tokio::fs::File;
use tokio_util::io::ReaderStream;

use crate::error::Error;

pub struct FileBody {
    pub body: Body,
    pub len: u64,
}

pub async fn ensure_success(response: Response) -> Result<Response, Error> {
    let status = response.status();
    if status.is_success() {
        Ok(response)
    } else {
        let body = response.text().await.unwrap_or_default();
        Err(Error::UnexpectedStatus { status, body })
    }
}

pub async fn streaming_file_body(path: &Path) -> Result<FileBody, Error> {
    let file = File::open(path)
        .await
        .map_err(|e| Error::AudioProcessing(format!("failed to open file: {e}")))?;
    let len = file
        .metadata()
        .await
        .map_err(|e| Error::AudioProcessing(format!("failed to stat file: {e}")))?
        .len();
    let stream = ReaderStream::new(file);

    Ok(FileBody {
        body: Body::wrap_stream(stream),
        len,
    })
}

pub async fn streaming_file_part(path: &Path) -> Result<Part, Error> {
    let fallback_name = match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("audio.{ext}"),
        None => "audio".to_string(),
    };
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or(fallback_name);
    let file = streaming_file_body(path).await?;

    Part::stream_with_length(file.body, file.len)
        .file_name(file_name)
        .mime_str(mime_type_from_extension(path))
        .map_err(|e| Error::AudioProcessing(e.to_string()))
}

pub fn mime_type_from_extension(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("mp3") => "audio/mpeg",
        Some("mp4") => "audio/mp4",
        Some("m4a") => "audio/mp4",
        Some("wav") => "audio/wav",
        Some("webm") => "audio/webm",
        Some("ogg") => "audio/ogg",
        Some("flac") => "audio/flac",
        _ => "application/octet-stream",
    }
}
