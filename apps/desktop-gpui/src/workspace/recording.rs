//! The listener store (`store/zustand/listener`) and `useStartListening` /
//! `stopListening`: the capture lifecycle as the header, sidebar, and toasts
//! see it. The engine itself is `listener-core`'s root actor
//! (`crate::recording::Recorder`).

use std::rc::Rc;

use anlg_listener_core::actors::SessionParams;
use anlg_listener_core::{
    DegradedError, SessionDataEvent, SessionLifecycleEvent, SessionProgressEvent, TranscriptionMode,
};
use gpui::{AnyElement, Context, Div, SharedString, Window, div, prelude::*, px};

use super::Workspace;
use crate::recording::{Event, Recorder};
use crate::ui::TailwindText as _;

/// `getSessionMode(sessionId)`
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SessionMode {
    Inactive,
    Active,
    Finalizing,
    RunningBatch,
}

/// `state.batch[sessionId]`
#[derive(Clone, Debug)]
pub(crate) struct BatchState {
    pub phase: BatchPhase,
    pub percentage: Option<f64>,
    pub error: Option<String>,
    /// `stop_transcription`: aborts the running `run_batch` task.
    pub abort: Option<tokio::task::AbortHandle>,
    /// The capture marker's transcript id when this is a post-stop repair,
    /// so a stop can clear the recovery it would otherwise leave behind.
    pub lifecycle_transcript_id: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum BatchPhase {
    Importing,
    Transcribing,
}

/// `AUDIO_EXTENSIONS`
const AUDIO_EXTENSIONS: [&str; 8] = ["wav", "mp3", "ogg", "mp4", "m4a", "flac", "webm", "aac"];

/// `DIRECT_BATCH_PROVIDERS`
const DIRECT_BATCH_PROVIDERS: [&str; 25] = [
    "deepgram",
    "cartesia",
    "soniox",
    "assemblyai",
    "openai",
    "openrouter",
    "siliconflow",
    "zai",
    "gladia",
    "elevenlabs",
    "mistral",
    "meta",
    "pyannote",
    "aquavoice",
    "cohere",
    "aws_transcribe",
    "azure_speech",
    "google_cloud",
    "google_generative_ai",
    "groq",
    "revai",
    "speechmatics",
    "together",
    "xai",
    "smallestai",
];

/// `BatchTarget`
#[derive(Debug, Clone)]
pub(super) struct BatchTarget {
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
}

/// `resolveBatchTarget`: the selected provider when it has a batch adapter,
/// otherwise the on-device Soniqo model where it exists.
pub(super) fn batch_target(connection: Option<&crate::db::SttConnection>) -> Option<BatchTarget> {
    let selected = connection.and_then(|conn| {
        batch_provider(&conn.provider, &conn.model).map(|provider| BatchTarget {
            provider: provider.to_string(),
            model: conn.model.clone(),
            base_url: conn.base_url.clone(),
            api_key: conn.api_key.clone(),
        })
    });
    let local_available = cfg!(all(target_os = "macos", target_arch = "aarch64"));
    let fallback = local_available.then(|| BatchTarget {
        provider: "soniqo".to_string(),
        model: "soniqo-parakeet-batch".to_string(),
        base_url: "soniqo://local".to_string(),
        api_key: String::new(),
    });
    selected.or(fallback)
}

/// `RunOptions.promotion`
#[derive(Debug, Clone)]
pub(crate) enum BatchPromotion {
    /// Tombstone the session's other transcripts.
    WholeSession,
    /// The post-stop repair of a capture: keep the words after the existing
    /// audio, re-based to the capture's start, and replace the live
    /// transcript it wrote.
    CurrentCapture {
        existing_audio_ms: i64,
        replace_transcript_id: Option<String>,
        started_at_ms: i64,
    },
}

/// What follows a completed batch.
#[derive(Debug, Clone)]
pub(crate) enum BatchFollowUp {
    /// Re-transcribe / import: `markSessionAudioTranscriptionComplete` and
    /// `queueAutoEnhanceIfSummaryEmpty`.
    Standalone,
    /// `finalizeStopped`'s `batch_then_enhance`: the capture lifecycle
    /// schedules the summary and finishes the audio itself.
    CaptureLifecycle {
        summary_mode: super::enhance::AutoEnhanceMode,
        live_transcript_id: String,
        audio_path: String,
        marker: Box<crate::capture_marker::Marker>,
        recovery_attempt: Option<u32>,
        /// `details.liveTranscriptionActive`: with live text on screen the
        /// repair finishes quietly (`notifyOnCompletion: false`), and its
        /// failure toast names the transcript save rather than the batch.
        live_active: bool,
        /// `transcriptWriteError`: part of the live transcript failed to save.
        transcript_write_failed: bool,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct BatchRun {
    pub promotion: BatchPromotion,
    pub after: BatchFollowUp,
}

/// `getBatchProvider`
fn batch_provider(provider: &str, model: &str) -> Option<&'static str> {
    if provider == "cloudflare_workers_ai" {
        return Some("deepgram");
    }
    if crate::db::is_local_file_stt_model(provider, model) {
        return Some("whispercpp");
    }
    DIRECT_BATCH_PROVIDERS
        .iter()
        .copied()
        .find(|p| *p == provider)
}

/// `createCaptureLifecycle`'s transcript identity and persistence queue: the
/// transcript row is created on the first delta and later deltas are
/// journaled, one write in flight at a time (the worker's batching), then
/// flushed into the columns when the capture ends.
pub(crate) struct LivePersistence {
    pub transcript_id: String,
    pub created_at: String,
    pub started_at_ms: i64,
    pub memo: String,
    pub provider: String,
    pub model: String,
    pub created: bool,
    pub writing: bool,
    pub pending: Vec<anlg_listener_core::LiveTranscriptDelta>,
    /// The capture ended: once the queue drains, flush the journal.
    pub finishing: bool,
}

pub(crate) struct LiveCapture {
    pub session_id: String,
    pub persistence: LivePersistence,
    /// `live.requestedLiveTranscription`: the session asked for live mode.
    pub requested_live: bool,
    /// `live.liveTranscriptionActive`: the engine is streaming live.
    pub live_active: bool,
    /// `live.degraded`: the engine's degradation error, if any.
    pub error: Option<DegradedError>,
    pub mic: f32,
    pub speaker: f32,
    pub muted: bool,
    /// `liveSegments`: the engine's rendered segments for the floating panel.
    pub segments: Vec<anlg_listener_core::LiveTranscriptSegment>,
    /// `MeetingFloatData` for this session: title, owner, participants, names.
    pub label_context: Option<super::floating_bar::LabelContext>,
    /// `SessionStateSnapshot.mic_isolated`: every stream so far came from an
    /// isolated (headphone) mic.
    pub mic_isolated: Option<bool>,
    /// `createCaptureLifecycle`'s post-capture inputs.
    pub lifecycle: CaptureLifecycle,
    /// `tickTranscriptionStallWatchdog`'s counters: audible seconds since any
    /// live transcript activity, and since the last finalized words.
    pub stall_audible_seconds: u32,
    pub final_stall_audible_seconds: u32,
    /// `live.transcriptionStalled`
    pub transcription_stalled: bool,
}

/// `TRANSCRIPTION_STALL_AMPLITUDE_THRESHOLD`: quieter seconds do not count.
const TRANSCRIPTION_STALL_AMPLITUDE_THRESHOLD: f32 = 0.05;
/// `TRANSCRIPTION_STALL_AUDIBLE_SECONDS`: audible seconds without any words.
const TRANSCRIPTION_STALL_AUDIBLE_SECONDS: u32 = 45;
/// `TRANSCRIPTION_FINAL_STALL_AUDIBLE_SECONDS`: audible seconds with partials
/// but no finalized words.
const TRANSCRIPTION_FINAL_STALL_AUDIBLE_SECONDS: u32 = 90;
/// The persistent warning's sonner id.
pub(crate) const STALLED_TOAST_ID: &str = "live-transcription-stalled";

/// `createCaptureLifecycle`: what the capture knew at start plus what the
/// live stream reported, deciding `getPostCaptureAction` at stop.
#[derive(Debug, Clone, Default)]
pub(crate) struct CaptureLifecycle {
    /// `preserveExistingTranscript`: the session already had a transcript.
    pub preserve_existing_transcript: bool,
    /// `getExistingAudioDurationMs` at start, when preserving.
    pub existing_audio_ms: i64,
    /// `live.needsBatchRepair`: live transcription was requested but not
    /// active or degraded at some point.
    pub needs_batch_repair: bool,
    /// `transcriptTouched`: a delta with content was persisted.
    pub transcript_touched: bool,
    /// `ownerUserId` for the recovery marker.
    pub owner_user_id: String,
    /// `initialTitle` for the recovery marker.
    pub initial_title: Option<String>,
    /// `startedAutomatically`: a scheduled meeting's auto-start.
    pub automatic: bool,
    /// `preserveExistingAudio`: `true` for a manual capture; for an
    /// automatic one, whether the session already had audio at start.
    pub preserve_existing_audio: bool,
    /// `live.requestedLiveTranscription`
    pub requested_live: bool,
}

impl CaptureLifecycle {
    /// `discardEmptyAutomaticCapture`'s preconditions that need no I/O: only
    /// a fresh automatic capture of a session the start left untouched, whose
    /// transcription completed, is a candidate.
    fn discard_candidate(&self, live_active: bool, transcript_write_failed: bool) -> bool {
        let transcription_complete = (!self.requested_live || live_active)
            && !self.needs_batch_repair
            && !transcript_write_failed;
        self.automatic
            && !self.preserve_existing_audio
            && !self.preserve_existing_transcript
            && self.initial_title.is_some()
            && !self.transcript_touched
            && transcription_complete
    }
}

impl CaptureLifecycle {
    /// `marker()`: the durable `CaptureLifecycleMarker` for this capture.
    fn marker(
        &self,
        session_id: &str,
        persistence: &LivePersistence,
        phase: crate::capture_marker::Phase,
        summary_mode: Option<crate::capture_marker::SummaryMode>,
    ) -> crate::capture_marker::Marker {
        crate::capture_marker::Marker {
            version: 1,
            phase: Some(phase),
            session_id: session_id.to_string(),
            transcript_id: persistence.transcript_id.clone(),
            started_at: persistence.started_at_ms,
            created_at: persistence.created_at.clone(),
            audio_offset_ms: self.existing_audio_ms.max(0),
            preserve_existing_transcript: self.preserve_existing_transcript,
            automatic: Some(self.automatic),
            preserve_existing_audio: Some(self.preserve_existing_audio),
            initial_title: self.initial_title.clone(),
            owner_user_id: self.owner_user_id.clone(),
            memo: persistence.memo.clone(),
            provider: Some(persistence.provider.clone()).filter(|p| !p.is_empty()),
            model: Some(persistence.model.clone()).filter(|m| !m.is_empty()),
            summary_mode,
            refresh_summary_after_repair: false,
        }
    }
}

fn summary_mode_marker(
    mode: super::enhance::AutoEnhanceMode,
) -> crate::capture_marker::SummaryMode {
    match mode {
        super::enhance::AutoEnhanceMode::Regenerate => {
            crate::capture_marker::SummaryMode::Regenerate
        }
        super::enhance::AutoEnhanceMode::IfEmpty => crate::capture_marker::SummaryMode::IfEmpty,
    }
}

impl LiveCapture {
    /// `Boolean(degraded)` for the amber tint: any degradation or a capture
    /// that is not transcribing live.
    pub fn degraded(&self) -> bool {
        self.error.is_some() || !self.live_active
    }
}

#[derive(Default)]
pub(crate) struct RecordingState {
    pub recorder: Option<Rc<Recorder>>,
    pub live: Option<LiveCapture>,
    pub finalizing: Vec<String>,
    /// Persistence queues of captures that ended and still have writes or
    /// the final flush outstanding, by session id.
    pub flushing: Vec<(String, LivePersistence)>,
    /// `state.batch`: import / batch transcription progress and errors.
    pub batch: std::collections::HashMap<String, BatchState>,
    /// The floating recording bar window while a live session shows it.
    pub floating_bar: Option<gpui::WindowHandle<super::floating_bar::FloatingBar>>,
    /// The persistent `recording-without-transcription` warning toast.
    pub toast: Option<RecordingToast>,
    /// `Record` was clicked and the engine has not answered yet.
    pub starting: bool,
    /// `MicIsolationCache` + its store scope.
    pub mic_isolation: crate::voiceprint::MicIsolation,
    /// Captures whose final transcript flush and `Inactive` event are still
    /// meeting up for `finalizeStopped`, by session id.
    pub pending_post_capture: std::collections::HashMap<String, PendingPostCapture>,
    /// Sessions whose batch `useRegenerateTranscript` started: its failure
    /// raises the `Re-transcription failed` toast.
    pub retranscribing: std::collections::HashSet<String>,
}

/// `onStopped` waits for both the transcript persistence flush and the
/// engine's `Inactive` details before `finalizeStopped` runs.
#[derive(Default)]
pub(crate) struct PendingPostCapture {
    /// The live transcript's id, whether a row was written
    /// (`transcriptCreated`), and whether the final flush succeeded.
    pub flush: Option<(String, bool, bool)>,
    /// What the capture knew when it ended: `liveTranscriptionActive`, the
    /// lifecycle inputs, and its `Finalizing` marker (without a summary mode).
    pub snapshot: Option<(bool, CaptureLifecycle, crate::capture_marker::Marker)>,
    /// `details.audioPath` once the engine reported `Inactive` and the audio
    /// was catalogued.
    pub inactive: Option<Option<String>>,
    /// `recoveredMarker.summaryMode`: a recovered finalization that only has
    /// the summary left.
    pub recovered_summary_mode: Option<super::enhance::AutoEnhanceMode>,
    /// `recoverStopped` rather than `onStopped`: failures do not request
    /// another recovery pass beyond the retry budget.
    pub recovery_attempt: Option<u32>,
    /// The empty-automatic-capture check ran and the audio was catalogued
    /// (`catalogLocalSessionAudio`), in `finalizeStopped`'s order.
    pub catalogued: bool,
}

/// `getPostCaptureAction`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PostCaptureAction {
    EnhanceOnly,
    BatchThenEnhance,
    None,
}

/// `getPostCaptureAction(details, canRunBatch)`
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PostCaptureInputs {
    pub has_audio: bool,
    pub live_transcription_active: bool,
    pub needs_batch_repair: bool,
    /// `shouldRefineSpeakerDiarization`: a settled diarization pass after a
    /// multi-speaker cloud transcript (unavailable to the signed-out shell).
    pub refine_speaker_diarization: bool,
    pub transcript_write_failed: bool,
}

pub(crate) fn post_capture_action(
    details: PostCaptureInputs,
    can_run_batch: bool,
) -> PostCaptureAction {
    let live_transcript_complete = details.live_transcription_active
        && !details.needs_batch_repair
        && !details.transcript_write_failed;
    if live_transcript_complete && !details.refine_speaker_diarization {
        return PostCaptureAction::EnhanceOnly;
    }
    if details.has_audio && can_run_batch {
        return PostCaptureAction::BatchThenEnhance;
    }
    if live_transcript_complete {
        return PostCaptureAction::EnhanceOnly;
    }
    PostCaptureAction::None
}

/// The `audioOffsetMs` of a `current_capture` promotion: the existing audio's
/// length when the final file is at least that long (minus a second of
/// tolerance), otherwise the capture starts the file over.
pub(crate) fn current_capture_audio_offset_ms(
    existing_audio_ms: i64,
    final_audio_ms: Option<i64>,
) -> i64 {
    match final_audio_ms {
        Some(final_ms) if existing_audio_ms > 0 && final_ms + 1_000 >= existing_audio_ms => {
            existing_audio_ms.min(final_ms)
        }
        _ => 0,
    }
}

pub(crate) struct RecordingToast {
    pub title: SharedString,
    pub description: SharedString,
    pub action: &'static str,
}

impl Workspace {
    pub(crate) fn session_mode(&self, session_id: &str) -> SessionMode {
        if self
            .recording
            .live
            .as_ref()
            .is_some_and(|live| live.session_id == session_id)
        {
            SessionMode::Active
        } else if self.recording.finalizing.iter().any(|id| id == session_id) {
            SessionMode::Finalizing
        } else if self
            .recording
            .batch
            .get(session_id)
            .is_some_and(|batch| batch.error.is_none())
        {
            SessionMode::RunningBatch
        } else {
            SessionMode::Inactive
        }
    }

