use std::io::Read;
use std::path::Path;

use owhisper_interface::{ListenParams, Word2};
use sha2::{Digest, Sha256};

pub fn process_recorded(
    model_path: impl AsRef<std::path::Path>,
    audio_path: impl AsRef<std::path::Path>,
) -> Result<Vec<Word2>, crate::Error> {
    process_recorded_with_params(model_path, audio_path, &ListenParams::default())
}

pub fn process_recorded_with_params(
    model_path: impl AsRef<std::path::Path>,
    audio_path: impl AsRef<std::path::Path>,
    params: &ListenParams,
) -> Result<Vec<Word2>, crate::Error> {
    let model = super::load_model(model_path.as_ref())?;
    super::batch::transcribe_recorded_file_with_params(
        &model,
        model_path.as_ref(),
        audio_path.as_ref(),
        params,
        None,
    )
}

/// Transcribe a recorded file while durably checkpointing completed audio chunks.
/// The checkpoint is private local state and is removed only after all chunks finish.
pub fn process_recorded_resumable(
    model_path: impl AsRef<Path>,
    audio_path: impl AsRef<Path>,
    params: &ListenParams,
) -> Result<Vec<Word2>, crate::Error> {
    let model_path = model_path.as_ref();
    let audio_path = audio_path.as_ref();
    let checkpoint_path = audio_path.with_file_name(format!(
        "{}.anlg-transcription-checkpoint.jsonl",
        audio_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("recording")
    ));
    let audio_fingerprint = file_fingerprint(audio_path)?;
    let model_metadata = std::fs::metadata(model_path)?;
    let model_fingerprint = format!(
        "{}:{}",
        model_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default(),
        model_metadata.len()
    );
    let mut stable_params = params.clone();
    stable_params.custom_query = None;
    let params_fingerprint = fingerprint(&serde_json::to_vec(&stable_params)?);
    let model = super::load_model(model_path)?;
    let mut checkpoint = super::batch::RecordedCheckpointJournal::open(
        &checkpoint_path,
        &audio_fingerprint,
        &model_fingerprint,
        &params_fingerprint,
        8,
    )?;
    super::batch::transcribe_recorded_file_with_params(
        &model,
        model_path,
        audio_path,
        params,
        Some(&mut checkpoint),
    )
}

fn file_fingerprint(path: &Path) -> Result<String, crate::Error> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hexadecimal(hasher.finalize().as_slice()))
}

fn fingerprint(bytes: &[u8]) -> String {
    hexadecimal(Sha256::digest(bytes).as_slice())
}

fn hexadecimal(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
