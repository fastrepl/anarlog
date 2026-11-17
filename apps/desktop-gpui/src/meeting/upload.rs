//! Shipping's audio and transcript file flows (`stt/useUploadFile.ts`, `stt/useRunBatch.ts`,
//! `transcript/actions.ts` and the audio player's `deleteRecording`). Uploaded or dropped audio
//! is imported into the session folder and batch-transcribed with the configured provider,
//! replacing the session's transcript; subtitle files import as a transcript; a recording can be
//! re-transcribed, shown in its folder or deleted.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use anlg_listener2_core::{BatchEvent, BatchParams, BatchRuntime, run_batch};
use anlg_transcript::{
    BatchRefinementOutcome, BatchRefinementRequest, BatchRefinementSource,
    BatchTranscriptPromotion, StoredLiveTranscriptDelta, StoredSpeakerHint, StoredTranscriptWord,
};
use desktop_runtime::{CancellationToken, Result, RuntimeHandle, ServiceError, SessionId};
use futures::{
    StreamExt,
    channel::mpsc,
    future::{BoxFuture, LocalBoxFuture},
};
use gpui::{
    App, AppContext, Bounds, Context, Entity, EventEmitter, ExternalPaths, Global, IntoElement,
    SharedString, Window, canvas, div, fill, point, prelude::*, px, rgba, size, svg,
};
use owhisper_interface::batch::Response;
use serde_json::{Value, json};

use super::config::{self, ProviderServices, normalize_keywords};
use super::model::{Retention, failure};
use super::playback::Command;
use super::retention::{Activities, session_directory};
use super::{MeetingServices, store::statement};
use crate::application::toasts;
use crate::platform::dialogs::{DialogAdapter, FileFilter, NativePicker, Picker};
use crate::ui::theme::{MEDIUM, Theme};

pub const AUDIO_EXTENSIONS: [&str; 8] = ["wav", "mp3", "ogg", "mp4", "m4a", "flac", "webm", "aac"];
const TRANSCRIPT_EXTENSIONS: [&str; 2] = ["vtt", "srt"];
const EMPTY_TRANSCRIPT: &str = "No speech was detected in the audio.";
const INCOMPLETE_TRANSCRIPT: &str = "The new transcription returned much less text. Your saved transcript and recording were kept. Try transcribing again.";
const NO_PROVIDER: &str =
    "STT connection is not available. Please configure your speech-to-text provider.";
const MISSING_RECORDING: &str = "Recording not found. It may have been deleted.";
const SYNTHETIC_TEXT_WORD_SECONDS: f64 = 0.4;
/// Shipping's `ChannelProfile.MixedCapture`, the channel of imported subtitle cues.
const MIXED_CAPTURE: f64 = 2.;

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

pub fn is_audio(path: &Path) -> bool {
    AUDIO_EXTENSIONS.contains(&extension(path).as_str())
}

/// Shipping's `hasSingleAudioUploadDrag`: exactly one audio file (QuickTime audio included).
pub fn single_audio(paths: &ExternalPaths) -> Option<PathBuf> {
    match paths.paths() {
        [path] if is_audio(path) || extension(path) == "qta" => Some(path.clone()),
        _ => None,
    }
}

fn is_transcript(path: &Path) -> bool {
    TRANSCRIPT_EXTENSIONS.contains(&extension(path).as_str())
}

pub type Progress = Arc<dyn Fn(f64) + Send + Sync>;

/// Receives shipping's "Using a batch transcription provider" description when the selected
/// provider cannot transcribe the file and the fallback runs instead.
pub type Warn = Arc<dyn Fn(String) + Send + Sync>;

pub struct Transcription {
    pub response: Response,
    pub provider: String,
    pub model: String,
}

/// Batch transcription of one session's audio file; a fake replaces the provider in tests.
pub type Transcriber = Arc<
    dyn Fn(
            SessionId,
            PathBuf,
            Progress,
            Warn,
            CancellationToken,
        ) -> BoxFuture<'static, Result<Transcription>>
        + Send
        + Sync,
>;

/// Overrides the configured provider (tests use a fake transcriber).
pub struct TranscriberGlobal(pub Transcriber);
impl Global for TranscriberGlobal {}

/// The file picker and folder reveal, behind an adapter so tests never open OS dialogs.
pub trait Files {
    fn pick(
        &self,
        request: Picker,
        cx: &mut App,
    ) -> LocalBoxFuture<'static, Result<Option<PathBuf>>>;
    fn open_folder(&self, path: &Path, cx: &mut App);
}

struct SystemFiles;

impl Files for SystemFiles {
    fn pick(
        &self,
        request: Picker,
        cx: &mut App,
    ) -> LocalBoxFuture<'static, Result<Option<PathBuf>>> {
        NativePicker.pick(request, cx)
    }

    fn open_folder(&self, path: &Path, cx: &mut App) {
        cx.open_with_system(path);
    }
}

pub struct FilesGlobal(pub Rc<dyn Files>);
impl Global for FilesGlobal {}

fn files(cx: &App) -> Rc<dyn Files> {
    cx.try_global::<FilesGlobal>()
        .map(|files| files.0.clone())
        .unwrap_or_else(|| Rc::new(SystemFiles))
}

/// The selected provider, falling back to Pro cloud or the on-device model as shipping's
/// `useRunBatch` does; session title, participants and dictionary terms are the keywords.
pub fn provider_transcriber(services: ProviderServices) -> Transcriber {
    Arc::new(
        move |session: SessionId,
              path: PathBuf,
              progress: Progress,
              warn: Warn,
              cancellation: CancellationToken|
              -> BoxFuture<'static, Result<Transcription>> {
            let services = services.clone();
            Box::pin(async move {
                let request = crate::dictation::transcribe::Request {
                    preferences: config::preferences(&services.runtime).await?,
                };
                let target = crate::dictation::transcribe::target(&services, &request).await?;
                if let Some(replaced) = &target.replaced {
                    warn(replaced.clone());
                }
                let (_, participants, mut keywords) =
                    services.session_hints(session.clone()).await?;
                keywords.extend(request.keywords());
                normalize_keywords(&mut keywords);
                let speakers = participants.iter().collect::<HashSet<_>>().len() as u32;
                let provider = target.provider.to_string();
                let model = target.model.clone();
                let params = BatchParams {
                    session_id: session.0.to_string(),
                    provider: target.provider,
                    file_path: path
                        .to_str()
                        .ok_or_else(|| failure("Audio path is not valid UTF-8."))?
                        .into(),
                    model: Some(target.model),
                    base_url: target.base_url,
                    api_key: target.api_key,
                    languages: request.languages(),
                    keywords,
                    num_speakers: (speakers > 1).then_some(speakers),
                    min_speakers: None,
                    max_speakers: None,
                    known_speakers: Vec::new(),
                };
                let batch = Arc::new(Batch {
                    cancellation: cancellation.clone(),
                    progress,
                });
                let output = tokio::select! {
                    _ = cancellation.cancelled() => return Err(ServiceError::Cancelled),
                    result = run_batch(batch, params) => result.map_err(failure)?,
                };
                Ok(Transcription {
                    response: output.response,
                    provider,
                    model,
                })
            })
        },
    )
}