    pub(crate) fn batch_state(&self, session_id: &str) -> Option<&BatchState> {
        self.recording.batch.get(session_id)
    }

    /// `selectAndUpload("audio")` → `processFile(path, "audio")`: the native
    /// dialog, then `runAudioImport`: the estimated note date, the import
    /// with progress (`handleBatchStarted(sessionId, "importing")`), the
    /// audio catalog, and the batch transcription — which, without a batch
    /// target, fails the way `useRunBatch` does.
    pub(crate) fn upload_audio(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_overflow_menu(cx);
        let Some(session_id) = self.selected.clone() else {
            return;
        };
        // `selectFile({ title: "Upload Audio", defaultPath: downloadDir(), filters })`
        let picker = crate::dialogs::pick(
            cx,
            crate::dialogs::Options {
                title: "Upload Audio".into(),
                pick: crate::dialogs::Pick::File,
                start_dir: dirs::download_dir(),
                filters: vec![crate::dialogs::Filter::Extensions {
                    name: "Audio",
                    extensions: &["wav", "mp3", "ogg", "mp4", "m4a", "flac", "webm", "aac"],
                }],
            },
        );
        cx.spawn_in(window, async move |this, cx| {
            let Some(paths) = picker.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let extension = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.to_ascii_lowercase())
                .unwrap_or_default();
            if !AUDIO_EXTENSIONS.contains(&extension.as_str()) {
                return;
            }
            this.update(cx, |this, cx| this.run_audio_import(session_id, path, cx))
                .ok();
        })
        .detach();
    }

