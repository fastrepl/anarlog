use std::path::{Path, PathBuf};

use swift_rs::{Bool, SRData, SRString, swift};

use crate::{
    DiarizationSegment, Error, FileTranscript, LivePartial, ModelDownloadState, Result,
    SoniqoModel, TranscriptSource,
};

swift!(fn _soniqo_model_cache_dir(model_id: &SRString) -> SRString);
swift!(fn _soniqo_diarization_cache_dir() -> SRString);
swift!(fn _soniqo_model_download_state(model_id: &SRString) -> SRString);
swift!(fn _soniqo_model_start_download(model_id: &SRString) -> Bool);
swift!(fn _soniqo_model_reset(model_id: &SRString) -> Bool);
swift!(fn _soniqo_transcribe_audio_file(
    model_id: &SRString,
    audio_path: &SRString,
    language: &SRString
) -> SRString);
swift!(fn _soniqo_diarize_audio(
    model_id: &SRString,
    samples: &SRData,
    exact_speakers: &SRString
) -> SRString);
swift!(fn _soniqo_live_start(model_id: &SRString) -> SRString);
swift!(fn _soniqo_live_append(
    session_token: &SRString,
    source: &SRString,
    samples: &SRData
) -> SRString);
swift!(fn _soniqo_live_finalize(session_token: &SRString, source: &SRString) -> SRString);
swift!(fn _soniqo_live_stop(session_token: &SRString) -> SRString);

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileTranscriptionPayload {
    text: String,
    duration_seconds: f64,
    error: Option<String>,
}