struct Batch {
    cancellation: CancellationToken,
    progress: Progress,
}

impl BatchRuntime for Batch {
    fn emit(&self, event: BatchEvent) {
        if let BatchEvent::BatchResponseStreamed { event, .. } = event {
            (self.progress)(event.percentage());
        }
    }
    fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }
}

struct ImportProgress(Progress);

impl anlg_fs_sync_core::runtime::AudioImportRuntime for ImportProgress {
    fn emit(&self, event: anlg_fs_sync_core::runtime::AudioImportEvent) {
        if let anlg_fs_sync_core::runtime::AudioImportEvent::Progress { percentage, .. } = event {
            (self.0)(percentage);
        }
    }
}

/// Shipping's `fsSyncCommands.audioImport` then `catalogLocalSessionAudio`.
pub async fn import_audio(
    runtime: &RuntimeHandle,
    directory: PathBuf,
    session: SessionId,
    source: PathBuf,
    progress: Progress,
) -> Result<PathBuf> {
    let id = session.0.to_string();
    let target = directory.clone();
    let path = tokio::task::spawn_blocking(move || {
        anlg_fs_sync_core::audio::import_to_session(
            &ImportProgress(progress),
            &id,
            &target,
            &source,
        )
        .map_err(failure)
    })
    .await
    .map_err(failure)??;
    // Shipping keeps imported audio when cataloguing fails.
    let _ = catalog_audio(runtime, directory, session).await;
    Ok(path)
}

async fn catalog_audio(
    runtime: &RuntimeHandle,
    directory: PathBuf,
    session: SessionId,
) -> Result<()> {
    let metadata =
        tokio::task::spawn_blocking(move || anlg_fs_sync_core::audio::metadata(&directory))
            .await
            .map_err(failure)?
            .map_err(failure)?
            .ok_or_else(|| failure("audio_path_not_found"))?;
    runtime
        .submit(move |services| async move {
            let session = session.0.to_string();
            let attachment = format!("session-audio:{session}");
            let size = i64::try_from(metadata.size_bytes).map_err(failure)?;
            let mut transaction = services
                .db
                .pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(failure)?;
            let delete_job = uuid::Uuid::new_v4().to_string();
            anlg_db_app::enqueue_replaced_attachment_delete_by_id(
                &mut transaction,
                &delete_job,
                &session,
                &attachment,
                &metadata.sha256,
                size,
            )
            .await
            .map_err(failure)?;
            let updated = anlg_db_app::update_session_audio_attachment(
                &mut transaction,
                &metadata.filename,
                &metadata.content_type,
                size,
                &metadata.sha256,
                &attachment,
                &session,
            )
            .await
            .map_err(failure)?;
            let inserted = anlg_db_app::insert_session_audio_attachment(
                &mut transaction,
                &attachment,
                &metadata.filename,
                &metadata.content_type,
                size,
                &metadata.sha256,
                &session,
            )
            .await
            .map_err(failure)?;
            anlg_db_app::upsert_attachment_local_state_by_id(
                &mut transaction,
                &attachment,
                &session,
            )
            .await
            .map_err(failure)?;
            let upload_job = uuid::Uuid::new_v4().to_string();
            anlg_db_app::enqueue_attachment_upload_by_id(
                &mut transaction,
                &upload_job,
                &session,
                &attachment,
            )
            .await
            .map_err(failure)?;
            if updated + inserted != 1 {
                return Err(failure("audio session is unavailable"));
            }
            transaction.commit().await.map_err(failure)
        })?
        .receive()
        .await
}