    /// `runAudioImport`
    pub(super) fn run_audio_import(
        &mut self,
        session_id: String,
        path: std::path::PathBuf,
        cx: &mut Context<Self>,
    ) {
        // `applyEstimatedAudioNoteDate`: only for sessions without an event.
        let has_event = match &self.note {
            super::Note::Ready { preview, .. } if preview.session.id == session_id => {
                !preview.session.event_json.trim().is_empty()
            }
            _ => true,
        };
        let date_task = (!has_event)
            .then(|| crate::db::Store::estimate_audio_created_at(path.clone()))
            .flatten()
            .map(|created_at| self.store.update_created_at(session_id.clone(), created_at));
        // `handleBatchStarted(sessionId, "importing")`
        self.recording.batch.insert(
            session_id.clone(),
            BatchState {
                phase: BatchPhase::Importing,
                percentage: None,
                error: None,
                abort: None,
                lifecycle_transcript_id: None,
            },
        );
        self.ensure_default_summary(cx);
        cx.notify();
        let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<f64>();
        let import = self
            .store
            .import_audio(session_id.clone(), path, progress_tx);
        let connection = self.store.stt_connection(&self.provider_settings);
        cx.spawn(async move |this, cx| {
            if let Some(task) = date_task {
                let _ = task.await;
            }
            // `updateBatchProgress` from the `audioImportProgress` events.
            let progress_session = session_id.clone();
            let progress_pump = cx.spawn({
                let this = this.clone();
                async move |cx| {
                    while let Some(percentage) = progress_rx.recv().await {
                        this.update(cx, |this, cx| {
                            if let Some(batch) = this.recording.batch.get_mut(&progress_session) {
                                batch.percentage = Some(percentage);
                                cx.notify();
                            }
                        })
                        .ok();
                    }
                }
            });
            let imported = import.await.map_err(anyhow::Error::from).and_then(|r| r);
            drop(progress_pump);
            let catalog = match &imported {
                Ok(_) => this
                    .update(cx, |this, _| {
                        this.store.catalog_session_audio(session_id.clone())
                    })
                    .ok(),
                Err(_) => None,
            };
            if let Some(catalog) = catalog
                && let Ok(Err(error)) = catalog.await
            {
                tracing::error!(%error, "[upload] failed to catalog imported audio");
            }
            let connection = connection.await.ok().flatten();
            this.update(cx, |this, cx| {
                match imported {
                    Ok(_) => {
                        // `clearBatchSession`, then `runBatch(importedPath)`.
                        this.recording.batch.remove(&session_id);
                        this.run_batch(session_id.clone(), connection, cx);
                    }
                    Err(error) => {
                        tracing::error!(%error, "[upload] audio import failed");
                        this.recording.batch.insert(
                            session_id.clone(),
                            BatchState {
                                phase: BatchPhase::Importing,
                                percentage: None,
                                error: Some(error.to_string()),
                                abort: None,
                                lifecycle_transcript_id: None,
                            },
                        );
                    }
                }
                if this.selected.as_deref() == Some(session_id.as_str()) {
                    this.reload_note(session_id.clone(), cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// `useRunBatch`: resolve the batch target — the configured provider when
    /// `getBatchProvider` accepts it, else the fallback (the local Soniqo
    /// batch model exists on Apple silicon only) — then `runBatchSession`:
    /// `handleBatchStarted`, the synthetic progress timer for providers that
    /// do not stream progress, `listener2-core`'s `run_batch`, and on
    /// completion `transformBatch` → the persist callback → `createTranscript`
    /// (`whole_session` promotion) → `markSessionAudioTranscriptionComplete`.
    pub(crate) fn run_batch(
        &mut self,
        session_id: String,
        connection: Option<crate::db::SttConnection>,
        cx: &mut Context<Self>,
    ) {
        self.run_batch_with(
            session_id,
            connection,
            BatchRun {
                promotion: BatchPromotion::WholeSession,
                after: BatchFollowUp::Standalone,
            },
            cx,
        );
    }

    fn run_batch_with(
        &mut self,
        session_id: String,
        connection: Option<crate::db::SttConnection>,
        batch_run: BatchRun,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = batch_target(connection.as_ref()) else {
            let label = connection
                .as_ref()
                .map(|conn| conn.model.clone())
                .unwrap_or_else(|| "the selected speech-to-text provider".to_string());
            let message = format!(
                "{label} is not available for batch transcription on this platform. Configure a batch-capable speech-to-text provider."
            );
            self.fail_batch(session_id.clone(), message.clone(), cx);
            self.after_lifecycle_batch_failed(&session_id, &message, &batch_run.after, cx);
            return;
        };
        let Ok(provider) = serde_json::from_value::<anlg_listener2_core::BatchProvider>(
            serde_json::Value::String(target.provider.clone()),
        ) else {
            self.fail_batch(session_id, "Transcription failed".to_string(), cx);
            return;
        };
        let Some(file_path) = anlg_fs_sync_core::audio::path(&self.store.session_dir(&session_id))
        else {
            self.fail_batch(session_id, "Transcription failed".to_string(), cx);
            return;
        };
        // `handleBatchStarted(sessionId)`
        self.recording.batch.insert(
            session_id.clone(),
            BatchState {
                phase: BatchPhase::Transcribing,
                percentage: Some(0.0),
                error: None,
                abort: None,
                lifecycle_transcript_id: match &batch_run.after {
                    BatchFollowUp::CaptureLifecycle { marker, .. } => {
                        Some(marker.transcript_id.clone())
                    }
                    BatchFollowUp::Standalone => None,
                },
            },
        );
        self.ensure_default_summary(cx);
        cx.notify();
        let languages = self.transcription_languages();
        let context = self.store.batch_session_context(session_id.clone());
        let keywords = self
            .store
            .session_keywords(session_id.clone(), self.dictionary_terms());
        let known_speakers = crate::voiceprint::known_speakers(&self.store, session_id.clone());
        let remember_speakers = self.remember_speakers();
        let mic_isolated = self
            .recording
            .mic_isolation
            .get(&self.store_file, &session_id);
        let runtime = self.store.runtime().clone();
        let synthetic = crate::batch::should_use_synthetic_batch_progress(
            &target.provider,
            Some(&target.model),
            &target.base_url,
        );
        cx.spawn(async move |this, cx| {
            let Ok(Ok((owner_user_id, memo, participant_humans))) = context.await else {
                this.update(cx, |this, cx| {
                    this.fail_batch(session_id, "Transcription failed".to_string(), cx)
                })
                .ok();
                return;
            };
            let keywords = keywords.await.unwrap_or_default();
            let known_speakers = known_speakers
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|known| anlg_listener2_core::KnownSpeaker {
                    id: known.human_id,
                    embedding: known.embedding,
                })
                .collect();
            let num_speakers = crate::batch::session_speaker_count(
                participant_humans.iter().map(String::as_str),
                Some(owner_user_id.as_str()),
            );
            let created_at = chrono::Utc::now();
            let started_at_ms = created_at.timestamp_millis();
            let params = anlg_listener2_core::BatchParams {
                session_id: session_id.clone(),
                provider,
                file_path: file_path.to_string_lossy().into_owned(),
                model: Some(target.model.clone()),
                base_url: target.base_url.clone(),
                api_key: target.api_key.clone(),
                languages,
                keywords,
                num_speakers,
                min_speakers: None,
                max_speakers: None,
                known_speakers,
            };
            let (events_tx, mut events_rx) =
                tokio::sync::mpsc::unbounded_channel::<anlg_listener2_core::BatchEvent>();
            struct Runtime(tokio::sync::mpsc::UnboundedSender<anlg_listener2_core::BatchEvent>);
            impl anlg_listener2_core::BatchRuntime for Runtime {
                fn emit(&self, event: anlg_listener2_core::BatchEvent) {
                    let _ = self.0.send(event);
                }
            }
            let run = runtime.spawn(anlg_listener2_core::run_batch(
                std::sync::Arc::new(Runtime(events_tx)),
                params,
            ));
            let abort = run.abort_handle();
            this.update(cx, |this, _| {
                if let Some(batch) = this.recording.batch.get_mut(&session_id) {
                    batch.abort = Some(abort);
                }
            })
            .ok();
            // `SYNTHETIC_BATCH_PROGRESS_*`: eased progress until the first
            // streamed event or the terminal event arrives.
            let synthetic_started = std::time::Instant::now();
            let synthetic_active =
                std::sync::Arc::new(std::sync::atomic::AtomicBool::new(synthetic));
            let synthetic_task = cx.spawn({
                let this = this.clone();
                let session_id = session_id.clone();
                let active = synthetic_active.clone();
                async move |cx| {
                    if !active.load(std::sync::atomic::Ordering::Relaxed) {
                        return;
                    }
                    let mut percentage = crate::batch::synthetic_batch_progress(0.0);
                    loop {
                        if this
                            .update(cx, |this, cx| {
                                if let Some(batch) = this.recording.batch.get_mut(&session_id)
                                    && batch.error.is_none()
                                {
                                    batch.percentage = Some(percentage);
                                    cx.notify();
                                }
                            })
                            .is_err()
                        {
                            return;
                        }
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(
                                crate::batch::SYNTHETIC_BATCH_PROGRESS_INTERVAL_MS,
                            ))
                            .await;
                        if !active.load(std::sync::atomic::Ordering::Relaxed) {
                            return;
                        }
                        percentage = crate::batch::synthetic_batch_progress(
                            synthetic_started.elapsed().as_millis() as f64,
                        );
                    }
                }
            });
            let mut settled = false;
            while let Some(event) = events_rx.recv().await {
                if settled {
                    break;
                }
                match event {
                    anlg_listener2_core::BatchEvent::BatchStarted { .. } => {}
                    anlg_listener2_core::BatchEvent::BatchResponseStreamed { event, .. } => {
                        synthetic_active.store(false, std::sync::atomic::Ordering::Relaxed);
                        let percentage = event.percentage();
                        this.update(cx, |this, cx| {
                            if let Some(batch) = this.recording.batch.get_mut(&session_id) {
                                batch.percentage = Some(percentage);
                                cx.notify();
                            }
                        })
                        .ok();
                    }
                    anlg_listener2_core::BatchEvent::BatchResponse { response, .. } => {
                        settled = true;
                        synthetic_active.store(false, std::sync::atomic::Ordering::Relaxed);
                        let words = crate::batch::transform_batch(&response);
                        if words.is_empty() {
                            let after = batch_run.after.clone();
                            this.update(cx, |this, cx| {
                                this.fail_batch(
                                    session_id.clone(),
                                    crate::batch::EMPTY_BATCH_TRANSCRIPT_ERROR.to_string(),
                                    cx,
                                );
                                this.after_lifecycle_batch_failed(
                                    &session_id,
                                    crate::batch::EMPTY_BATCH_TRANSCRIPT_ERROR,
                                    &after,
                                    cx,
                                );
                            })
                            .ok();
                            break;
                        }
                        // `prepareTranscriptPromotion`
                        let (words, replace_session, replace_transcript_id, started_at_ms) =
                            match batch_run.promotion.clone() {
                                BatchPromotion::WholeSession => (words, true, None, started_at_ms),
                                BatchPromotion::CurrentCapture {
                                    existing_audio_ms,
                                    replace_transcript_id,
                                    started_at_ms: capture_started_at_ms,
                                } => {
                                    let final_audio_ms = this
                                        .update(cx, |this, _| {
                                            this.store.audio_duration_ms(file_path.clone())
                                        })
                                        .ok();
                                    let final_audio_ms = match final_audio_ms {
                                        Some(task) => task.await.ok().flatten(),
                                        None => None,
                                    };
                                    let offset = current_capture_audio_offset_ms(
                                        existing_audio_ms,
                                        final_audio_ms,
                                    );
                                    (
                                        crate::batch::promote_current_capture(words, offset),
                                        false,
                                        replace_transcript_id,
                                        capture_started_at_ms,
                                    )
                                }
                            };
                        if words.is_empty() {
                            let after = batch_run.after.clone();
                            this.update(cx, |this, cx| {
                                this.fail_batch(
                                    session_id.clone(),
                                    crate::batch::EMPTY_CURRENT_CAPTURE_TRANSCRIPT_ERROR
                                        .to_string(),
                                    cx,
                                );
                                this.after_lifecycle_batch_failed(
                                    &session_id,
                                    crate::batch::EMPTY_CURRENT_CAPTURE_TRANSCRIPT_ERROR,
                                    &after,
                                    cx,
                                );
                            })
                            .ok();
                            break;
                        }
                        // `assertTranscriptNotTruncated` (#7480): a replacement
                        // that lost most of the saved text keeps the saved
                        // transcript and the recording; a failed read of the
                        // saved words fails the batch too.
                        let previous = this
                            .update(cx, |this, _| {
                                this.store.replaced_transcript_texts(
                                    session_id.clone(),
                                    replace_session,
                                    replace_transcript_id.clone(),
                                )
                            })
                            .ok();
                        let previous = match previous {
                            Some(task) => task.await.map_err(anyhow::Error::from).and_then(|r| r),
                            None => return,
                        };
                        let truncation = match previous {
                            Ok(previous_texts) => {
                                let previous: Vec<&str> =
                                    previous_texts.iter().map(String::as_str).collect();
                                let replacement: Vec<&str> =
                                    words.iter().map(|word| word.text.as_str()).collect();
                                crate::batch::transcript_truncated(&previous, &replacement).then(
                                    || crate::batch::INCOMPLETE_BATCH_TRANSCRIPT_ERROR.to_string(),
                                )
                            }
                            Err(error) => Some(error.to_string()),
                        };
                        if let Some(error) = truncation {
                            let after = batch_run.after.clone();
                            this.update(cx, |this, cx| {
                                this.fail_batch(session_id.clone(), error.clone(), cx);
                                this.after_lifecycle_batch_failed(&session_id, &error, &after, cx);
                            })
                            .ok();
                            break;
                        }
                        let (rows, hints) = crate::batch::stage_words(&words, &target.provider);
                        let transcript_id = uuid::Uuid::new_v4().to_string();
                        let write = this
                            .update(cx, |this, _| {
                                this.store.create_batch_transcript(
                                    transcript_id.clone(),
                                    session_id.clone(),
                                    created_at.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                                    started_at_ms,
                                    memo.clone(),
                                    target.provider.clone(),
                                    target.model.clone(),
                                    serde_json::Value::Array(rows).to_string(),
                                    serde_json::Value::Array(hints).to_string(),
                                    replace_session,
                                    replace_transcript_id,
                                )
                            })
                            .ok();
                        let written = match write {
                            Some(write) => write.await.map_err(anyhow::Error::from).and_then(|r| r),
                            None => return,
                        };
                        match written {
                            Ok(()) => {
                                // `maybeExtractVoiceprintCandidates` before the
                                // audio is marked processed.
                                let extract = this
                                    .update(cx, |this, _| {
                                        crate::voiceprint::maybe_extract_candidates(
                                            &this.store,
                                            remember_speakers,
                                            session_id.clone(),
                                            transcript_id.clone(),
                                            Some(file_path.to_string_lossy().into_owned()),
                                            mic_isolated,
                                        )
                                    })
                                    .ok();
                                if let Some(extract) = extract {
                                    let _ = extract.await;
                                }
                                // `deferAudioFinalization`: the capture
                                // lifecycle marks the audio itself.
                                if matches!(batch_run.after, BatchFollowUp::Standalone) {
                                    let mark = this
                                        .update(cx, |this, _| {
                                            this.store.mark_session_audio_transcription_complete(
                                                session_id.clone(),
                                            )
                                        })
                                        .ok();
                                    if let Some(mark) = mark
                                        && let Ok(Err(error)) = mark.await
                                    {
                                        tracing::error!(
                                            %error,
                                            "[runBatch] failed to mark session audio as processed"
                                        );
                                    }
                                }
                                let after = batch_run.after.clone();
                                this.update(cx, |this, cx| {
                                    // `clearBatchSession`
                                    this.recording.batch.remove(&session_id);
                                    if this.selected.as_deref() == Some(session_id.as_str()) {
                                        this.reload_note(session_id.clone(), cx);
                                    }
                                    this.recording.retranscribing.remove(&session_id);
                                    match after {
                                        // `triggerEnhanceIfSummaryEmpty`; then
                                        // `runBatch`'s completion notification,
                                        // cue and attention request.
                                        BatchFollowUp::Standalone => {
                                            this.queue_auto_enhance_if_summary_empty(
                                                session_id.clone(),
                                                cx,
                                            );
                                            this.notify_batch_completed(&session_id);
                                            this.play_completion_sound(cx);
                                            this.request_app_attention();
                                        }
                                        BatchFollowUp::CaptureLifecycle {
                                            summary_mode,
                                            live_transcript_id,
                                            audio_path,
                                            marker,
                                            live_active,
                                            recovery_attempt,
                                            ..
                                        } => {
                                            tracing::info!(
                                                session_id,
                                                "[listener] completed post-stop transcript repair"
                                            );
                                            // `notifyOnCompletion: !details.liveTranscriptionActive`
                                            if !live_active {
                                                this.notify_batch_completed(&session_id);
                                                this.play_completion_sound(cx);
                                                this.request_app_attention();
                                            }
                                            this.finish_capture(
                                                session_id.clone(),
                                                live_transcript_id,
                                                Some(audio_path),
                                                *marker,
                                                Some(summary_mode),
                                                recovery_attempt,
                                                cx,
                                            );
                                            // The awaited `runBatch` resolved
                                            // `finalizeStopped`.
                                            if recovery_attempt.is_none() {
                                                this.meeting_completed(&session_id);
                                            }
                                        }
                                    }
                                    cx.notify();
                                })
                                .ok();
                            }
                            Err(error) => {
                                tracing::error!(%error, "[runBatch] error handling batch response");
                                let after = batch_run.after.clone();
                                this.update(cx, |this, cx| {
                                    let message = error.to_string();
                                    this.fail_batch(session_id.clone(), message.clone(), cx);
                                    this.after_lifecycle_batch_failed(
                                        &session_id,
                                        &message,
                                        &after,
                                        cx,
                                    );
                                })
                                .ok();
                            }
                        }
                    }
                    anlg_listener2_core::BatchEvent::BatchCompleted { .. } => {}
                    anlg_listener2_core::BatchEvent::BatchFailed { error, .. } => {
                        settled = true;
                        synthetic_active.store(false, std::sync::atomic::Ordering::Relaxed);
                        let after = batch_run.after.clone();
                        this.update(cx, |this, cx| {
                            this.fail_batch(session_id.clone(), error.clone(), cx);
                            this.after_lifecycle_batch_failed(&session_id, &error, &after, cx);
                        })
                        .ok();
                    }
                }
            }
            drop(synthetic_task);
            let _ = run.await;
        })
        .detach();
    }

    /// `useConfigValue("remember_speakers")`
    fn remember_speakers(&self) -> bool {
        self.provider_settings.bool_setting(
            "remember_speakers",
            &["general", "remember_speakers"],
            true,
        )
    }

    /// The live capture ended (`Finalizing` or `Inactive`, whichever came
    /// first): remember its mic isolation and lifecycle inputs, then let the
    /// persistence queue write its tail and flush.
    fn end_live_capture(&mut self, live: LiveCapture, session_id: &str, cx: &mut Context<Self>) {
        self.recording
            .mic_isolation
            .persist(&self.store_file, session_id, live.mic_isolated);
        let marker = live.lifecycle.marker(
            session_id,
            &live.persistence,
            crate::capture_marker::Phase::Finalizing,
            None,
        );
        self.recording
            .pending_post_capture
            .entry(session_id.to_string())
            .or_default()
            .snapshot = Some((live.live_active, live.lifecycle.clone(), marker));
        // The engine keeps emitting deltas while it finalizes; the queue stays
        // open until `Inactive`, where `onStopped` awaits the flush.
        self.park_live_persistence(live.persistence, session_id.to_string(), cx);
    }

    /// `finalizeStopped`, once the transcript flush and the `Inactive`
    /// details have both arrived: `getPostCaptureAction` decides between the
    /// post-stop batch repair and the summary alone; the summary mode is
    /// `regenerate` when the capture extended an existing transcript. The
    /// marker moves to `finalizing` with the summary mode before the summary
    /// is requested and clears once the audio is marked processed.
    fn finalize_capture_when_ready(&mut self, session_id: &str, cx: &mut Context<Self>) {
        let ready = self
            .recording
            .pending_post_capture
            .get(session_id)
            .is_some_and(|pending| {
                pending.flush.is_some() && pending.snapshot.is_some() && pending.inactive.is_some()
            });
        if !ready {
            return;
        }
        let Some(pending) = self.recording.pending_post_capture.remove(session_id) else {
            return;
        };
        let (
            Some((transcript_id, transcript_created, flushed)),
            Some((live_active, lifecycle, marker)),
            Some(audio_path),
        ) = (pending.flush, pending.snapshot, pending.inactive)
        else {
            return;
        };
        let transcript_write_failed = !flushed;
        if !pending.catalogued
            && let Some(path) = audio_path.clone()
        {
            // `finalizeStopped`: after the flush, an empty automatic capture is
            // discarded before anything is catalogued; otherwise
            // `catalogLocalSessionAudio` runs and the finalization goes on.
            let discard = lifecycle
                .discard_candidate(live_active, transcript_write_failed)
                .then(|| {
                    self.store.discard_empty_automatic_capture(
                        session_id.to_string(),
                        lifecycle.initial_title.clone().unwrap_or_default(),
                    )
                });
            let catalog_store = self.store.clone();
            let session_id = session_id.to_string();
            let pending = PendingPostCapture {
                flush: Some((transcript_id.clone(), transcript_created, flushed)),
                snapshot: Some((live_active, lifecycle, marker)),
                inactive: Some(Some(path)),
                recovered_summary_mode: pending.recovered_summary_mode,
                recovery_attempt: pending.recovery_attempt,
                catalogued: true,
            };
            cx.spawn(async move |this, cx| {
                let discarded = match discard {
                    Some(task) => match task.await.map_err(anyhow::Error::from).and_then(|r| r) {
                        Ok(discarded) => discarded,
                        Err(error) => {
                            tracing::warn!(%error, "[listener] keeping automatic capture after an inconclusive activity check");
                            false
                        }
                    },
                    None => false,
                };
                if discarded {
                    tracing::info!(session_id, "[listener] discarded empty automatic capture");
                    let fresh_stop = pending.recovery_attempt.is_none();
                    this.update(cx, |this, cx| {
                        this.clear_capture_marker(session_id.clone(), transcript_id);
                        if fresh_stop {
                            this.meeting_completed(&session_id);
                        }
                        if this.selected.as_deref() == Some(session_id.as_str()) {
                            this.reload_note(session_id, cx);
                        }
                    })
                    .ok();
                    return;
                }
                // `onStopped` → `catalogLocalSessionAudio`: the primary audio
                // attachment row, `transcript_status: processing`.
                if let Ok(Err(error)) = catalog_store.catalog_session_audio(session_id.clone()).await
                {
                    tracing::error!(%error, "[listener] failed to catalog session audio");
                }
                this.update(cx, |this, cx| {
                    this.recording
                        .pending_post_capture
                        .insert(session_id.clone(), pending);
                    this.finalize_capture_when_ready(&session_id, cx);
                    if this.selected.as_deref() == Some(session_id.as_str()) {
                        this.reload_note(session_id, cx);
                    }
                })
                .ok();
            })
            .detach();
            return;
        }
        // `canRunBatchTranscription` is unconditional; a missing batch target
        // surfaces as the batch's own error.
        let action = match pending.recovered_summary_mode {
            Some(_) => PostCaptureAction::EnhanceOnly,
            None => post_capture_action(
                PostCaptureInputs {
                    has_audio: audio_path.is_some(),
                    live_transcription_active: live_active,
                    needs_batch_repair: lifecycle.needs_batch_repair,
                    refine_speaker_diarization: false,
                    transcript_write_failed,
                },
                true,
            ),
        };
        let session_id = session_id.to_string();
        match action {
            PostCaptureAction::BatchThenEnhance => {
                let Some(audio_path) = audio_path else {
                    return;
                };
                let mut marker = marker;
                // A fresh stop with the live text saved starts the summary at
                // once; the repair then regenerates it (`refreshSummaryAfterRepair`
                // survives a crash in the marker).
                if transcript_created
                    && !transcript_write_failed
                    && pending.recovery_attempt.is_none()
                {
                    marker.refresh_summary_after_repair = true;
                    tracing::info!(session_id, "[listener] starting live transcript summary");
                    drop(self.save_capture_marker(marker.clone()));
                    let flush = self.flush_memo_editor(&session_id, cx);
                    let live_mode = if lifecycle.preserve_existing_transcript {
                        super::enhance::AutoEnhanceMode::Regenerate
                    } else {
                        super::enhance::AutoEnhanceMode::IfEmpty
                    };
                    let live_session_id = session_id.clone();
                    cx.spawn(async move |this, cx| {
                        if let Some(flush) = flush
                            && let Ok(Err(error)) = flush.await
                        {
                            tracing::warn!(%error, "[listener] failed to flush the memo before the live summary");
                        }
                        this.update(cx, |this, cx| {
                            this.request_auto_enhance_with(live_session_id, live_mode, cx)
                        })
                        .ok();
                    })
                    .detach();
                }
                tracing::info!(
                    session_id,
                    live_active,
                    needs_batch_repair = lifecycle.needs_batch_repair,
                    transcript_write_failed,
                    "[listener] starting post-stop transcript repair"
                );
                let summary_mode = if marker.refresh_summary_after_repair
                    || lifecycle.preserve_existing_transcript
                {
                    super::enhance::AutoEnhanceMode::Regenerate
                } else {
                    super::enhance::AutoEnhanceMode::IfEmpty
                };
                let promotion = if lifecycle.preserve_existing_transcript || transcript_created {
                    BatchPromotion::CurrentCapture {
                        existing_audio_ms: lifecycle.existing_audio_ms,
                        replace_transcript_id: transcript_created.then(|| transcript_id.clone()),
                        started_at_ms: marker.started_at,
                    }
                } else {
                    BatchPromotion::WholeSession
                };
                let connection = self.store.stt_connection(&self.provider_settings);
                cx.spawn(async move |this, cx| {
                    let connection = connection.await.ok().flatten();
                    this.update(cx, |this, cx| {
                        this.run_batch_with(
                            session_id,
                            connection,
                            BatchRun {
                                promotion,
                                after: BatchFollowUp::CaptureLifecycle {
                                    summary_mode,
                                    live_transcript_id: transcript_id,
                                    audio_path,
                                    marker: Box::new(marker),
                                    recovery_attempt: pending.recovery_attempt,
                                    live_active,
                                    transcript_write_failed,
                                },
                            },
                            cx,
                        )
                    })
                    .ok();
                })
                .detach();
            }
            PostCaptureAction::EnhanceOnly => {
                // `playCompletionSound` / `requestAppAttention` when the
                // capture produced or extended a transcript without a repair.
                if pending.recovered_summary_mode.is_none()
                    && (lifecycle.transcript_touched || lifecycle.preserve_existing_transcript)
                {
                    self.play_completion_sound(cx);
                    self.request_app_attention();
                }
                let has_transcript_evidence = pending.recovered_summary_mode.is_some()
                    || lifecycle.preserve_existing_transcript
                    || lifecycle.transcript_touched;
                let summary_mode = pending.recovered_summary_mode.or_else(|| {
                    has_transcript_evidence.then_some(
                        if lifecycle.preserve_existing_transcript && lifecycle.transcript_touched {
                            super::enhance::AutoEnhanceMode::Regenerate
                        } else {
                            super::enhance::AutoEnhanceMode::IfEmpty
                        },
                    )
                });
                self.finish_capture(
                    session_id.clone(),
                    transcript_id,
                    audio_path,
                    marker,
                    summary_mode,
                    pending.recovery_attempt,
                    cx,
                );
                if pending.recovery_attempt.is_none() {
                    self.meeting_completed(&session_id);
                }
            }
            PostCaptureAction::None => {
                // `transcriptIsComplete` only for an empty fresh capture; a
                // recording with neither transcript nor batch target keeps
                // its marker for the next recovery pass.
                let empty_fresh_capture = pending.recovery_attempt.is_none()
                    && audio_path.is_none()
                    && !lifecycle.transcript_touched
                    && !transcript_write_failed;
                if empty_fresh_capture {
                    self.clear_capture_marker(session_id.clone(), transcript_id);
                } else {
                    // `requestRecovery`: the recovery component retries the
                    // finalization with backoff until its budget runs out.
                    tracing::warn!(
                        session_id,
                        "[listener] capture ended without a complete transcript"
                    );
                    self.retry_capture_recovery(session_id.clone(), pending.recovery_attempt, cx);
                }
                // `finalizeStopped` resolved either way; a fresh stop dispatches.
                if pending.recovery_attempt.is_none() {
                    self.meeting_completed(&session_id);
                }
            }
        }
    }

    /// `finishCaptureSyncDeferral` + the summary request +
    /// `complete_session_audio`: the marker turns `finalizing` carrying the
    /// summary mode, the summary is scheduled, the audio is finished, and
    /// the marker clears.
    #[allow(clippy::too_many_arguments)]
    fn finish_capture(
        &mut self,
        session_id: String,
        transcript_id: String,
        audio_path: Option<String>,
        mut marker: crate::capture_marker::Marker,
        summary_mode: Option<super::enhance::AutoEnhanceMode>,
        recovery_attempt: Option<u32>,
        cx: &mut Context<Self>,
    ) {
        marker.phase = Some(crate::capture_marker::Phase::Finalizing);
        marker.summary_mode = summary_mode.map(summary_mode_marker);
        // The frontend awaits this write before it schedules the summary or
        // clears the marker: a failed save means the summary is not started,
        // the user hears about it, and the recovery retries the stop.
        let saved = self.store.save_capture_marker(marker);
        cx.spawn(async move |this, cx| {
            let saved = match saved.await {
                Ok(Ok(())) => Ok(()),
                Ok(Err(error)) => Err(error.to_string()),
                Err(error) => Err(error.to_string()),
            };
            this.update(cx, |this, cx| {
                if let Err(error) = saved {
                    tracing::error!(%error, "[listener] failed to persist summary recovery state");
                    if summary_mode.is_some() {
                        this.notify_capture_failure(
                            &session_id,
                            "The transcript was saved, but Anarlog could not start the summary. Try generating it again.",
                            recovery_attempt,
                            cx,
                        );
                        this.retry_capture_recovery(session_id, recovery_attempt, cx);
                        return;
                    }
                }
                if let Some(mode) = summary_mode {
                    this.request_auto_enhance_with(session_id.clone(), mode, cx);
                }
                match audio_path {
                    Some(audio_path) => {
                        this.complete_session_audio(session_id, transcript_id, audio_path, cx)
                    }
                    None => this.clear_capture_marker(session_id, transcript_id),
                }
            })
            .ok();
        })
        .detach();
    }

    /// `notifyFailure(message)`: a toast for a fresh stop's failure unless the
    /// note was deleted meanwhile; the recovery retries stay quiet.
    fn notify_capture_failure(
        &mut self,
        session_id: &str,
        message: &'static str,
        recovery_attempt: Option<u32>,
        cx: &mut Context<Self>,
    ) {
        if recovery_attempt.is_some() {
            return;
        }
        let deleted = self.store.session_deleted(session_id.to_string());
        cx.spawn(async move |this, cx| {
            if matches!(deleted.await, Ok(Ok(true))) {
                return;
            }
            this.update(cx, |this, cx| {
                this.flash(super::toast::FlashVariant::Error, message, cx)
            })
            .ok();
        })
        .detach();
    }

    fn save_capture_marker(
        &self,
        marker: crate::capture_marker::Marker,
    ) -> tokio::task::JoinHandle<()> {
        let task = self.store.save_capture_marker(marker);
        self.store.runtime().spawn(async move {
            if let Ok(Err(error)) = task.await {
                tracing::error!(%error, "[listener] failed to persist capture recovery state");
            }
        })
    }

    fn clear_capture_marker(&self, session_id: String, transcript_id: String) {
        let task = self.store.clear_capture_marker(session_id, transcript_id);
        self.store.runtime().spawn(async move {
            if let Ok(Err(error)) = task.await {
                tracing::error!(%error, "[listener] failed to clear capture recovery state");
            }
        });
    }

    /// `LiveCaptureRecovery` at launch: every `capture_lifecycle_pending:`
    /// marker without a running capture is finalized as a recovered stop
    /// (`recoverStopped`: live transcription inactive, batch repair needed).
    pub(crate) fn recover_captures(&mut self, cx: &mut Context<Self>) {
        let markers = self.store.load_capture_markers();
        cx.spawn(async move |this, cx| {
            let markers = match markers.await.map_err(anyhow::Error::from).and_then(|r| r) {
                Ok(markers) => markers,
                Err(error) => {
                    tracing::error!(%error, "[listener] failed to load capture recovery state");
                    return;
                }
            };
            for marker in markers {
                this.update(cx, |this, cx| this.recover_capture(marker, 1, cx))
                    .ok();
            }
        })
        .detach();
    }

    /// `useResumeListeningLifecycle` for a marker whose capture is gone:
    /// flush its journal, then run `finalizeStopped` with the recovered
    /// details.
    fn recover_capture(
        &mut self,
        marker: crate::capture_marker::Marker,
        attempt: u32,
        cx: &mut Context<Self>,
    ) {
        let session_id = marker.session_id.clone();
        if self.session_mode(&session_id) != SessionMode::Inactive
            || self
                .recording
                .pending_post_capture
                .contains_key(&session_id)
        {
            return;
        }
        tracing::info!(session_id, attempt, "[listener] recovering capture");
        let flush = self.store.flush_live_deltas(marker.transcript_id.clone());
        let exists = self.store.transcript_exists(marker.transcript_id.clone());
        let audio_path = anlg_fs_sync_core::audio::path(&self.store.session_dir(&session_id))
            .map(|path| path.to_string_lossy().into_owned());
        cx.spawn(async move |this, cx| {
            let flushed = matches!(flush.await, Ok(Ok(())));
            let created = matches!(exists.await, Ok(Ok(true)));
            this.update(cx, |this, cx| {
                let lifecycle = CaptureLifecycle {
                    preserve_existing_transcript: marker.preserve_existing_transcript,
                    existing_audio_ms: marker.audio_offset_ms,
                    needs_batch_repair: true,
                    transcript_touched: created,
                    owner_user_id: marker.owner_user_id.clone(),
                    initial_title: marker.initial_title.clone(),
                    automatic: marker.automatic == Some(true),
                    preserve_existing_audio: marker.preserve_existing_audio.unwrap_or(true),
                    requested_live: true,
                };
                let recovered_summary_mode = marker.summary_mode.map(|mode| match mode {
                    crate::capture_marker::SummaryMode::Regenerate => {
                        super::enhance::AutoEnhanceMode::Regenerate
                    }
                    crate::capture_marker::SummaryMode::IfEmpty => {
                        super::enhance::AutoEnhanceMode::IfEmpty
                    }
                });
                this.recording.pending_post_capture.insert(
                    session_id.clone(),
                    PendingPostCapture {
                        flush: Some((marker.transcript_id.clone(), created, flushed)),
                        snapshot: Some((false, lifecycle, marker)),
                        inactive: Some(audio_path),
                        recovered_summary_mode,
                        recovery_attempt: Some(attempt),
                        catalogued: false,
                    },
                );
                this.finalize_capture_when_ready(&session_id, cx);
            })
            .ok();
        })
        .detach();
    }

    /// `LiveCaptureSessionRecovery`'s retry: `CAPTURE_RECOVERY_BASE_RETRY_MS`
    /// doubling per attempt, abandoning (clearing the marker) after
    /// `CAPTURE_RECOVERY_MAX_ATTEMPTS`. A fresh capture's failure leaves the
    /// marker for the next launch instead.
    fn retry_capture_recovery(
        &mut self,
        session_id: String,
        attempt: Option<u32>,
        cx: &mut Context<Self>,
    ) {
        const BASE_RETRY: std::time::Duration = std::time::Duration::from_millis(2_000);
        const MAX_ATTEMPTS: u32 = 5;
        // `requestCaptureRecovery` from a fresh capture starts the recovery
        // component at attempt 1; a recovery attempt's failure retries.
        let attempt = attempt.unwrap_or(0);
        let markers = self.store.load_capture_markers();
        cx.spawn(async move |this, cx| {
            let Ok(Ok(markers)) = markers.await else {
                return;
            };
            let Some(marker) = markers
                .into_iter()
                .find(|marker| marker.session_id == session_id)
            else {
                return;
            };
            if attempt >= MAX_ATTEMPTS {
                tracing::error!(session_id, "[listener] abandoning capture recovery");
                this.update(cx, |this, _| {
                    this.clear_capture_marker(session_id.clone(), marker.transcript_id.clone())
                })
                .ok();
                return;
            }
            cx.background_executor()
                .timer(BASE_RETRY * 2u32.pow(attempt.saturating_sub(1)))
                .await;
            this.update(cx, |this, cx| this.recover_capture(marker, attempt + 1, cx))
                .ok();
        })
        .detach();
    }

    /// The tail of `finalizeStoppedInner` for a completed transcript with
    /// audio: `maybeExtractVoiceprintCandidates`,
    /// `markSessionAudioTranscriptionComplete`, then
    /// `deleteProcessedAudioForRetention`.
    fn complete_session_audio(
        &mut self,
        session_id: String,
        transcript_id: String,
        audio_path: String,
        cx: &mut Context<Self>,
    ) {
        let mic_isolated = self
            .recording
            .mic_isolation
            .get(&self.store_file, &session_id);
        let extract = crate::voiceprint::maybe_extract_candidates(
            &self.store,
            self.remember_speakers(),
            session_id.clone(),
            transcript_id.clone(),
            Some(audio_path),
            mic_isolated,
        );
        let mark = self
            .store
            .mark_session_audio_transcription_complete(session_id.clone());
        cx.spawn(async move |this, cx| {
            let _ = extract.await;
            if let Ok(Err(error)) = mark.await {
                tracing::error!(%error, session_id, "[listener] failed to mark session audio as processed");
                return;
            }
            this.update(cx, |this, cx| {
                // `clearCaptureLifecycleMarker`, then the retention policy.
                this.clear_capture_marker(session_id.clone(), transcript_id);
                this.delete_processed_audio_for_retention(session_id, cx)
            })
            .ok();
        })
        .detach();
    }

    /// `stopTranscription(sessionId)`: abort the batch job, then
    /// `handleBatchStopped` leaves the "Transcription stopped." error behind.
    /// A stopped post-stop repair also clears its capture marker (#7473), so
    /// the cancellation holds across a relaunch; the audio and the saved
    /// transcripts stay.
    pub(super) fn stop_transcription(&mut self, session_id: String, cx: &mut Context<Self>) {
        let Some(batch) = self.recording.batch.get_mut(&session_id) else {
            return;
        };
        if batch.error.is_some() {
            return;
        }
        if let Some(abort) = batch.abort.take() {
            abort.abort();
        }
        let lifecycle_transcript_id = batch.lifecycle_transcript_id.take();
        // `isStoppedTranscriptionError`: no failure toast for a stop.
        self.recording.retranscribing.remove(&session_id);
        self.fail_batch(session_id.clone(), "Transcription stopped.".to_string(), cx);
        if let Some(transcript_id) = lifecycle_transcript_id {
            self.clear_capture_marker(session_id, transcript_id);
        }
    }

    /// A failed post-stop repair (`finalizeStopped`'s `requestRecovery`):
    /// the marker stays and the recovery component retries with backoff. The
    /// stop that started it (`requestRecoveryOnFailure`) also raises
    /// `notifyFailure`'s toast, unless the note was deleted meanwhile.
    fn after_lifecycle_batch_failed(
        &mut self,
        session_id: &str,
        error: &str,
        after: &BatchFollowUp,
        cx: &mut Context<Self>,
    ) {
        if let BatchFollowUp::CaptureLifecycle {
            recovery_attempt,
            live_active,
            transcript_write_failed,
            marker,
            ..
        } = after
        {
            tracing::error!(session_id, "[listener] post-stop transcript repair failed");
            if recovery_attempt.is_none() {
                let message = if *transcript_write_failed || !*live_active {
                    "Anarlog could not finish saving the transcript. The recording was kept so you can try again."
                } else {
                    "Post-meeting transcription failed. The recording was kept so you can try again."
                };
                let deleted = self.store.session_deleted(session_id.to_string());
                cx.spawn(async move |this, cx| {
                    if matches!(deleted.await, Ok(Ok(true))) {
                        return;
                    }
                    this.update(cx, |this, cx| {
                        this.flash(super::toast::FlashVariant::Error, message, cx)
                    })
                    .ok();
                })
                .detach();
            }
            // `isTerminalTranscriptionError`: another attempt cannot fix it, so
            // the marker clears and the recovery stops.
            if crate::batch::is_terminal_transcription_error(error) {
                self.clear_capture_marker(session_id.to_string(), marker.transcript_id.clone());
            } else {
                self.retry_capture_recovery(session_id.to_string(), *recovery_attempt, cx);
            }
            // `finishPostStopProcessing` runs on the rejection too.
            if recovery_attempt.is_none() {
                self.meeting_completed(session_id);
            }
        }
    }

    /// `dispatchMeetingCompleted`: the `meeting.completed` webhooks and
    /// automations once a fresh stop's post-stop processing settled.
    fn meeting_completed(&self, session_id: &str) {
        tracing::info!(session_id, "meeting.completed");
        self.store
            .run_meeting_completed_automations(session_id.to_string());
    }

    /// `handleBatchFailed(sessionId, error)`
    fn fail_batch(&mut self, session_id: String, error: String, cx: &mut Context<Self>) {
        // `useRegenerateTranscript`'s catch: `handleBatchFailed` plus the toast
        // with the message as its description (a stop stays quiet).
        if self.recording.retranscribing.remove(&session_id) {
            self.flash_with_description(
                super::toast::FlashVariant::Error,
                "Re-transcription failed",
                error.clone(),
                cx,
            );
        }
        self.recording.batch.insert(
            session_id,
            BatchState {
                phase: BatchPhase::Transcribing,
                percentage: None,
                error: Some(error),
                abort: None,
                lifecycle_transcript_id: None,
            },
        );
        self.ensure_default_summary(cx);
        cx.notify();
    }

    /// Spawn the root actor once the window is up and pump its events.
    pub(crate) fn spawn_recorder(&mut self, cx: &mut Context<Self>) {
        let runtime = self.store.runtime().clone();
        let global_base = self.store.global_base().to_path_buf();
        let vault_base = self.store.vault_base().to_path_buf();
        let audio = cx.global::<crate::audio::Audio>().0.clone();
        cx.spawn(async move |this, cx| {
            let spawned = Recorder::spawn(runtime, global_base, vault_base, audio).await;
            let (recorder, mut events) = match spawned {
                Ok(spawned) => spawned,
                Err(error) => {
                    tracing::error!(%error, "failed_to_spawn_root_actor");
                    return;
                }
            };
            this.update(cx, |this, _| {
                this.recording.recorder = Some(Rc::new(recorder));
            })
            .ok();
            while let Some(event) = events.recv().await {
                if this
                    .update(cx, |this, cx| this.handle_recording_event(event, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    /// `startListening`: the capture params from the note and settings, then
    /// `start_capture`; on success the sidebar collapses and, without a
    /// transcription provider, the warning toast appears.
    pub(crate) fn start_listening(&mut self, session_id: String, cx: &mut Context<Self>) {
        self.start_listening_with(session_id, false, cx);
    }

    /// `useStartListeningState(sessionId, { automatic })`: `automatic` marks
    /// a scheduled meeting's auto-start, whose empty recording is discarded.
    pub(crate) fn start_listening_with(
        &mut self,
        session_id: String,
        automatic: bool,
        cx: &mut Context<Self>,
    ) {
        // `canStartLiveSession`: one capture at a time.
        if self.recording.live.is_some() || self.recording.starting {
            return;
        }
        // `existingAudioPromise`: a manual capture keeps prior audio; an
        // automatic one records whether there was any.
        let preserve_existing_audio = !automatic || self.store.audio_exists(&session_id);
        let Some(recorder) = self.recording.recorder.clone() else {
            self.flash(
                super::toast::FlashVariant::Error,
                "Anarlog could not safely start recording. Please try again.",
                cx,
            );
            return;
        };
        // `useSTTConnection`: the provider's base URL and credential-store
        // key; `None` (no `conn`) records without a transcription endpoint.
        let connection = self.store.stt_connection(&self.provider_settings);
        let requested_languages = self.transcription_languages();
        // `memoMd = session?.raw_md ?? ""`
        let memo = match &self.note {
            super::Note::Ready { preview, .. } if preview.session.id == session_id => {
                preview.memo_body.clone()
            }
            _ => String::new(),
        };
        let mic_device = self
            .provider_settings
            .string_setting("microphone_device", &["general", "microphone_device"])
            .filter(|device| !device.is_empty());
        // `getSessionKeywords({ sessionId, dictionaryTerms })`
        let keywords = self
            .store
            .session_keywords(session_id.clone(), self.dictionary_terms());
        // `useSessionParticipantHumanIds` / `session.user_id` /
        // `transcriptExistence` / `getExistingAudioDurationMs`.
        let context = self.store.capture_context(session_id.clone());
        self.recording.starting = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let connection = connection.await.ok().flatten();
            // `getLiveTranscriptionConfig`: a batch-only model records without a
            // live session, and a provider that cannot carry every language live
            // keeps the primary one.
            let live_config = crate::stt_capabilities::live_transcription_config(
                connection.as_ref().map(|c| c.provider.as_str()),
                connection.as_ref().map(|c| c.model.as_str()),
                &requested_languages,
            );
            let languages = live_config.languages.clone();
            let requested_live = live_config.mode == TranscriptionMode::Live;
            let keywords = keywords.await.unwrap_or_default();
            let context = match context.await.map_err(anyhow::Error::from).and_then(|r| r) {
                Ok(context) => context,
                Err(error) => {
                    // A missing context falls back to the empty inputs
                    // rather than blocking the capture.
                    tracing::warn!(%error, "[listener] failed to load capture context");
                    crate::db::CaptureContext::default()
                }
            };
            let has_provider = connection.is_some();
            let params = SessionParams {
                session_id: session_id.clone(),
                languages,
                onboarding: false,
                transcription_mode: live_config.mode,
                model: connection.as_ref().map(|c| c.model.clone()).unwrap_or_default(),
                base_url: connection.as_ref().map(|c| c.base_url.clone()).unwrap_or_default(),
                api_key: connection.as_ref().map(|c| c.api_key.clone()).unwrap_or_default(),
                keywords,
                mic_device,
                participant_human_ids: context.participant_human_ids.clone(),
                self_human_id: Some(context.owner_user_id.clone()).filter(|id| !id.is_empty()),
                speaker_assignments: Vec::new(),
            };
            let result = recorder.start(params).await;
            this.update(cx, |this, cx| {
                this.recording.starting = false;
                match result {
                    Ok(Ok(())) => {
                        this.recording.live = Some(LiveCapture {
                            session_id,
                            persistence: LivePersistence {
                                transcript_id: uuid::Uuid::new_v4().to_string(),
                                created_at: chrono::Utc::now()
                                    .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                                    .to_string(),
                                started_at_ms: chrono::Utc::now().timestamp_millis(),
                                memo,
                                provider: connection
                                    .as_ref()
                                    .map(|c| c.provider.clone())
                                    .unwrap_or_default(),
                                model: connection
                                    .as_ref()
                                    .map(|c| c.model.clone())
                                    .unwrap_or_default(),
                                created: false,
                                writing: false,
                                pending: Vec::new(),
                                finishing: false,
                            },
                            requested_live,
                            live_active: has_provider && requested_live,
                            error: None,
                            mic: 0.0,
                            speaker: 0.0,
                            muted: false,
                            segments: Vec::new(),
                            label_context: None,
                            mic_isolated: None,
                            stall_audible_seconds: 0,
                            final_stall_audible_seconds: 0,
                            transcription_stalled: false,
                            lifecycle: CaptureLifecycle {
                                preserve_existing_transcript: context.preserve_existing_transcript,
                                existing_audio_ms: context.existing_audio_ms,
                                needs_batch_repair: false,
                                transcript_touched: false,
                                owner_user_id: context.owner_user_id.clone(),
                                initial_title: context.initial_title.clone(),
                                automatic,
                                preserve_existing_audio,
                                requested_live,
                            },
                        });
                        // `lifecycle.persistMarker()`: the durable capture state
                        // a relaunch recovers from.
                        if let Some(live) = this.recording.live.as_ref() {
                            let marker = live.lifecycle.marker(
                                &live.session_id,
                                &live.persistence,
                                crate::capture_marker::Phase::Capturing,
                                None,
                            );
                            this.save_capture_marker(marker);
                        }
                        this.on_live_session_started(cx);
                        // `setLeftSidebarExpanded(false)`
                        this.sidebar_expanded = false;
                        // `Live transcription is using <primary>` when the provider
                        // dropped languages, `not configured` without a provider.
                        if has_provider
                            && let Some(primary) = live_config.languages.first()
                            && !live_config.omitted_languages.is_empty()
                        {
                            let name = |language: &anlg_language::Language| {
                                super::settings::base_language_display_name(&language.to_string())
                            };
                            let omitted = live_config
                                .omitted_languages
                                .iter()
                                .map(name)
                                .collect::<Vec<_>>()
                                .join(", ");
                            this.recording.toast = Some(RecordingToast {
                                title: format!("Live transcription is using {}", name(primary)).into(),
                                description: format!(
                                    "Live transcription won't include {omitted}. Audio is still being saved."
                                )
                                .into(),
                                action: "Change",
                            });
                        } else if !has_provider {
                            this.recording.toast = Some(RecordingToast {
                                title: "Live transcription is not configured".into(),
                                description: "Audio is being saved. Choose a transcription provider to ensure this recording can be transcribed.".into(),
                                action: "Configure",
                            });
                        }
                    }
                    Ok(Err(error)) => {
                        tracing::error!(?error, "[listener] failed to start recording");
                        this.flash(
                            super::toast::FlashVariant::Error,
                            "Anarlog could not safely start recording. Please try again.",
                            cx,
                        );
                    }
                    Err(error) => {
                        tracing::error!(%error, "[listener] failed to start recording");
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// `stopListening` → `stop_capture`; the session finalizes until the
    /// engine reports `inactive` with the encoded audio path.
    pub(crate) fn stop_listening(&mut self, cx: &mut Context<Self>) {
        let Some(recorder) = self.recording.recorder.clone() else {
            return;
        };
        if self.recording.live.is_none() {
            return;
        }
        // The tokio task runs to completion on its own; the lifecycle events
        // carry the outcome.
        drop(recorder.stop());
        cx.notify();
    }

    fn handle_recording_event(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::Lifecycle(SessionLifecycleEvent::Active {
                session_id,
                requested_transcription_mode,
                current_transcription_mode,
                error,
            }) => {
                let requested_live = requested_transcription_mode == TranscriptionMode::Live;
                let live_active = current_transcription_mode == TranscriptionMode::Live;
                // The listener runtime's tray updates on `Active`.
                {
                    let tray = cx.global::<crate::tray::Tray>();
                    tray.send(crate::tray::TrayCommand::StartDisabled(true));
                    tray.send(crate::tray::TrayCommand::Degraded(error.is_some()));
                    tray.send(crate::tray::TrayCommand::Recording(true));
                }
                match self.recording.live.as_mut() {
                    Some(live) if live.session_id == session_id => {
                        live.requested_live = requested_live;
                        live.live_active = live_active;
                        // `live.needsBatchRepair ||= requested && (!active || degraded)`
                        live.lifecycle.needs_batch_repair |=
                            requested_live && (!live_active || error.is_some());
                        live.error = error;
                    }
                    _ => {
                        self.recording.live = Some(LiveCapture {
                            session_id,
                            persistence: LivePersistence {
                                transcript_id: uuid::Uuid::new_v4().to_string(),
                                created_at: chrono::Utc::now()
                                    .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                                    .to_string(),
                                started_at_ms: chrono::Utc::now().timestamp_millis(),
                                memo: String::new(),
                                provider: String::new(),
                                model: String::new(),
                                created: false,
                                writing: false,
                                pending: Vec::new(),
                                finishing: false,
                            },
                            requested_live,
                            live_active,
                            error,
                            mic: 0.0,
                            speaker: 0.0,
                            muted: false,
                            segments: Vec::new(),
                            label_context: None,
                            mic_isolated: None,
                            stall_audible_seconds: 0,
                            final_stall_audible_seconds: 0,
                            transcription_stalled: false,
                            lifecycle: CaptureLifecycle::default(),
                        });
                        self.on_live_session_started(cx);
                    }
                }
            }
            Event::Lifecycle(SessionLifecycleEvent::Finalizing { session_id }) => {
                {
                    let tray = cx.global::<crate::tray::Tray>();
                    tray.send(crate::tray::TrayCommand::StartDisabled(false));
                    tray.send(crate::tray::TrayCommand::Recording(false));
                }
                if let Some(live) = self
                    .recording
                    .live
                    .take_if(|live| live.session_id == session_id)
                {
                    self.end_live_capture(live, &session_id, cx);
                }
                self.recording.toast = None;
                if !self.recording.finalizing.contains(&session_id) {
                    self.recording.finalizing.push(session_id);
                }
            }
            Event::Lifecycle(SessionLifecycleEvent::Inactive {
                session_id,
                audio_path,
                error,
            }) => {
                {
                    let tray = cx.global::<crate::tray::Tray>();
                    tray.send(crate::tray::TrayCommand::StartDisabled(false));
                    tray.send(crate::tray::TrayCommand::Recording(false));
                    tray.send(crate::tray::TrayCommand::Degraded(false));
                }
                if let Some(live) = self
                    .recording
                    .live
                    .take_if(|live| live.session_id == session_id)
                {
                    self.end_live_capture(live, &session_id, cx);
                }
                self.finish_live_persistence(session_id.clone(), cx);
                self.recording.toast = None;
                self.recording.finalizing.retain(|id| *id != session_id);
                if let Some(error) = error {
                    tracing::error!(%error, "[listener] capture ended with an error");
                }
                // The audio is catalogued once the flush and the snapshot
                // are in, after the empty-automatic-capture check.
                self.recording
                    .pending_post_capture
                    .entry(session_id.clone())
                    .or_default()
                    .inactive = Some(audio_path);
                self.finalize_capture_when_ready(&session_id, cx);
            }
            Event::Progress(SessionProgressEvent::AudioReady { .. })
            | Event::Progress(SessionProgressEvent::AudioInitializing { .. })
            | Event::Progress(SessionProgressEvent::Connecting { .. })
            | Event::Progress(SessionProgressEvent::Connected { .. }) => {}
            Event::Error(error) => match error {
                anlg_listener_core::SessionErrorEvent::AudioError {
                    error, is_fatal, ..
                } => {
                    tracing::warn!(%error, is_fatal, "[listener] audio error");
                }
                anlg_listener_core::SessionErrorEvent::ConnectionError { error, .. } => {
                    tracing::warn!(%error, "[listener] connection error");
                    if let Some(live) = self.recording.live.as_mut()
                        && live.error.is_none()
                    {
                        live.error = Some(DegradedError::StreamError { message: error });
                    }
                }
            },
            Event::Data(SessionDataEvent::AudioAmplitude { mic, speaker, .. }) => {
                // `updateLiveAmplitude`: `clamp(value / 1000, 0, 1)`.
                if let Some(live) = self.recording.live.as_mut() {
                    live.mic = (f32::from(mic) / 1000.0).clamp(0.0, 1.0);
                    live.speaker = (f32::from(speaker) / 1000.0).clamp(0.0, 1.0);
                }
            }
            Event::Data(SessionDataEvent::MicMuted { value, .. }) => {
                if let Some(live) = self.recording.live.as_mut() {
                    live.muted = value;
                }
            }
            Event::Data(SessionDataEvent::MicIsolated { session_id, value }) => {
                if let Some(live) = self
                    .recording
                    .live
                    .as_mut()
                    .filter(|live| live.session_id == session_id)
                {
                    live.mic_isolated = Some(crate::voiceprint::merge_mic_isolation(
                        live.mic_isolated,
                        value,
                    ));
                }
            }
            Event::Data(SessionDataEvent::TranscriptSegmentDelta { session_id, delta }) => {
                if let Some(live) = self
                    .recording
                    .live
                    .as_mut()
                    .filter(|live| live.session_id == session_id)
                {
                    let delta = *delta;
                    super::floating_bar::apply_segment_delta(
                        &mut live.segments,
                        delta.upserts,
                        &delta.removed_ids,
                    );
                }
            }
            Event::Data(SessionDataEvent::TranscriptDelta { session_id, delta }) => {
                // `noteLiveTranscriptActivity`: partials prove the stream is
                // alive; finalized words mark the pipeline healthy again and
                // put the stalled warning away (#7466).
                let has_final_words = !delta.new_words.is_empty();
                if (has_final_words || !delta.partials.is_empty())
                    && let Some(live) = self
                        .recording
                        .live
                        .as_mut()
                        .filter(|live| live.session_id == session_id)
                {
                    live.stall_audible_seconds = 0;
                    if has_final_words {
                        live.final_stall_audible_seconds = 0;
                        if live.transcription_stalled {
                            live.transcription_stalled = false;
                            self.dismiss_flash_with_id(STALLED_TOAST_ID, cx);
                        }
                    }
                }
                // `handlePersist`: empty deltas are ignored; the rest are
                // queued whether the capture is live or already finalizing.
                if delta.new_words.is_empty() && delta.replaced_ids.is_empty() {
                    return;
                }
                if let Some(live) = self
                    .recording
                    .live
                    .as_mut()
                    .filter(|live| live.session_id == session_id)
                {
                    live.lifecycle.transcript_touched = true;
                } else if let Some((_, lifecycle, _)) = self
                    .recording
                    .pending_post_capture
                    .get_mut(&session_id)
                    .and_then(|pending| pending.snapshot.as_mut())
                {
                    lifecycle.transcript_touched = true;
                }
                if let Some(persistence) = self.persistence_mut(&session_id) {
                    persistence.pending.push(*delta);
                    self.drain_live_persistence(session_id, cx);
                }
            }
            Event::Data(_) => {}
        }
        self.sync_floating_bar(cx);
        cx.notify();
    }

    /// `createLiveSecondsInterval`: once a second while this capture is live,
    /// run the stall watchdog.
    pub(crate) fn start_transcription_stall_watchdog(&mut self, cx: &mut Context<Self>) {
        let Some(session_id) = self
            .recording
            .live
            .as_ref()
            .map(|live| live.session_id.clone())
        else {
            return;
        };
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(1))
                    .await;
                let alive = this
                    .update(cx, |this, cx| {
                        this.tick_transcription_stall_watchdog(&session_id, cx)
                    })
                    .unwrap_or(false);
                if !alive {
                    break;
                }
            }
        })
        .detach();
    }

    /// `tickTranscriptionStallWatchdog`: live STT can hang without any error
    /// or degraded event — nothing arrives, or partials never finalize. Count
    /// audible seconds since the last activity of each kind; past either
    /// threshold, flag the session so the stop runs a batch repair from the
    /// recording, and warn. Returns whether the capture is still live.
    fn tick_transcription_stall_watchdog(
        &mut self,
        session_id: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(live) = self
            .recording
            .live
            .as_mut()
            .filter(|live| live.session_id == session_id)
        else {
            return false;
        };
        if !live.requested_live || !live.live_active || live.transcription_stalled {
            return true;
        }
        // Mic mute must not disable the watchdog: speaker audio keeps feeding
        // live STT, and the amplitude gate ignores silent stretches.
        if live.mic.max(live.speaker) < TRANSCRIPTION_STALL_AMPLITUDE_THRESHOLD {
            return true;
        }
        live.stall_audible_seconds += 1;
        live.final_stall_audible_seconds += 1;
        if live.stall_audible_seconds < TRANSCRIPTION_STALL_AUDIBLE_SECONDS
            && live.final_stall_audible_seconds < TRANSCRIPTION_FINAL_STALL_AUDIBLE_SECONDS
        {
            return true;
        }
        live.transcription_stalled = true;
        live.lifecycle.needs_batch_repair = true;
        tracing::warn!(session_id, "[listener] live transcription stalled");
        // `notifyTranscriptionStalled`: `sonnerToast.warning` with
        // `duration: Infinity`.
        self.flash_persistent(
            super::toast::FlashVariant::Warning,
            STALLED_TOAST_ID,
            "Live transcription stalled",
            "Anarlog keeps recording while live transcription reconnects. Any missing text will be rebuilt from the recording after you stop listening.",
            cx,
        );
        true
    }

    /// `FloatingMeetingWindowSync`: while `floating_bar_enabled` holds and a
    /// live session is active the floating bar window shows with the current
    /// `FloatingRouteState`; otherwise it hides.
    pub(crate) fn sync_floating_bar(&mut self, cx: &mut Context<Self>) {
        let enabled = self.provider_settings.bool_setting(
            "floating_bar_enabled",
            &["general", "floating_bar_enabled"],
            true,
        );
        let state = self
            .recording
            .live
            .as_ref()
            .filter(|_| enabled)
            .map(|live| super::floating_bar::FloatingBarState {
                // `Math.min(Math.hypot(mic, speaker), 1)`
                amplitude: live.mic.hypot(live.speaker).min(1.0),
                // `getFloatingSessionTitle` reads the session row; the open
                // note's title covers the moment before that row loads.
                title: super::floating_bar::floating_title(
                    live.label_context
                        .as_ref()
                        .and_then(|ctx| ctx.title.as_deref())
                        .or(self.note_title_for(&live.session_id).as_deref()),
                ),
                error: live.error.is_some() || live.degraded(),
                dark: self.theme.dark,
                opacity: self
                    .provider_settings
                    .string_setting("floating_bar_opacity", &["general", "floating_bar_opacity"])
                    .and_then(|value| value.parse::<f32>().ok())
                    .unwrap_or(0.78),
                // `shouldShowFloatingLiveCaptionToggle({ liveTranscriptionActive })`
                live_caption_toggle_visible: live.live_active,
                live_caption_minimized: self.provider_settings.bool_setting(
                    "live_caption_minimized",
                    &["general", "live_caption_minimized"],
                    true,
                ),
                transcript_bubbles: super::floating_bar::transcript_bubbles(
                    &live.segments,
                    live.label_context.as_ref(),
                ),
            });
        match (state, self.recording.floating_bar.take()) {
            (Some(state), Some(handle)) => {
                let previous = handle
                    .update(cx, |bar, _, _| bar.state.container_size())
                    .ok();
                if previous.is_some_and(|size| size != state.container_size()) {
                    self.recording.floating_bar =
                        super::floating_bar::reopen_resized(handle, cx.weak_entity(), state, cx);
                    return;
                }
                let updated = handle
                    .update(cx, |bar, _, cx| {
                        if bar.state != state {
                            bar.state = state;
                            cx.notify();
                        }
                    })
                    .is_ok();
                if updated {
                    self.recording.floating_bar = Some(handle);
                }
            }
            (Some(state), None) => {
                self.recording.floating_bar =
                    super::floating_bar::show(cx.weak_entity(), state, cx);
            }
            (None, Some(handle)) => {
                handle
                    .update(cx, |_, window, _| window.remove_window())
                    .ok();
            }
            (None, None) => {}
        }
    }

    /// `LiveCaptionDefaultVisibilitySync` (a new live session starts with the
    /// panel minimized) and the `MeetingFloatData` load for its labels.
    fn on_live_session_started(&mut self, cx: &mut Context<Self>) {
        if let Some(session_id) = self
            .recording
            .live
            .as_ref()
            .map(|live| live.session_id.clone())
        {
            self.enhancer_on_live_started(&session_id);
        }
        self.start_transcription_stall_watchdog(cx);
        let Some(session_id) = self
            .recording
            .live
            .as_ref()
            .map(|live| live.session_id.clone())
        else {
            return;
        };
        if !self.provider_settings.bool_setting(
            "live_caption_minimized",
            &["general", "live_caption_minimized"],
            true,
        ) {
            self.set_live_caption_minimized(true, cx);
        }
        self.reload_float_label_context(session_id, cx);
    }

    /// `subscribeMeetingFloatData`: the session title, owner, participants and
    /// human names behind the panel's speaker labels.
    pub(crate) fn reload_float_label_context(
        &mut self,
        session_id: String,
        cx: &mut Context<Self>,
    ) {
        let task = self.store.meeting_float_context(session_id.clone());
        cx.spawn(async move |this, cx| {
            let Ok(Ok(context)) = task.await else {
                return;
            };
            this.update(cx, |this, cx| {
                if let Some(live) = this
                    .recording
                    .live
                    .as_mut()
                    .filter(|live| live.session_id == session_id)
                    && live.label_context.as_ref() != Some(&context)
                {
                    live.label_context = Some(context);
                    this.sync_floating_bar(cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// `onToggleExpanded` → `floatingBarSettingsChange { liveCaptionMinimized }`.
    pub(crate) fn set_live_caption_minimized(&mut self, minimized: bool, cx: &mut Context<Self>) {
        self.set_bool_setting("live_caption_minimized", minimized, cx);
        self.sync_floating_bar(cx);
    }

    fn note_title_for(&self, session_id: &str) -> Option<String> {
        match &self.note {
            super::Note::Ready { preview, .. } if preview.session.id == session_id => {
                Some(preview.session.title.clone()).filter(|title| !title.trim().is_empty())
            }
            _ => None,
        }
    }

    fn persistence_mut(&mut self, session_id: &str) -> Option<&mut LivePersistence> {
        if let Some(live) = self
            .recording
            .live
            .as_mut()
            .filter(|live| live.session_id == session_id)
        {
            return Some(&mut live.persistence);
        }
        self.recording
            .flushing
            .iter_mut()
            .find(|(id, _)| id == session_id)
            .map(|(_, persistence)| persistence)
    }

    /// The persistence worker's drain: coalesce the pending deltas into one
    /// write — `createLiveTranscript` first, `applyLiveTranscriptDeltaToDatabase`
    /// after — keep draining while more arrive, and once a finished capture's
    /// queue is empty run `flushLiveTranscriptDeltasToDatabase`.
    fn drain_live_persistence(&mut self, session_id: String, cx: &mut Context<Self>) {
        let Some(persistence) = self.persistence_mut(&session_id) else {
            return;
        };
        if persistence.writing {
            return;
        }
        if persistence.pending.is_empty() {
            if !persistence.finishing {
                return;
            }
            let transcript_id = persistence.transcript_id.clone();
            // `hasTranscriptEvidence`: a transcript row was written.
            let has_transcript = persistence.created;
            self.recording.flushing.retain(|(id, _)| *id != session_id);
            let flush = self.store.flush_live_deltas(transcript_id.clone());
            cx.spawn(async move |this, cx| {
                let flushed = match flush.await {
                    Ok(Err(error)) => {
                        tracing::error!(%error, "[listener] failed to flush live transcript");
                        false
                    }
                    Err(error) => {
                        tracing::error!(%error, "[listener] failed to flush live transcript");
                        false
                    }
                    Ok(Ok(())) => true,
                };
                this.update(cx, |this, cx| {
                    if this.selected.as_deref() == Some(session_id.as_str()) {
                        this.reload_note(session_id.clone(), cx);
                    }
                    // `transcriptPersistence.flush()` done: `finalizeStopped`
                    // continues once the engine's `Inactive` details arrive.
                    this.recording
                        .pending_post_capture
                        .entry(session_id.clone())
                        .or_default()
                        .flush = Some((transcript_id, has_transcript, flushed));
                    this.finalize_capture_when_ready(&session_id, cx);
                })
                .ok();
            })
            .detach();
            return;
        }
        let deltas = std::mem::take(&mut persistence.pending);
        let delta = crate::live_transcript::coalesce_deltas(&deltas);
        persistence.writing = true;
        let transcript_id = persistence.transcript_id.clone();
        let created = persistence.created;
        let (created_at, started_at_ms, memo, provider, model) = (
            persistence.created_at.clone(),
            persistence.started_at_ms,
            persistence.memo.clone(),
            persistence.provider.clone(),
            persistence.model.clone(),
        );
        let task = if created {
            self.store.journal_live_delta(transcript_id, delta)
        } else {
            self.store.create_live_transcript(
                transcript_id,
                session_id.clone(),
                created_at,
                started_at_ms,
                memo,
                provider,
                model,
                delta,
            )
        };
        cx.spawn(async move |this, cx| {
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update(cx, |this, cx| {
                if let Err(error) = &result {
                    tracing::error!(%error, "[listener] failed to persist transcript");
                }
                if let Some(persistence) = this.persistence_mut(&session_id) {
                    persistence.writing = false;
                    if result.is_ok() {
                        persistence.created = true;
                    }
                }
                this.drain_live_persistence(session_id.clone(), cx);
                if this.selected.as_deref() == Some(session_id.as_str()) {
                    this.reload_note(session_id.clone(), cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// The capture is finalizing: its queue moves aside, still taking the
    /// engine's late deltas, and the drain keeps writing them.
    fn park_live_persistence(
        &mut self,
        persistence: LivePersistence,
        session_id: String,
        cx: &mut Context<Self>,
    ) {
        self.recording.flushing.retain(|(id, _)| *id != session_id);
        self.recording
            .flushing
            .push((session_id.clone(), persistence));
        self.drain_live_persistence(session_id, cx);
    }

    /// The engine is `Inactive`: no more deltas will come, so the queue is
    /// marked finishing and the drain flushes the journal once it is empty
    /// (`transcriptPersistence.flush()` in `onStopped`).
    fn finish_live_persistence(&mut self, session_id: String, cx: &mut Context<Self>) {
        let Some((_, persistence)) = self
            .recording
            .flushing
            .iter_mut()
            .find(|(id, _)| *id == session_id)
        else {
            return;
        };
        persistence.finishing = true;
        self.drain_live_persistence(session_id, cx);
    }

    /// `getTranscriptionLanguages(aiLanguage, spokenLanguages)`: the AI
    /// language first, then the distinct spoken languages, all as base codes.
    /// `getTranscriptionLanguages(aiLanguage, spokenLanguages)`: the full
    /// codes (`en-US`, not `en`), first occurrence per base language.
    pub(super) fn transcription_languages(&self) -> Vec<anlg_language::Language> {
        let ai = self
            .provider_settings
            .string_setting("ai_language", &["language", "ai_language"])
            .unwrap_or_else(|| "en".to_string());
        let spoken = self
            .provider_settings
            .string_setting("spoken_languages", &["language", "spoken_languages"])
            .and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok())
            .unwrap_or_default();
        let mut seen = std::collections::HashSet::new();
        std::iter::once(ai)
            .chain(spoken)
            .filter(|code| !code.is_empty())
            .filter(|code| {
                let base = super::settings::base_language_code(code);
                !base.is_empty() && seen.insert(base)
            })
            .filter_map(|code| code.parse::<anlg_language::Language>().ok())
            .collect()
    }

    /// `HeaderViewTranscriptLiveIcon` → `DancingSticks` at 16×16, amber
    /// while degraded, the waveform while muted.
    pub(super) fn render_dancing_sticks(&self, live: &LiveCapture) -> AnyElement {
        let color = if live.degraded() {
            gpui::rgb(0xfd9a00)
        } else {
            gpui::rgb(0xfb2c36)
        };
        if live.muted {
            return crate::ui::icon("audio-lines", px(16.0), self.theme.foreground)
                .into_any_element();
        }
        dancing_sticks(
            live.mic.hypot(live.speaker).min(1.0),
            color,
            16.0,
            16.0,
            2.0,
            1.0,
        )
    }

    /// The transcript tab body while `getSessionMode` is not inactive, after
    /// `useTranscriptScreen`: `batch_fallback` (`BatchState`) when the
    /// capture is not transcribing live, else `listening` / `finalizing`
    /// (`TranscriptListeningState`).
    pub(super) fn render_live_transcript_screen(
        &self,
        session_id: &str,
        has_words: bool,
        window: &gpui::Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let theme = self.theme;
        let mode = self.session_mode(session_id);
        if let Some(batch) = self.batch_state(session_id) {
            if let Some(error) = &batch.error {
                // `useTranscriptScreen`: with words the viewer stays (`ready`);
                // the `TranscriptEmptyState` with `error` is for a session
                // without them.
                if has_words {
                    return None;
                }
                return Some(
                    transcript_screen()
                        .child(div().mb_5().child(crate::ui::icon(
                            "warning-circle",
                            px(36.0),
                            theme.muted_foreground,
                        )))
                        .child(self.transcript_screen_copy(
                            "Transcription failed",
                            error,
                            24.0,
                            window,
                        ))
                        .child(self.transcript_icon_button(
                            "transcript-retranscribe",
                            Some(("arrows-clockwise", 16.0)),
                            "Re-transcribe",
                            true,
                            cx,
                            |this, _, cx| this.retranscribe(cx),
                        ))
                        .into_any_element(),
                );
            }
            // `running_batch`
            let has_progress = batch.percentage.is_some_and(|p| p > 0.0);
            // `onStopTranscription` is withheld while importing.
            let can_stop = batch.phase == BatchPhase::Transcribing;
            let session_id = session_id.to_string();
            return Some(
                transcript_screen()
                    .child(div().mb_5().child(crate::ui::icon(
                        "circle-notch",
                        px(36.0),
                        theme.muted_foreground,
                    )))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .when(can_stop, |c| c.mb_6())
                            .child(
                                div()
                                    .tw_text_base()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(theme.foreground)
                                    .child(match batch.phase {
                                        BatchPhase::Importing => "Importing audio...",
                                        BatchPhase::Transcribing => "Generating transcript...",
                                    }),
                            )
                            .when(has_progress, |c| {
                                c.child(
                                    div()
                                        .mt_2()
                                        .tw_text_sm()
                                        .line_height(px(22.0))
                                        .text_color(theme.muted_foreground)
                                        .child(SharedString::from(format!(
                                            "{}% complete",
                                            (batch.percentage.unwrap_or(0.0) * 100.0).round()
                                                as i64
                                        ))),
                                )
                            }),
                    )
                    .when(can_stop, |screen| {
                        screen.child(self.transcript_icon_button(
                            "transcript-stop-transcription",
                            Some(("square", 12.0)),
                            "Stop transcription",
                            false,
                            cx,
                            move |this, _, cx| this.stop_transcription(session_id.clone(), cx),
                        ))
                    })
                    .into_any_element(),
            );
        }
        if mode == SessionMode::Inactive {
            return None;
        }
        // `hasVisibleTranscriptState`: once words exist the viewer renders

        // them (the `ready` screen) unless the capture fell back to batch.
        let live_transcribing = self
            .recording
            .live
            .as_ref()
            .is_none_or(|live| live.session_id != session_id || live.live_active);
        if has_words && live_transcribing {
            return None;
        }
        let live = self
            .recording
            .live
            .as_ref()
            .filter(|live| live.session_id == session_id);
        let copy = |title: String, description: String| {
            self.transcript_screen_copy(&title, &description, 0.0, window)
        };
        if let Some(live) = live.filter(|live| !live.live_active) {
            // `BatchState`
            let fallback = live.requested_live;
            let reconnecting = fallback
                && live.error.as_ref().is_some_and(|error| {
                    !matches!(
                        error,
                        DegradedError::AuthenticationFailed { .. }
                            | DegradedError::ProviderConfiguration { .. }
                    )
                });
            let title = if fallback {
                if reconnecting {
                    "Reconnecting live transcription"
                } else if live.error.is_some() {
                    "Live transcription stopped"
                } else {
                    "Live transcription unavailable"
                }
            } else {
                "Batch transcription mode"
            };
            let description = if fallback {
                format!(
                    "{}Recording continues{}. A complete transcript will be generated after you stop.",
                    live.error
                        .as_ref()
                        .map(|error| format!("{}. ", degraded_message(error)))
                        .unwrap_or_default(),
                    if reconnecting {
                        " while we reconnect"
                    } else {
                        ""
                    }
                )
            } else {
                "Recording continues. Your transcript will be generated after you stop.".to_string()
            };
            return Some(
                transcript_screen()
                    .child(div().mb_5().child(dancing_sticks(
                        live.mic.hypot(live.speaker).min(1.0),
                        gpui::rgb(0xa3a3a3),
                        36.0,
                        80.0,
                        3.0,
                        3.0,
                    )))
                    .child(copy(title.to_string(), description))
                    .into_any_element(),
            );
        }
        // `TranscriptListeningState`
        let finalizing = mode == SessionMode::Finalizing;
        Some(
            transcript_screen()
                .child(div().mb_5().child(if finalizing {
                    crate::ui::icon("circle-notch", px(36.0), theme.muted_foreground)
                } else {
                    crate::ui::icon("waveform", px(36.0), theme.muted_foreground)
                }))
                .child(copy(
                    if finalizing {
                        "Finalizing transcript..."
                    } else {
                        "Listening..."
                    }
                    .to_string(),
                    if finalizing {
                        "Transcript is still being written."
                    } else {
                        "Transcript will appear here when the first segment arrives."
                    }
                    .to_string(),
                ))
                .into_any_element(),
        )
    }

    /// `TranscriptEmptyState` without a batch: `Audio available` with
    /// Re-transcribe and Upload transcript when the session has audio.
    pub(super) fn render_transcript_empty_state(
        &self,
        has_audio: bool,
        window: &gpui::Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        transcript_screen()
            .child(div().mb_5().child(crate::ui::icon(
                "waveform",
                px(36.0),
                theme.muted_foreground,
            )))
            .child(self.transcript_screen_copy(
                if has_audio {
                    "Audio available"
                } else {
                    "No transcript available"
                },
                if has_audio {
                    "Re-transcribe this audio, or upload a transcript file."
                } else {
                    "Upload audio or a transcript file to populate this note."
                },
                24.0,
                window,
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(has_audio, |row| {
                        row.child(self.transcript_icon_button(
                            "transcript-retranscribe",
                            Some(("arrows-clockwise", 16.0)),
                            "Re-transcribe",
                            true,
                            cx,
                            |this, _, cx| this.retranscribe(cx),
                        ))
                    })
                    .when(!has_audio, |row| {
                        row.child(self.transcript_button(
                            "transcript-upload-audio",
                            "Upload audio",
                            false,
                            cx,
                            |this, window, cx| this.upload_audio(window, cx),
                        ))
                    })
                    .child(self.transcript_button(
                        "transcript-upload",
                        "Upload transcript",
                        false,
                        cx,
                        |this, window, cx| this.upload_transcript(window, cx),
                    )),
            )
            .into_any_element()
    }

    /// `Button size="sm"` (`h-7 px-2 text-xs`) with the screens' `gap-2`,
    /// default or outline.
    fn transcript_button(
        &self,
        id: &'static str,
        label: &'static str,
        primary: bool,
        cx: &Context<Self>,
        on_click: impl Fn(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static,
    ) -> gpui::Stateful<Div> {
        self.transcript_icon_button(id, None, label, primary, cx, on_click)
    }

    /// `Button size="sm"` with an optional leading icon of the given size.
    fn transcript_icon_button(
        &self,
        id: &'static str,
        icon: Option<(&'static str, f32)>,
        label: &'static str,
        primary: bool,
        cx: &Context<Self>,
        on_click: impl Fn(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static,
    ) -> gpui::Stateful<Div> {
        let theme = self.theme;
        let foreground = if primary {
            theme.primary_foreground
        } else {
            theme.foreground
        };
        div()
            .id(id)
            .relative()
            .flex()
            .h(px(28.0))
            .items_center()
            .gap_2()
            .px_2()
            .tw_text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .cursor_pointer()
            .child(crate::squircle::squircle(
                crate::squircle::CONTROL_RADIUS,
                Some(if primary {
                    theme.primary
                } else {
                    theme.background
                }),
                (!primary).then_some((1.0, theme.border)),
            ))
            .text_color(foreground)
            .on_click(
                cx.listener(move |this, _: &gpui::ClickEvent, window, cx| {
                    on_click(this, window, cx)
                }),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .items_center()
                    .gap_2()
                    .children(icon.map(|(icon, size)| crate::ui::icon(icon, px(size), foreground)))
                    .child(label),
            )
    }
    /// `flex max-w-md flex-col gap-2` with the `text-base font-medium` title
    /// and the centred `text-sm leading-relaxed` description, wrapped the
    /// way WebKit wraps it.
    fn transcript_screen_copy(
        &self,
        title: &str,
        description: &str,
        margin_bottom: f32,
        window: &gpui::Window,
    ) -> Div {
        let theme = self.theme;
        let mut style = window.text_style();
        style.font_size = px(14.0).into();
        style.color = theme.muted_foreground.into();
        if let Some(font) = &self.font_family {
            style.font_family = font.clone();
        }
        let run = style.to_run(description.len());
        div()
            .flex()
            .w_full()
            .max_w(px(448.0))
            .mb(px(margin_bottom))
            .flex_col()
            .gap_2()
            .items_center()
            .child(
                div()
                    .tw_text_base()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.foreground)
                    .child(SharedString::from(title.to_string())),
            )
            .child(
                div().w_full().child(
                    crate::prose_text::ProseText::new(
                        description.to_string(),
                        vec![run],
                        px(14.0),
                        px(22.0),
                    )
                    .centered()
                    .pretty()
                    .max_width(px(448.0)),
                ),
            )
    }

    /// `useRegenerateTranscript` runs the batch pipeline over the stored
    /// audio, which needs a configured provider; the batch pipeline is not
    /// ported yet, so the missing-provider outcome is reported directly.
    /// `useRegenerateTranscript`: `fsSyncCommands.audioPath` first (its
    /// error toasts `Recording not found`), then the `whole_session` batch.
    pub(crate) fn retranscribe(&mut self, cx: &mut Context<Self>) {
        let Some(session_id) = self.selected.clone() else {
            return;
        };
        if !self.store.audio_exists(&session_id) {
            self.flash(
                super::toast::FlashVariant::Error,
                "Recording not found. It may have been deleted.",
                cx,
            );
            return;
        }
        self.recording.retranscribing.insert(session_id.clone());
        let connection = self.store.stt_connection(&self.provider_settings);
        cx.spawn(async move |this, cx| {
            let connection = connection.await.ok().flatten();
            this.update(cx, |this, cx| this.run_batch(session_id, connection, cx))
                .ok();
        })
        .detach();
    }

    /// The persistent sonner warning (`richColors`, `duration: Infinity`)
    /// with the title, description, `Configure` action, and close button.
    pub(super) fn render_recording_toast(&self, cx: &Context<Self>) -> Option<gpui::Stateful<Div>> {
        let toast = self.recording.toast.as_ref()?;
        let (background, border, text) = if self.theme.dark {
            (
                gpui::rgb(0x1d1f00),
                gpui::rgb(0x3d3d00),
                gpui::rgb(0xf3cf58),
            )
        } else {
            (
                gpui::rgb(0xfffcf0),
                gpui::rgb(0xfdf5d3),
                gpui::rgb(0xdc7609),
            )
        };
        Some(
            div()
                .id("recording-toast")
                .absolute()
                .right(px(32.0))
                .bottom(px(32.0))
                .w(px(300.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .px_4()
                .py_4()
                .rounded(px(8.0))
                .border_1()
                .border_color(border)
                .bg(background)
                .text_color(text)
                .shadow(vec![gpui::BoxShadow {
                    color: gpui::hsla(0.0, 0.0, 0.0, 0.1),
                    offset: gpui::point(px(0.0), px(4.0)),
                    blur_radius: px(12.0),
                    spread_radius: px(0.0),
                }])
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .flex()
                        .size(px(16.0))
                        .flex_shrink_0()
                        .items_center()
                        .ml(px(-3.0))
                        .mr(px(4.0))
                        .child(crate::ui::icon("alert-triangle", px(20.0), text)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .line_height(px(19.0))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child(toast.title.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(13.0))
                                .line_height(px(19.0))
                                .child(toast.description.clone()),
                        ),
                )
                .child(
                    div()
                        .id("recording-toast-action")
                        .flex_shrink_0()
                        .h(px(24.0))
                        .px(px(8.0))
                        .flex()
                        .items_center()
                        .rounded(px(4.0))
                        .bg(self.theme.foreground)
                        .text_color(self.theme.background)
                        .tw_text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                            this.open_settings(
                                super::settings::SettingsTab::Transcription,
                                window,
                                cx,
                            );
                        }))
                        .child(SharedString::from(toast.action)),
                )
                .child(
                    // sonner's close button: a 20px circle over the top-left corner.
                    div()
                        .id("recording-toast-close")
                        .absolute()
                        .left(px(-10.0))
                        .top(px(-10.0))
                        .size(px(20.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .border_1()
                        .border_color(border)
                        .bg(background)
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            this.recording.toast = None;
                            cx.notify();
                        }))
                        .child(crate::ui::icon("x", px(12.0), text)),
                ),
        )
    }
}

/// `flex h-full min-h-[400px] flex-col items-center justify-center px-6 text-center`
fn transcript_screen() -> Div {
    div()
        .flex()
        .h_full()
        .min_h(px(400.0))
        .flex_col()
        .items_center()
        .justify_center()
        .px_6()
}

/// `degradedMessage`
fn degraded_message(error: &DegradedError) -> String {
    match error {
        DegradedError::AuthenticationFailed { provider } => {
            format!("Authentication failed ({provider})")
        }
        DegradedError::UpstreamUnavailable { message } => message.clone(),
        DegradedError::ConnectionTimeout => "Transcription connection timed out".to_string(),
        DegradedError::ProviderConfiguration { provider, .. } => {
            format!("Transcription provider is misconfigured ({provider})")
        }
        DegradedError::StreamError { .. } => "Transcription stream error".to_string(),
    }
}

/// `DancingSticks`: a 1px line while silent, otherwise sticks of
/// `stick_width` with `gap`, their heights following `generatePattern`
/// scaled by `0.2 + 0.8 * amplitude`.
pub(super) fn dancing_sticks(
    amplitude: f32,
    color: gpui::Rgba,
    height: f32,
    width: f32,
    stick_width: f32,
    gap: f32,
) -> AnyElement {
    let container = div()
        .flex()
        .w(px(width))
        .h(px(height))
        .items_center()
        .justify_center();
    if amplitude == 0.0 {
        return container
            .child(div().w(px(width)).h(px(1.0)).rounded_full().bg(color))
            .into_any_element();
    }
    let count = (((width + gap) / (stick_width + gap)).floor() as usize).max(1);
    let scale = 0.2 + 0.8 * amplitude.clamp(0.0, 1.0);
    let mid = (count as f32 - 1.0) / 2.0;
    container
        .gap(px(gap))
        .children((0..count).map(|index| {
            let base = if count <= 1 {
                100.0
            } else {
                let distance = (index as f32 - mid).abs() / mid;
                50.0 + 50.0 * (1.0 - distance)
            };
            let stick = height * scale * (base / 100.0).clamp(0.25, 1.0);
            div()
                .w(px(stick_width))
                .h(px(stick))
                .rounded_full()
                .bg(color)
        }))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn details(live: bool, repair: bool) -> PostCaptureInputs {
        PostCaptureInputs {
            has_audio: true,
            live_transcription_active: live,
            needs_batch_repair: repair,
            refine_speaker_diarization: false,
            transcript_write_failed: false,
        }
    }

    #[test]
    fn post_capture_action_follows_get_post_capture_action() {
        // Record-only capture with audio: batch then enhance.
        assert_eq!(
            post_capture_action(details(false, false), true),
            PostCaptureAction::BatchThenEnhance
        );
        // Live transcription completed during recording.
        assert_eq!(
            post_capture_action(details(true, false), true),
            PostCaptureAction::EnhanceOnly
        );
        // Settled diarization refines a complete live transcript when batch can run.
        let refine = PostCaptureInputs {
            refine_speaker_diarization: true,
            ..details(true, false)
        };
        assert_eq!(
            post_capture_action(refine, true),
            PostCaptureAction::BatchThenEnhance
        );
        assert_eq!(
            post_capture_action(refine, false),
            PostCaptureAction::EnhanceOnly
        );
        // Live transcription recovered mid-way, or a write failed: repair.
        assert_eq!(
            post_capture_action(details(true, true), true),
            PostCaptureAction::BatchThenEnhance
        );
        let failed = PostCaptureInputs {
            transcript_write_failed: true,
            ..details(true, false)
        };
        assert_eq!(
            post_capture_action(failed, true),
            PostCaptureAction::BatchThenEnhance
        );
        // No batch connection, or no saved audio: nothing.
        assert_eq!(
            post_capture_action(details(false, false), false),
            PostCaptureAction::None
        );
        let no_audio = PostCaptureInputs {
            has_audio: false,
            ..details(false, false)
        };
        assert_eq!(post_capture_action(no_audio, true), PostCaptureAction::None);
    }

    #[test]
    fn current_capture_offset_keeps_the_existing_audio_when_the_file_grew() {
        assert_eq!(current_capture_audio_offset_ms(0, Some(10_000)), 0);
        assert_eq!(current_capture_audio_offset_ms(4_000, Some(10_000)), 4_000);
        // Within the one-second tolerance the shorter final file wins.
        assert_eq!(current_capture_audio_offset_ms(4_000, Some(3_500)), 3_500);
        // A re-recorded (shorter) file starts over.
        assert_eq!(current_capture_audio_offset_ms(4_000, Some(2_000)), 0);
        assert_eq!(current_capture_audio_offset_ms(4_000, None), 0);
    }
}
