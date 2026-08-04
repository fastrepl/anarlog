use std::path::Path;

use reqwest::Response;
use reqwest::multipart::Part;

use crate::error::Error;

const MAX_ERROR_BODY_BYTES: usize = 64 * 1024;

pub async fn ensure_success(response: Response) -> Result<Response, Error> {
    let status = response.status();
    if status.is_success() {
        Ok(response)
    } else {
        let body = error_body(response).await;
        Err(Error::UnexpectedStatus { status, body })
    }
}

pub async fn error_body(response: Response) -> String {
    error_body_with_limit(response, MAX_ERROR_BODY_BYTES).await
}

async fn error_body_with_limit(mut response: Response, limit: usize) -> String {
    let mut body = Vec::with_capacity(
        response
            .content_length()
            .and_then(|length| usize::try_from(length).ok())
            .unwrap_or_default()
            .min(limit),
    );
    let mut truncated = response
        .content_length()
        .is_some_and(|length| length > limit as u64);

    while let Ok(Some(chunk)) = response.chunk().await {
        let remaining = limit.saturating_sub(body.len());
        let keep = remaining.min(chunk.len());
        body.extend_from_slice(&chunk[..keep]);
        if keep < chunk.len() {
            truncated = true;
            break;
        }
        if body.len() == limit {
            if let Ok(Some(_)) = response.chunk().await {
                truncated = true;
            }
            break;
        }
    }

    let mut body = String::from_utf8_lossy(&body).into_owned();
    if truncated {
        body.push_str("\n[response body truncated]");
    }
    body
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

#[cfg(test)]
mod tests {
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers::path};

    use super::error_body_with_limit;

    #[tokio::test]
    async fn error_body_is_capped_and_marked() {
        let server = MockServer::start().await;
        Mock::given(path("/error"))
            .respond_with(ResponseTemplate::new(500).set_body_bytes(vec![b'x'; 17]))
            .mount(&server)
            .await;
        let response = reqwest::get(format!("{}/error", server.uri()))
            .await
            .unwrap();

        let body = error_body_with_limit(response, 16).await;
        assert_eq!(
            body,
            format!("{}\n[response body truncated]", "x".repeat(16))
        );
    }
}