/// Shipping's `applyEstimatedAudioNoteDate` (picked files: created or modified time less the
/// duration) and `applyDroppedAudioNoteDate` (dropped files: modified time). Notes linked to a
/// calendar event keep their date.
async fn date_note(runtime: &RuntimeHandle, session: SessionId, source: PathBuf, dropped: bool) {
    let date = tokio::task::spawn_blocking(move || {
        let metadata = anlg_fs_sync_core::audio::source_metadata(&source).ok()?;
        let anchor = if dropped {
            metadata.modified_at
        } else {
            metadata.created_at.or(metadata.modified_at)
        }?;
        let anchor = chrono::DateTime::parse_from_rfc3339(&anchor).ok()?;
        let duration = if dropped {
            0
        } else {
            metadata.duration_ms.unwrap_or(0)
        };
        let date = anchor - chrono::Duration::milliseconds(i64::try_from(duration).ok()?);
        Some(
            date.with_timezone(&chrono::Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        )
    })
    .await
    .ok()
    .flatten();
    let Some(date) = date else {
        return;
    };
    if let Ok(reply) = runtime.submit(move |services| async move {
        services.executor.execute(
            "UPDATE sessions SET created_at = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND deleted_at IS NULL AND locked = 0 AND trim(event_json) = ''".into(),
            vec![json!(date), json!(session)],
        ).await.map_err(failure)?;
        Ok(())
    }) {
        let _ = reply.receive().await;
    }
}

/// Shipping's `useRunBatch` with `promotion: whole_session`: the new words replace every
/// transcript in the session unless they would drop most of the saved text.
pub async fn transcribe(
    runtime: &RuntimeHandle,
    session: SessionId,
    audio: PathBuf,
    transcriber: Transcriber,
    progress: Progress,
    warn: Warn,
    cancellation: CancellationToken,
) -> Result<()> {
    let started_at = chrono::Utc::now();
    let output = transcriber(session.clone(), audio, progress, warn, cancellation.clone()).await?;
    if cancellation.is_cancelled() {
        return Err(ServiceError::Cancelled);
    }
    let (words, hints) = stored_words(&output.response, &output.provider);
    if words.is_empty() {
        return Err(failure(EMPTY_TRANSCRIPT));
    }
    let saved = runtime
        .submit(move |services| async move {
            save_whole_session(
                services.db.pool(),
                session,
                started_at,
                output.provider,
                output.model,
                words,
                hints,
            )
            .await
        })?
        .receive()
        .await?;
    if !saved {
        return Err(failure(INCOMPLETE_TRANSCRIPT));
    }
    Ok(())
}

/// The transcription plugin's `save_batch_transcript` for a whole-session promotion; false
/// when refinement refuses a truncated replacement.
async fn save_whole_session(
    pool: &sqlx::SqlitePool,
    session: SessionId,
    started_at: chrono::DateTime<chrono::Utc>,
    provider: String,
    model: String,
    words: Vec<StoredTranscriptWord>,
    hints: Vec<StoredSpeakerHint>,
) -> Result<bool> {
    let session = session.0.to_string();
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.map_err(failure)?;
    let previous = anlg_db_app::load_session_batch_transcripts(&mut transaction, &session)
        .await
        .map_err(failure)?
        .into_iter()
        .map(|row| {
            let (words, hints) = anlg_transcript::normalize_stored_transcript(
                &row.id,
                serde_json::from_str(&row.words_json).unwrap_or_default(),
                anlg_transcript::parse_stored_speaker_hints(&row.speaker_hints_json),
            );
            let deltas: Vec<StoredLiveTranscriptDelta> = row
                .pending_delta_jsons
                .iter()
                .filter_map(|delta| serde_json::from_str(delta).ok())
                .collect();
            let (mut words, mut speaker_hints) =
                anlg_transcript::materialize_live_transcript(words, hints, &deltas);
            anlg_transcript::normalize_legacy_batch_channels(
                &row.source,
                &row.provider,
                &mut words,
                &mut speaker_hints,
            );
            BatchRefinementSource {
                id: row.id,
                started_at: row.started_at_ms as f64,
                words,
                speaker_hints,
            }
        })
        .collect();
    let refined = anlg_transcript::refine_batch_transcript(BatchRefinementRequest {
        words,
        hints,
        promotion: BatchTranscriptPromotion::WholeSession,
        previous_transcripts: previous,
    });
    let BatchRefinementOutcome::Ready {
        words,
        speaker_hints,
        replace_session,
        replace_transcript_id,
        started_at: refined_started_at,
    } = refined
    else {
        transaction.rollback().await.map_err(failure)?;
        return Ok(false);
    };
    if !words.is_empty() {
        let insert = anlg_db_app::BatchTranscriptInsert {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session.clone(),
            owner_user_id: String::new(),
            created_at: started_at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            started_at_ms: refined_started_at.unwrap_or(started_at.timestamp_millis() as f64),
            memo: String::new(),
            provider,
            model,
            words_json: anlg_transcript::serialize_batch_transcript_words(&words)
                .map_err(failure)?,
            speaker_hints_json: anlg_transcript::serialize_batch_transcript_hints(&speaker_hints)
                .map_err(failure)?,
        };
        anlg_db_app::insert_batch_transcript(
            &mut transaction,
            &insert,
            &anlg_db_app::BatchTranscriptReplace {
                replace_session,
                replace_transcript_id,
            },
        )
        .await
        .map_err(failure)?;
    }
    if replace_session {
        anlg_db_app::clear_incomplete_capture_markers(&mut transaction, &session)
            .await
            .map_err(failure)?;
    }
    anlg_db_app::mark_session_audio_transcription_complete(&mut transaction, &session)
        .await
        .map_err(failure)?;
    transaction.commit().await.map_err(failure)?;
    Ok(true)
}

/// Shipping's `getBatchResponseTimingSource` and its per-channel override.
fn timing_source(response: &Response, channel: usize) -> Option<String> {
    let metadata = &response.metadata;
    metadata["timing_sources_by_channel"][channel]
        .as_str()
        .or_else(|| metadata["timing"]["source"].as_str())
        .or_else(|| metadata["timing_source"].as_str())
        .filter(|source| {
            matches!(
                *source,
                "provider_word" | "provider_segment_interpolated" | "synthetic_text"
            )
        })
        .map(Into::into)
}

/// Shipping's `transformBatch`: each channel's first alternative, provider words spaced as in the
/// transcript (or evenly timed tokens when a provider returns only text), and a
/// `provider_speaker_index` hint per diarized word.
fn stored_words(
    response: &Response,
    provider: &str,
) -> (Vec<StoredTranscriptWord>, Vec<StoredSpeakerHint>) {
    let mut words = Vec::new();
    let mut hints = Vec::new();
    for (index, channel) in response.results.channels.iter().enumerate() {
        let Some(alternative) = channel.alternatives.first() else {
            continue;
        };
        let transcript = alternative.transcript.as_str();
        let entries: Vec<(String, f64, f64, i32, Option<usize>)> = if alternative.words.is_empty() {
            let tokens: Vec<&str> = transcript.split_whitespace().collect();
            tokens
                .iter()
                .enumerate()
                .map(|(position, token)| {
                    let start = position as f64 * SYNTHETIC_TEXT_WORD_SECONDS;
                    (
                        (*token).to_owned(),
                        start,
                        start + SYNTHETIC_TEXT_WORD_SECONDS,
                        index as i32,
                        None,
                    )
                })
                .collect()
        } else {
            alternative
                .words
                .iter()
                .map(|word| {
                    (
                        word.punctuated_word
                            .clone()
                            .unwrap_or_else(|| word.word.clone()),
                        word.start,
                        word.end,
                        word.channel,
                        word.speaker,
                    )
                })
                .collect()
        };
        let source = timing_source(response, index).unwrap_or_else(|| {
            if alternative.words.is_empty() {
                "synthetic_text".into()
            } else {
                "provider_word".into()
            }
        });
        let texts = spaced(
            entries.iter().map(|entry| entry.0.as_str()).collect(),
            transcript,
        );
        for ((_, start, end, channel, speaker), text) in entries.into_iter().zip(texts) {
            let id = uuid::Uuid::new_v4().to_string();
            if let Some(speaker) = speaker {
                hints.push(StoredSpeakerHint {
                    id: uuid::Uuid::new_v4().to_string(),
                    word_id: Some(id.clone()),
                    hint_type: "provider_speaker_index".into(),
                    value: json!(
                        json!({"provider": provider, "channel": channel, "speaker_index": speaker})
                            .to_string()
                    ),
                });
            }
            words.push(StoredTranscriptWord {
                id,
                text: Some(text),
                start_ms: Some((start * 1000.).round()),
                end_ms: Some((end * 1000.).round()),
                channel: Some(f64::from(channel)),
                speaker: None,
                metadata: Some(json!(json!({"timing": {"source": source}}).to_string())),
            });
        }
    }
    (words, hints)
}

/// Shipping's `fixSpacingForWords`: each word keeps the whitespace before it in the transcript.
fn spaced(words: Vec<&str>, transcript: &str) -> Vec<String> {
    let mut position = 0;
    words
        .into_iter()
        .enumerate()
        .map(|(index, word)| {
            let trimmed = word.trim();
            if trimmed.is_empty() {
                return word.to_owned();
            }
            let Some(found) = transcript[position..].find(trimmed).map(|at| at + position) else {
                return word.to_owned();
            };
            let prefix = if index == 0 {
                " "
            } else {
                &transcript[position..found]
            };
            position = found + trimmed.len();
            format!("{prefix}{trimmed}")
        })
        .collect()
}

/// Shipping's subtitle upload: each cue becomes a word on the mixed channel of a new
/// `subtitle_import` transcript. Returns the cue count; an empty file adds nothing.
pub async fn import_subtitle(
    runtime: &RuntimeHandle,
    session: SessionId,
    path: PathBuf,
) -> Result<usize> {
    let subtitle = tokio::task::spawn_blocking(move || {
        anlg_listener2_core::parse_subtitle_from_path(&path).map_err(failure)
    })
    .await
    .map_err(failure)??;
    let subtitle = serde_json::to_value(&subtitle).map_err(failure)?;
    let tokens = subtitle["tokens"].as_array().cloned().unwrap_or_default();
    if tokens.is_empty() {
        return Ok(0);
    }
    let transcript = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();
    let created_at = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let words: Vec<Value> = tokens
        .iter()
        .map(|token| {
            json!({
                "id": uuid::Uuid::new_v4().to_string(),
                "transcript_id": transcript,
                "text": token["text"],
                "start_ms": token["start_time"],
                "end_ms": token["end_time"],
                "channel": MIXED_CAPTURE as i64,
                "user_id": "",
                "created_at": created_at,
            })
        })
        .collect();
    let count = words.len();
    let words = serde_json::to_string(&words).map_err(failure)?;
    runtime
        .submit(move |services| async move {
            services.executor.execute_transaction(vec![statement(
                "INSERT INTO transcripts (id, workspace_id, owner_user_id, session_id, source, provider, model, language, started_at_ms, ended_at_ms, audio_attachment_id, memo, words_json, speaker_hints_json, metadata_json, created_at, updated_at, deleted_at)
                 SELECT ?, session.workspace_id, session.owner_user_id, session.id, 'subtitle_import', '', '', '', ?, NULL, '', '', ?, '[]', '{}', ?, ?, NULL
                 FROM sessions AS session WHERE session.id = ? AND session.deleted_at IS NULL AND session.locked = 0",
                vec![json!(transcript), json!(now.timestamp_millis()), json!(words), json!(created_at), json!(created_at), json!(session)],
                Some(1),
            )]).await.map_err(failure)?;
            Ok(count)
        })?
        .receive()
        .await
}

/// Shipping's `deleteSessionAudio`: tombstones the catalogued audio, deletes the files and
/// marks the recording absent on this device.
pub async fn delete_audio(
    runtime: &RuntimeHandle,
    directory: PathBuf,
    session: SessionId,
) -> Result<()> {
    let id = session.0.to_string();
    runtime
        .submit(move |services| async move {
            let mut transaction = services
                .db
                .pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(failure)?;
            anlg_db_app::tombstone_session_audio_attachment(
                &mut transaction,
                &format!("session-audio:{id}"),
                &id,
            )
            .await
            .map_err(failure)?;
            transaction.commit().await.map_err(failure)
        })?
        .receive()
        .await?;
    tokio::task::spawn_blocking(move || {
        anlg_listener_core::actors::recorder::delete_capture_audio(&directory).map_err(failure)
    })
    .await
    .map_err(failure)??;
    runtime
        .submit(move |services| async move {
            anlg_db_app::mark_session_audio_absent(services.db.pool(), &session.0)
                .await
                .map_err(failure)
        })?
        .receive()
        .await
}

/// Shipping's `deleteProcessedAudioForRetention`: with "Don't save audio", the transcribed
/// recording is deleted.
async fn apply_retention(
    runtime: &RuntimeHandle,
    directory: PathBuf,
    session: SessionId,
) -> Result<bool> {
    let preferences = config::preferences(runtime).await?;
    if !matches!(
        Retention::from_setting(preferences.value("audio_retention")),
        Ok(Retention::Never)
    ) {
        return Ok(false);
    }
    delete_audio(runtime, directory, session).await?;
    Ok(true)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Importing,
    Transcribing,
}

/// A session's batch state (shipping's listener `batch[sessionId]`).
#[derive(Clone, Debug, PartialEq)]
pub enum Job {
    Running { phase: Phase, percentage: f64 },
    Failed(SharedString),
}

pub enum UploadEvent {
    /// The session's saved recording appeared or went away.
    AudioChanged(SessionId),
}

enum Update {
    Phase(Phase),
    Progress(f64),
    AudioChanged,
    Warning(String),
}

struct Run {
    id: u64,
    cancellation: CancellationToken,
    /// Shipping toasts a failed re-transcription; a failed upload only shows in the transcript.
    retranscribe: bool,
}

/// Every session's upload, transcription and deletion state, shared by the note's transcript,
/// header and menus.
#[derive(Default)]
pub struct Uploads {
    jobs: HashMap<SessionId, Job>,
    runs: HashMap<SessionId, Run>,
    deleting: HashSet<SessionId>,
    next_run: u64,
}

struct UploadsGlobal(Entity<Uploads>);
impl Global for UploadsGlobal {}

impl EventEmitter<UploadEvent> for Uploads {}

/// What a run needs from the started meeting services.
struct Host {
    runtime: RuntimeHandle,
    directory: PathBuf,
    activities: Activities,
    transcriber: Transcriber,
}

impl Host {
    fn new(runtime: &RuntimeHandle, session: &SessionId, cx: &App) -> Result<Self> {
        let services = cx
            .try_global::<MeetingServices>()
            .ok_or_else(|| failure("Meetings are still starting. Try again in a moment."))?;
        let transcriber = match cx.try_global::<TranscriberGlobal>() {
            Some(global) => global.0.clone(),
            None => provider_transcriber(
                cx.try_global::<ProviderServices>()
                    .ok_or_else(|| failure(NO_PROVIDER))?
                    .clone(),
            ),
        };
        Ok(Self {
            runtime: runtime.clone(),
            directory: session_directory(&services.vault, session)?,
            activities: services.capture.activities.clone(),
            transcriber,
        })
    }
}

impl Uploads {
    pub fn global(cx: &mut App) -> Entity<Self> {
        if let Some(global) = cx.try_global::<UploadsGlobal>() {
            return global.0.clone();
        }
        let uploads = cx.new(|_| Self::default());
        cx.set_global(UploadsGlobal(uploads.clone()));
        uploads
    }

    pub fn job(&self, session: &SessionId) -> Option<&Job> {
        self.jobs.get(session)
    }

    /// Shipping's `running_batch` session mode.
    pub fn running(&self, session: &SessionId) -> bool {
        matches!(self.jobs.get(session), Some(Job::Running { .. }))
    }

    pub fn deleting(&self, session: &SessionId) -> bool {
        self.deleting.contains(session)
    }

    /// Shipping's `stopTranscription`: the transcript returns to what it showed before.
    pub fn stop(&mut self, session: &SessionId, cx: &mut Context<Self>) {
        if let Some(run) = self.runs.remove(session) {
            run.cancellation.cancel();
            self.jobs.remove(session);
            cx.notify();
        }
    }

    pub fn upload_audio(
        &mut self,
        runtime: &RuntimeHandle,
        session: SessionId,
        cx: &mut Context<Self>,
    ) {
        self.pick(runtime, session, true, cx);
    }

    pub fn upload_transcript(
        &mut self,
        runtime: &RuntimeHandle,
        session: SessionId,
        cx: &mut Context<Self>,
    ) {
        self.pick(runtime, session, false, cx);
    }

    fn pick(
        &mut self,
        runtime: &RuntimeHandle,
        session: SessionId,
        audio: bool,
        cx: &mut Context<Self>,
    ) {
        let (label, extensions, prompt) = if audio {
            ("Audio", &AUDIO_EXTENSIONS[..], "Upload Audio")
        } else {
            (
                "Transcript",
                &TRANSCRIPT_EXTENSIONS[..],
                "Upload Transcript",
            )
        };
        let request = Picker {
            directory: false,
            initial_directory: std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join("Downloads")),
            filters: vec![FileFilter {
                label: label.into(),
                extensions: extensions
                    .iter()
                    .map(|extension| (*extension).into())
                    .collect(),
            }],
            prompt: prompt.into(),
        };
        let picked = files(cx).pick(request, cx);
        let runtime = runtime.clone();
        cx.spawn(async move |this, cx| {
            let Ok(Some(path)) = picked.await else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                if audio {
                    this.import(&runtime, session, path, false, cx);
                } else {
                    this.import_transcript(&runtime, session, path, cx);
                }
            });
        })
        .detach();
    }

    /// Shipping's `processAudioFile` for a file dropped on the note.
    pub fn drop_audio(
        &mut self,
        runtime: &RuntimeHandle,
        session: SessionId,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) {
        self.import(runtime, session, path, true, cx);
    }

    fn import(
        &mut self,
        runtime: &RuntimeHandle,
        session: SessionId,
        source: PathBuf,
        dropped: bool,
        cx: &mut Context<Self>,
    ) {
        if !is_audio(&source) && !(dropped && extension(&source) == "qta") {
            return;
        }
        self.start(
            runtime,
            session,
            Phase::Importing,
            false,
            cx,
            move |host, session, updates, cancellation| {
                Box::pin(async move {
                    let _lease = host.activities.acquire(session.clone())?;
                    date_note(&host.runtime, session.clone(), source.clone(), dropped).await;
                    let progress = updates.clone();
                    let audio = import_audio(
                        &host.runtime,
                        host.directory.clone(),
                        session.clone(),
                        source,
                        Arc::new(move |percentage| {
                            let _ = progress.unbounded_send(Update::Progress(percentage));
                        }),
                    )
                    .await?;
                    let _ = updates.unbounded_send(Update::AudioChanged);
                    let _ = updates.unbounded_send(Update::Phase(Phase::Transcribing));
                    transcribe_and_retain(&host, session, audio, updates, cancellation).await
                })
            },
        );
    }

    /// Shipping's `useRegenerateTranscript`.
    pub fn retranscribe(
        &mut self,
        runtime: &RuntimeHandle,
        session: SessionId,
        cx: &mut Context<Self>,
    ) {
        let Some(services) = cx.try_global::<MeetingServices>() else {
            return;
        };
        let Ok(audio) = services.capture.audio_path(session.clone()) else {
            return;
        };
        let runtime = runtime.clone();
        cx.spawn(async move |this, cx| {
            let audio = audio.await.ok().and_then(Result::ok);
            let _ = this.update(cx, |this, cx| {
                let Some(audio) = audio else {
                    toasts::show(
                        toasts::Toast::error(MISSING_RECORDING)
                            .id(format!("transcript-regenerate-audio-missing-{}", session.0)),
                        cx,
                    );
                    return;
                };
                this.start(
                    &runtime,
                    session,
                    Phase::Transcribing,
                    true,
                    cx,
                    move |host, session, updates, cancellation| {
                        Box::pin(async move {
                            let _lease = host.activities.acquire(session.clone())?;
                            transcribe_and_retain(&host, session, audio, updates, cancellation)
                                .await
                        })
                    },
                );
            });
        })
        .detach();
    }

    fn start(
        &mut self,
        runtime: &RuntimeHandle,
        session: SessionId,
        phase: Phase,
        retranscribe: bool,
        cx: &mut Context<Self>,
        work: impl FnOnce(
            Host,
            SessionId,
            mpsc::UnboundedSender<Update>,
            CancellationToken,
        ) -> BoxFuture<'static, Result<()>>
        + Send
        + 'static,
    ) {
        if self.running(&session) || self.deleting(&session) {
            return;
        }
        let host = match Host::new(runtime, &session, cx) {
            Ok(host) => host,
            Err(error) => {
                self.jobs
                    .insert(session, Job::Failed(error.to_string().into()));
                cx.notify();
                return;
            }
        };
        self.next_run += 1;
        let id = self.next_run;
        let cancellation = CancellationToken::new();
        self.runs.insert(
            session.clone(),
            Run {
                id,
                cancellation: cancellation.clone(),
                retranscribe,
            },
        );
        self.jobs.insert(
            session.clone(),
            Job::Running {
                phase,
                percentage: 0.,
            },
        );
        cx.notify();
        let (updates, mut received) = mpsc::unbounded();
        let job_session = session.clone();
        let reply = runtime.service(move |_| work(host, job_session, updates, cancellation));
        cx.spawn(async move |this, cx| {
            while let Some(update) = received.next().await {
                let _ = this.update(cx, |this, cx| this.apply(&session, id, update, cx));
            }
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| this.finish(&session, id, result, cx));
        })
        .detach();
    }

    fn current(&self, session: &SessionId, id: u64) -> bool {
        self.runs.get(session).is_some_and(|run| run.id == id)
    }

    fn apply(&mut self, session: &SessionId, id: u64, update: Update, cx: &mut Context<Self>) {
        match update {
            Update::AudioChanged => {
                cx.emit(UploadEvent::AudioChanged(session.clone()));
                return;
            }
            Update::Warning(description) => {
                toasts::show(
                    toasts::Toast::warning("Using a batch transcription provider")
                        .description(description),
                    cx,
                );
                return;
            }
            _ => {}
        }
        if !self.current(session, id) {
            return;
        }
        if let Some(Job::Running { phase, percentage }) = self.jobs.get_mut(session) {
            match update {
                Update::Phase(next) => {
                    *phase = next;
                    *percentage = 0.;
                }
                Update::Progress(next) => *percentage = next.clamp(0., 1.),
                Update::AudioChanged | Update::Warning(_) => {}
            }
            cx.notify();
        }
    }

    fn finish(&mut self, session: &SessionId, id: u64, result: Result<()>, cx: &mut Context<Self>) {
        cx.emit(UploadEvent::AudioChanged(session.clone()));
        if !self.current(session, id) {
            return;
        }
        let retranscribe = self
            .runs
            .remove(session)
            .is_some_and(|run| run.retranscribe);
        match result {
            Ok(()) | Err(ServiceError::Cancelled) => {
                self.jobs.remove(session);
            }
            Err(error) => {
                let message = message(&error);
                if retranscribe {
                    toasts::show(
                        toasts::Toast::error("Re-transcription failed")
                            .id(format!("transcript-regenerate-failed-{}", session.0))
                            .description(message.clone()),
                        cx,
                    );
                }
                self.jobs
                    .insert(session.clone(), Job::Failed(message.into()));
            }
        }
        cx.notify();
    }

    fn import_transcript(
        &mut self,
        runtime: &RuntimeHandle,
        session: SessionId,
        path: PathBuf,
        cx: &mut Context<Self>,
    ) {
        if !is_transcript(&path) {
            return;
        }
        let reply = {
            let runtime_copy = runtime.clone();
            runtime.service(
                move |_| async move { import_subtitle(&runtime_copy, session, path).await },
            )
        };
        cx.spawn(async move |_, _| {
            if let Ok(reply) = reply {
                let _ = reply.receive().await;
            }
        })
        .detach();
    }

    /// The audio player's `deleteRecording`: stops playback, then deletes once the session's
    /// audio is idle.
    pub fn delete_recording(
        &mut self,
        runtime: &RuntimeHandle,
        session: SessionId,
        cx: &mut Context<Self>,
    ) {
        if self.deleting(&session) || self.running(&session) {
            return;
        }
        let Ok(host) = Host::new(runtime, &session, cx) else {
            return;
        };
        if let Some(services) = cx.try_global::<MeetingServices>() {
            let playing = services.playback.snapshot().path;
            if playing.as_ref().and_then(|path| path.parent()) == Some(host.directory.as_path()) {
                let _ = services.playback.send(Command::Stop);
            }
        }
        self.deleting.insert(session.clone());
        cx.notify();
        let id = session.clone();
        let reply = runtime.service(move |_| async move {
            let _lease = host.activities.acquire(id.clone())?;
            delete_audio(&host.runtime, host.directory, id).await
        });
        cx.spawn(async move |this, cx| {
            if let Ok(reply) = reply {
                let _ = reply.receive().await;
            }
            let _ = this.update(cx, |this, cx| {
                this.deleting.remove(&session);
                cx.emit(UploadEvent::AudioChanged(session));
                cx.notify();
            });
        })
        .detach();
    }

    /// Shipping's "Show Audio in Finder": opens the session's audio folder.
    pub fn show_audio(&mut self, session: SessionId, cx: &mut Context<Self>) {
        let Some(services) = cx.try_global::<MeetingServices>() else {
            return;
        };
        let Ok(audio) = services.capture.audio_path(session) else {
            return;
        };
        cx.spawn(async move |_, cx| {
            let Some(folder) = audio
                .await
                .ok()
                .and_then(Result::ok)
                .and_then(|path| path.parent().map(Path::to_path_buf))
            else {
                return;
            };
            cx.update(|cx| files(cx).open_folder(&folder, cx));
        })
        .detach();
    }
}

