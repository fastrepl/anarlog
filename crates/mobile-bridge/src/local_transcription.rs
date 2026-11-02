use std::path::PathBuf;

use anlg_local_model::LocalModel;
use anlg_whisper_local_model::WhisperModel as WhisperModelDefinition;
use owhisper_interface::{ListenParams, Word2};
use serde::{Deserialize, Serialize};

const MAX_AUDIO_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum LocalWhisperError {
    #[error("Check the local transcription settings.")]
    InvalidSettings,
    #[error("The recording could not be read.")]
    AudioMissing,
    #[error("This recording is too large for local transcription.")]
    AudioTooLarge,
    #[error("Download the Whisper Small model before transcribing offline.")]
    ModelMissing,
    #[error("The local model file did not pass its integrity check.")]
    ModelIntegrityFailed,
    #[error("Local transcription could not be completed.")]
    InferenceFailed,
    #[error("The local transcription result could not be encoded.")]
    SerializationFailed,
}

#[derive(Deserialize)]
struct Request {
    audio_uri: String,
    model_uri: String,
    #[serde(default)]
    params: ListenParams,
}

#[derive(Serialize)]
struct ModelInfo {
    model: String,
    url: String,
    uri: String,
    size_bytes: u64,
}

fn file_path(uri: &str) -> Option<PathBuf> {
    url::Url::parse(uri).ok()?.to_file_path().ok()
}

fn expected_model_path(uri: &str) -> Result<PathBuf, LocalWhisperError> {
    let path = file_path(uri).ok_or(LocalWhisperError::ModelMissing)?;
    if path.file_name().and_then(|name| name.to_str())
        != Some(WhisperModelDefinition::QuantizedSmall.file_name())
    {
        return Err(LocalWhisperError::ModelMissing);
    }
    Ok(path)
}

fn model_file_is_valid(path: &std::path::Path) -> bool {
    let model = WhisperModelDefinition::QuantizedSmall;
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() || metadata.len() != model.model_size_bytes() {
        return false;
    }
    anlg_file::calculate_file_checksum(path).is_ok_and(|checksum| checksum == model.checksum())
}

#[uniffi::export]
pub fn local_whisper_model_info(models_base_uri: String) -> Result<String, LocalWhisperError> {
    let models_base = file_path(&models_base_uri).ok_or(LocalWhisperError::InvalidSettings)?;
    let model = WhisperModelDefinition::QuantizedSmall;
    let destination = LocalModel::Whisper(model.clone()).install_path(&models_base);
    let uri = url::Url::from_file_path(&destination)
        .map_err(|_| LocalWhisperError::InvalidSettings)?
        .to_string();
    serde_json::to_string(&ModelInfo {
        model: model.display_name().to_string(),
        url: model.model_url().to_string(),
        uri,
        size_bytes: model.model_size_bytes(),
    })
    .map_err(|_| LocalWhisperError::SerializationFailed)
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn verify_local_whisper_model(model_uri: String) -> Result<bool, LocalWhisperError> {
    let path = expected_model_path(&model_uri)?;
    tokio::task::spawn_blocking(move || model_file_is_valid(&path))
        .await
        .map_err(|_| LocalWhisperError::InferenceFailed)
}

fn validate_request(
    request: Request,
) -> Result<(PathBuf, PathBuf, ListenParams), LocalWhisperError> {
    let audio_path = file_path(&request.audio_uri).ok_or(LocalWhisperError::AudioMissing)?;
    let model_path = expected_model_path(&request.model_uri)?;

    let audio_metadata =
        std::fs::metadata(&audio_path).map_err(|_| LocalWhisperError::AudioMissing)?;
    if !audio_metadata.is_file() || audio_metadata.len() == 0 {
        return Err(LocalWhisperError::AudioMissing);
    }
    if audio_metadata.len() > MAX_AUDIO_BYTES {
        return Err(LocalWhisperError::AudioTooLarge);
    }

    if !model_path.exists() {
        return Err(LocalWhisperError::ModelMissing);
    }
    validate_params(&request.params)?;
    Ok((audio_path, model_path, request.params))
}

fn validate_params(params: &ListenParams) -> Result<(), LocalWhisperError> {
    if params.languages.len() > 8
        || params.keywords.len() > 100
        || params
            .keywords
            .iter()
            .any(|keyword| keyword.trim().is_empty() || keyword.chars().count() > 100)
        || params.keywords.iter().map(String::len).sum::<usize>() > 4_000
    {
        return Err(LocalWhisperError::InvalidSettings);
    }
    Ok(())
}

// Local inference runs on a blocking worker so a long meeting never occupies
// the bridge's async runtime. Audio and model stay on disk and never leave the
// device through this entry point.
#[uniffi::export(async_runtime = "tokio")]
pub async fn transcribe_local_whisper(request_json: String) -> Result<String, LocalWhisperError> {
    let request: Request =
        serde_json::from_str(&request_json).map_err(|_| LocalWhisperError::InvalidSettings)?;
    let (audio_path, model_path, params) = validate_request(request)?;
    let words: Vec<Word2> = tokio::task::spawn_blocking(move || {
        if !model_file_is_valid(&model_path) {
            return Err(LocalWhisperError::ModelIntegrityFailed);
        }
        anlg_transcribe_whisper_local::process_recorded_resumable(model_path, audio_path, &params)
            .map_err(|_| LocalWhisperError::InferenceFailed)
    })
    .await
    .map_err(|_| LocalWhisperError::InferenceFailed)??;
    serde_json::to_string(&words).map_err(|_| LocalWhisperError::SerializationFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(audio_uri: &str, model_uri: &str) -> Request {
        Request {
            audio_uri: audio_uri.to_string(),
            model_uri: model_uri.to_string(),
            params: ListenParams::default(),
        }
    }

    #[test]
    fn local_transcription_rejects_a_model_with_an_unexpected_filename() {
        let error = validate_request(request(
            "file:///tmp/meeting.wav",
            "file:///tmp/ggml-small.en-q8_0.bin",
        ))
        .unwrap_err();
        assert!(matches!(error, LocalWhisperError::ModelMissing));
    }

    #[test]
    fn local_transcription_bounds_keyword_context() {
        let mut params = ListenParams::default();
        params.keywords = vec!["x".repeat(101)];
        assert!(matches!(
            validate_params(&params),
            Err(LocalWhisperError::InvalidSettings)
        ));
    }
}