#[derive(serde::Deserialize)]
struct LiveAppendPayload {
    partials: Vec<LivePartial>,
    error: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiarizationPayload {
    segments: Vec<DiarizationSegment>,
    num_speakers: usize,
    error: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatusPayload {
    running: bool,
    session_token: Option<String>,
    error: Option<String>,
}

pub(crate) fn model_cache_dir(model: SoniqoModel) -> Result<PathBuf> {
    let model_id = sr_string(model.as_str());
    let path = unsafe { _soniqo_model_cache_dir(&model_id) };
    let path = path.as_str().to_string();

    if path.is_empty() {
        return Err(Error::Bridge(format!(
            "cache path unavailable for {}",
            model.as_str()
        )));
    }

    Ok(PathBuf::from(path))
}

pub(crate) fn diarization_cache_dir() -> Result<PathBuf> {
    let path = unsafe { _soniqo_diarization_cache_dir() };
    let path = path.as_str().to_string();

    if path.is_empty() {
        return Err(Error::Bridge(
            "Soniqo diarization cache path unavailable".to_string(),
        ));
    }

    Ok(PathBuf::from(path))
}

pub(crate) fn model_download_state(model: SoniqoModel) -> Result<ModelDownloadState> {
    let model_id = sr_string(model.as_str());
    let payload = unsafe { _soniqo_model_download_state(&model_id) };
    let state: ModelDownloadState = serde_json::from_str(payload.as_str())?;

    Ok(state)
}

pub(crate) fn start_model_download(model: SoniqoModel) -> Result<()> {
    let model_id = sr_string(model.as_str());
    if unsafe { _soniqo_model_start_download(&model_id) } {
        Ok(())
    } else {
        Err(Error::Bridge(format!(
            "failed to start Soniqo download for {}",
            model.as_str()
        )))
    }
}

pub(crate) fn reset_model(model: SoniqoModel) -> Result<()> {
    let model_id = sr_string(model.as_str());
    if unsafe { _soniqo_model_reset(&model_id) } {
        Ok(())
    } else {
        Err(Error::Bridge(format!(
            "failed to reset Soniqo model {}",
            model.as_str()
        )))
    }
}

pub(crate) fn transcribe_file(
    model: SoniqoModel,
    path: &Path,
    language: &str,
) -> Result<FileTranscript> {
    let model_id = sr_string(model.as_str());
    let audio_path = sr_string(&path.to_string_lossy());
    let language = sr_string(language);
    let payload = unsafe { _soniqo_transcribe_audio_file(&model_id, &audio_path, &language) };
    let result: FileTranscriptionPayload = serde_json::from_str(payload.as_str())?;

    if let Some(error) = result.error {
        return Err(Error::Bridge(error));
    }

    Ok(FileTranscript {
        text: result.text,
        duration_seconds: result.duration_seconds,
        chunks: Vec::new(),
        speaker_segments: Vec::new(),
    })
}

pub(crate) fn diarize_samples(
    model: SoniqoModel,
    samples: &[f32],
    exact_speakers: usize,
) -> Result<Vec<DiarizationSegment>> {
    let model_id = sr_string(model.as_str());
    let samples = floats_to_sr_data(samples);
    let exact_speakers_value = sr_string(&exact_speakers.to_string());
    let payload = unsafe { _soniqo_diarize_audio(&model_id, &samples, &exact_speakers_value) };
    let result: DiarizationPayload = serde_json::from_str(payload.as_str())?;

    if let Some(error) = result.error {
        return Err(Error::Bridge(error));
    }
    if result.num_speakers != exact_speakers {
        return Err(Error::Bridge(format!(
            "Soniqo diarization returned {} speakers instead of {}",
            result.num_speakers, exact_speakers
        )));
    }

    Ok(result.segments)
}

pub(crate) fn live_start(model: SoniqoModel) -> Result<String> {
    let model_id = sr_string(model.as_str());
    let payload = unsafe { _soniqo_live_start(&model_id) };
    let result: StatusPayload = serde_json::from_str(payload.as_str())?;

    live_start_result(result)
}

fn live_start_result(result: StatusPayload) -> Result<String> {
    if result.running {
        result.session_token.ok_or_else(|| {
            Error::Bridge("Soniqo live session started without a session token".to_string())
        })
    } else {
        Err(Error::Bridge(result.error.unwrap_or_else(|| {
            "failed to start Soniqo live session".to_string()
        })))
    }
}

pub(crate) fn live_append(
    session_token: &str,
    source: TranscriptSource,
    samples: &[f32],
) -> Result<Vec<LivePartial>> {
    let session_token = sr_string(session_token);
    let source = sr_string(source.as_str());
    let samples = floats_to_sr_data(samples);
    let payload = unsafe { _soniqo_live_append(&session_token, &source, &samples) };
    let result: LiveAppendPayload = serde_json::from_str(payload.as_str())?;

    if let Some(error) = result.error {
        return Err(Error::Bridge(error));
    }

    Ok(result.partials)
}

pub(crate) fn live_finalize(
    session_token: &str,
    source: TranscriptSource,
) -> Result<Vec<LivePartial>> {
    let session_token = sr_string(session_token);
    let source = sr_string(source.as_str());
    let payload = unsafe { _soniqo_live_finalize(&session_token, &source) };
    let result: LiveAppendPayload = serde_json::from_str(payload.as_str())?;

    if let Some(error) = result.error {
        return Err(Error::Bridge(error));
    }

    Ok(result.partials)
}

pub(crate) fn live_stop(session_token: &str) -> Result<()> {
    let session_token = sr_string(session_token);
    let payload = unsafe { _soniqo_live_stop(&session_token) };
    let result: StatusPayload = serde_json::from_str(payload.as_str())?;

    if let Some(error) = result.error {
        return Err(Error::Bridge(error));
    }

    Ok(())
}

fn sr_string(value: &str) -> SRString {
    SRString::from(value)
}

fn floats_to_sr_data(samples: &[f32]) -> SRData {
    let bytes = samples
        .iter()
        .flat_map(|sample| sample.to_bits().to_le_bytes())
        .collect::<Vec<_>>();
    SRData::from(bytes.as_slice())
}

#[cfg(test)]
mod tests {
    use super::{StatusPayload, live_start_result};

    #[test]
    fn live_start_requires_session_token() {
        let result = live_start_result(StatusPayload {
            running: true,
            session_token: None,
            error: None,
        });

        assert_eq!(
            result.unwrap_err().to_string(),
            "Soniqo bridge failed: Soniqo live session started without a session token"
        );
    }

    #[test]
    fn live_start_returns_session_token() {
        let token = live_start_result(StatusPayload {
            running: true,
            session_token: Some("42".to_string()),
            error: None,
        })
        .unwrap();

        assert_eq!(token, "42");
    }

    #[test]
    fn live_start_decodes_swift_camel_case_token() {
        let payload: StatusPayload =
            serde_json::from_str(r#"{"running":true,"sessionToken":"42","error":null}"#).unwrap();

        assert_eq!(live_start_result(payload).unwrap(), "42");
    }
}