/// Shipping's `AudioDropTarget` around the memo and summary: dragging one audio file over the
/// note shows the drop overlay; dropping it uploads and transcribes it.
pub fn audio_drop_target(
    content: gpui::AnyElement,
    colors: Theme,
    runtime: RuntimeHandle,
    session: SessionId,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let active = window.use_keyed_state("audio-drop-target", cx, |_, _| false);
    let over = *active.read(cx) && cx.has_active_drag();
    let hover = active.clone();
    div()
        .id("audio-drop-target")
        .relative()
        .size_full()
        .on_drag_move::<ExternalPaths>(move |event, _, cx| {
            let inside = event.bounds.contains(&event.event.position)
                && single_audio(event.drag(cx)).is_some();
            hover.update(cx, |value, cx| {
                if *value != inside {
                    *value = inside;
                    cx.notify();
                }
            });
        })
        .can_drop(|value, _, _| {
            value
                .downcast_ref::<ExternalPaths>()
                .and_then(single_audio)
                .is_some()
        })
        .on_drop(move |paths: &ExternalPaths, _, cx| {
            active.update(cx, |value, cx| {
                *value = false;
                cx.notify();
            });
            if let Some(path) = single_audio(paths) {
                let uploads = Uploads::global(cx);
                uploads.update(cx, |uploads, cx| {
                    uploads.drop_audio(&runtime, session.clone(), path, cx)
                });
            }
        })
        .child(content)
        .when(over, |view| view.child(drop_overlay(colors)))
}

