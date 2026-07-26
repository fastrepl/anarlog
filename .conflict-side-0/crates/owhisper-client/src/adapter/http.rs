use std::path::Path;

use reqwest::Response;
use reqwest::multipart::Part;

use crate::error::Error;

pub async fn ensure_success(response: Response) -> Result<Response, Error> {
    let status = response.status();
    if status.is_success() {
        Ok(response)
    } else {
        let body = response.text().await.unwrap_or_default();
        Err(Error::UnexpectedStatus { status, body })
    }
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

pub async fn streaming_file_body(file_path: &Path) -> Result<(reqwest::Body, u64), Error> {
    let file = tokio::fs::File::open(file_path)
        .await
        .map_err(|e| Error::AudioProcessing(e.to_string()))?;
    let length = file
        .metadata()
        .await
        .map_err(|e| Error::AudioProcessing(e.to_string()))?
        .len();

    Ok((reqwest::Body::from(file), length))
}

pub async fn streaming_file_part(file_path: &Path) -> Result<Part, Error> {
    let fallback_name = match file_path.extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("audio.{}", ext),
        None => "audio".to_string(),
    };

    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or(fallback_name);

    let (body, length) = streaming_file_body(file_path).await?;
    let mime_type = mime_type_from_extension(file_path);

    Part::stream_with_length(body, length)
        .file_name(file_name)
        .mime_str(mime_type)
        .map_err(|e| Error::AudioProcessing(e.to_string()))
}