/// The overlay's dashed frame over an 18px dot grid, and its centered card.
fn drop_overlay(colors: Theme) -> impl IntoElement {
    let formats = AUDIO_EXTENSIONS.map(str::to_uppercase);
    let (last, rest) = formats.split_last().unwrap_or((&formats[0], &[]));
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .rounded_lg()
        .border_1()
        .border_dashed()
        .border_color(colors.border.opacity(0.7))
        .bg(colors.background)
        .text_color(colors.muted_foreground)
        .child(
            canvas(
                |_, _, _| {},
                |bounds, _, window, _| {
                    let step = 18.;
                    let (width, height) =
                        (f32::from(bounds.size.width), f32::from(bounds.size.height));
                    // `radial-gradient(circle at center, ...)` tiles from the element's center.
                    let offset = |extent: f32| (extent / 2.) % step - step / 2.;
                    let mut y = offset(height) + step / 2.;
                    while y < height {
                        let mut x = offset(width) + step / 2.;
                        while x < width {
                            window.paint_quad(
                                fill(
                                    Bounds::new(
                                        bounds.origin + point(px(x - 1.), px(y - 1.)),
                                        size(px(2.), px(2.)),
                                    ),
                                    rgba(0x71717a57),
                                )
                                .corner_radii(px(1.)),
                            );
                            x += step;
                        }
                        y += step;
                    }
                },
            )
            .absolute()
            .inset_0(),
        )
        .child(
            div()
                .relative()
                .flex()
                .items_center()
                .gap_3()
                .rounded(px(6.))
                .border_1()
                .border_color(colors.border.opacity(0.7))
                .bg(colors.card.opacity(0.95))
                .px_4()
                .py_3()
                .shadow_sm()
                .text_color(colors.foreground)
                .child(
                    svg()
                        .path("meeting/AudioWaveformIcon.svg")
                        .size_5()
                        .flex_shrink_0()
                        .text_color(colors.muted_foreground),
                )
                .child(
                    div()
                        .flex()
                        .min_w_0()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(MEDIUM)
                                .child("Drop to upload and transcribe audio"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child(format!("{}, or {last} audio", rest.join(", "))),
                        ),
                ),
        )
}

async fn transcribe_and_retain(
    host: &Host,
    session: SessionId,
    audio: PathBuf,
    updates: mpsc::UnboundedSender<Update>,
    cancellation: CancellationToken,
) -> Result<()> {
    let (progress, warnings) = (updates.clone(), updates.clone());
    transcribe(
        &host.runtime,
        session.clone(),
        audio,
        host.transcriber.clone(),
        Arc::new(move |percentage| {
            let _ = progress.unbounded_send(Update::Progress(percentage));
        }),
        Arc::new(move |description| {
            let _ = warnings.unbounded_send(Update::Warning(description));
        }),
        cancellation,
    )
    .await?;
    // Shipping logs a failed retention deletion; the transcript is already saved.
    if apply_retention(&host.runtime, host.directory.clone(), session)
        .await
        .unwrap_or(false)
    {
        let _ = updates.unbounded_send(Update::AudioChanged);
    }
    Ok(())
}

fn message(error: &ServiceError) -> String {
    match error {
        ServiceError::Failed(message) => message.to_string(),
        error => error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use desktop_runtime::Profile;
    use owhisper_interface::batch::{Alternatives, Channel, Results, Word};

    struct Fixture {
        runtime: RuntimeHandle,
        session: SessionId,
        directory: PathBuf,
    }

    impl Fixture {
        async fn new() -> Self {
            let directory =
                std::env::temp_dir().join(format!("anarlog-upload-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&directory).unwrap();
            let (runtime, ready) = RuntimeHandle::start(Profile {
                database: directory.join("db.sqlite"),
            })
            .unwrap();
            ready.receive().await.unwrap();
            let session = runtime
                .create_note("Upload fixture".into())
                .unwrap()
                .receive()
                .await
                .unwrap()
                .summary
                .id;
            Self {
                runtime,
                session,
                directory,
            }
        }

        async fn rows(&self, sql: &str) -> Vec<Value> {
            let session = self.session.clone();
            let sql = sql.to_owned();
            self.runtime
                .submit(move |services| async move {
                    services
                        .executor
                        .execute(sql, vec![json!(session)])
                        .await
                        .map_err(failure)
                })
                .unwrap()
                .receive()
                .await
                .unwrap()
        }

        fn audio_directory(&self) -> PathBuf {
            self.directory
                .join("vault")
                .join("sessions")
                .join(self.session.0.as_ref())
        }

        fn wav(&self) -> PathBuf {
            let path = self.directory.join("meeting.wav");
            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: 16_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            };
            let mut writer = hound::WavWriter::create(&path, spec).unwrap();
            for sample in 0..16_000 {
                writer
                    .write_sample(((sample % 100) as i16 - 50) * 100)
                    .unwrap();
            }
            writer.finalize().unwrap();
            path
        }
    }

    fn fake(text: &[&str]) -> Transcriber {
        let words: Vec<Word> = text
            .iter()
            .enumerate()
            .map(|(index, text)| Word {
                word: text.to_lowercase(),
                start: index as f64 * 0.5,
                end: index as f64 * 0.5 + 0.4,
                confidence: 1.,
                channel: 0,
                speaker: Some(index % 2),
                punctuated_word: Some((*text).into()),
            })
            .collect();
        let transcript = text.join(" ");
        Arc::new(
            move |_: SessionId,
                  _: PathBuf,
                  progress: Progress,
                  _: Warn,
                  _: CancellationToken|
                  -> BoxFuture<'static, Result<Transcription>> {
                let response = Response {
                    metadata: json!({}),
                    results: Results {
                        channels: vec![Channel {
                            alternatives: vec![Alternatives {
                                transcript: transcript.clone(),
                                confidence: 1.,
                                words: words.clone(),
                            }],
                        }],
                    },
                };
                progress(0.5);
                Box::pin(async move {
                    Ok(Transcription {
                        response,
                        provider: "deepgram".into(),
                        model: "nova-3".into(),
                    })
                })
            },
        )
    }

    fn quiet() -> Progress {
        Arc::new(|_| {})
    }

    #[tokio::test]
    async fn uploaded_audio_is_imported_catalogued_and_its_transcript_replaces_the_session() {
        let fixture = Fixture::new().await;
        let session = fixture.session.clone();
        let words = json!([{"id": "old", "text": " Old words here", "start_ms": 0, "end_ms": 400, "channel": 0}]).to_string();
        fixture.runtime.submit(move |services| async move {
            services.executor.execute("INSERT INTO transcripts (id, session_id, started_at_ms, words_json) VALUES ('old', ?, 0, ?)".into(), vec![json!(session), json!(words)]).await.map_err(failure)?;
            Ok(())
        }).unwrap().receive().await.unwrap();

        let audio = import_audio(
            &fixture.runtime,
            fixture.audio_directory(),
            fixture.session.clone(),
            fixture.wav(),
            quiet(),
        )
        .await
        .unwrap();
        assert_eq!(audio, fixture.audio_directory().join("audio.mp3"));
        assert!(audio.is_file());
        let attachments = fixture.rows("SELECT id FROM session_attachments WHERE session_id = ? AND source_type = 'session_audio' AND deleted_at IS NULL").await;
        assert_eq!(attachments.len(), 1);

        transcribe(
            &fixture.runtime,
            fixture.session.clone(),
            audio,
            fake(&["Hello", "there.", "Goodbye", "now."]),
            quiet(),
            Arc::new(|_| {}),
            CancellationToken::new(),
        )
        .await
        .unwrap();
        let transcripts = fixture.rows("SELECT id, provider, words_json, speaker_hints_json FROM transcripts WHERE session_id = ? AND deleted_at IS NULL").await;
        assert_eq!(
            transcripts.len(),
            1,
            "the new transcript replaces the session's old one"
        );
        assert_eq!(transcripts[0]["provider"], "deepgram");
        let words: Vec<Value> =
            serde_json::from_str(transcripts[0]["words_json"].as_str().unwrap()).unwrap();
        let text: String = words
            .iter()
            .map(|word| word["text"].as_str().unwrap())
            .collect();
        assert_eq!(text, " Hello there. Goodbye now.");
        assert_eq!(words[1]["start_ms"], 500);
        let hints: Vec<Value> =
            serde_json::from_str(transcripts[0]["speaker_hints_json"].as_str().unwrap()).unwrap();
        assert_eq!(hints.len(), 4);
        assert_eq!(hints[1]["type"], "provider_speaker_index");

        let silent: Transcriber = Arc::new(
            |_: SessionId,
             _: PathBuf,
             _: Progress,
             _: Warn,
             _: CancellationToken|
             -> BoxFuture<'static, Result<Transcription>> {
                Box::pin(async {
                    Ok(Transcription {
                        response: Response {
                            metadata: json!({}),
                            results: Results { channels: vec![] },
                        },
                        provider: "deepgram".into(),
                        model: "nova-3".into(),
                    })
                })
            },
        );
        let error = transcribe(
            &fixture.runtime,
            fixture.session.clone(),
            fixture.wav(),
            silent,
            quiet(),
            Arc::new(|_| {}),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
        assert_eq!(message(&error), EMPTY_TRANSCRIPT);
        assert_eq!(
            fixture
                .rows("SELECT id FROM transcripts WHERE session_id = ? AND deleted_at IS NULL")
                .await
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn deleting_a_recording_removes_the_files_and_marks_the_audio_absent() {
        let fixture = Fixture::new().await;
        let directory = fixture.audio_directory();
        import_audio(
            &fixture.runtime,
            directory.clone(),
            fixture.session.clone(),
            fixture.wav(),
            quiet(),
        )
        .await
        .unwrap();
        delete_audio(&fixture.runtime, directory.clone(), fixture.session.clone())
            .await
            .unwrap();
        assert!(!directory.join("audio.mp3").exists());
        let rows = fixture.rows("SELECT a.deleted_at IS NOT NULL AS deleted, l.availability FROM session_attachments a JOIN attachment_local_state l ON l.attachment_id = a.id WHERE a.session_id = ?").await;
        assert_eq!(rows[0]["deleted"], 1);
        assert_eq!(rows[0]["availability"], "absent");
    }

    #[tokio::test]
    async fn uploaded_subtitles_become_a_transcript_of_their_cues() {
        let fixture = Fixture::new().await;
        let path = fixture.directory.join("meeting.vtt");
        std::fs::write(&path, "WEBVTT\n\n00:00:01.000 --> 00:00:02.500\nHello there.\n\n00:00:03.000 --> 00:00:04.000\nGoodbye.\n").unwrap();
        assert_eq!(
            import_subtitle(&fixture.runtime, fixture.session.clone(), path)
                .await
                .unwrap(),
            2
        );
        let rows = fixture.rows("SELECT source, words_json FROM transcripts WHERE session_id = ? AND deleted_at IS NULL").await;
        assert_eq!(rows[0]["source"], "subtitle_import");
        let words: Vec<Value> =
            serde_json::from_str(rows[0]["words_json"].as_str().unwrap()).unwrap();
        assert_eq!(
            (
                words[0]["text"].as_str(),
                words[0]["start_ms"].as_i64(),
                words[0]["end_ms"].as_i64()
            ),
            (Some("Hello there."), Some(1000), Some(2500))
        );
        assert_eq!(words[1]["channel"], 2);
    }

    #[test]
    fn text_only_responses_get_evenly_timed_synthetic_words() {
        let response = Response {
            metadata: json!({}),
            results: Results {
                channels: vec![Channel {
                    alternatives: vec![Alternatives {
                        transcript: "Just text".into(),
                        confidence: 1.,
                        words: vec![],
                    }],
                }],
            },
        };
        let (words, hints) = stored_words(&response, "openai");
        assert!(hints.is_empty());
        assert_eq!(words[1].text.as_deref(), Some(" text"));
        assert_eq!(
            (words[1].start_ms, words[1].end_ms),
            (Some(400.), Some(800.))
        );
        assert!(
            words[0]
                .metadata
                .as_ref()
                .unwrap()
                .as_str()
                .unwrap()
                .contains("synthetic_text")
        );
        assert!(is_audio(Path::new("call.M4A")) && !is_audio(Path::new("notes.txt")));
    }
}
