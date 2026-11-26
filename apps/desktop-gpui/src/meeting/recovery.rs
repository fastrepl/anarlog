use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use anlg_listener_core::actors::recorder::{
    RecoveryAudioChunk, acknowledge_recovery_chunk, delete_capture_audio,
    delete_transcribed_capture_audio, list_recovery_chunks, recover_interrupted_captures,
};
use anlg_listener2_core::{BatchEvent, BatchParams, BatchRuntime, run_batch};
use desktop_runtime::{CancellationToken, Result, RuntimeHandle, ServiceError, SessionId};
use futures::future::BoxFuture;
use serde_json::json;
use tokio::sync::Mutex;

use super::model::{Interval, MAX_TEXT, MAX_WORDS, Word, failure};
use super::retention::{Activities, session_directory};
use super::store::TranscriptStore;
use super::store::{statement, string};

pub type RecoveryResolver =
    Arc<dyn Fn(SessionId, PathBuf) -> BoxFuture<'static, Result<BatchParams>> + Send + Sync>;

pub struct InterruptedCapture {
    pub session: SessionId,
    pub transcript: Arc<str>,
    pub title: Arc<str>,
    /// Whether the capture keeps its recording; discarding a "Don't save" capture deletes it.
    pub retained: bool,
    pub result: Result<()>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TranscriptPresence {
    Present,
    Missing,
}

struct CaptureTarget {
    transcript: Arc<str>,
    started: i64,
    provider: String,
    model: String,
    memo: String,
    created_at: Option<String>,
}

struct ParsedMarker {
    session: SessionId,
    primary: CaptureTarget,
    inherited: Vec<CaptureTarget>,
    retained: bool,
    earliest: i64,
}

fn finite_i64(value: f64) -> Option<i64> {
    const MIN: f64 = i64::MIN as f64;
    const MAX_EXCLUSIVE: f64 = 9_223_372_036_854_775_808.0;
    let trunc = value.trunc();
    (value.is_finite() && (MIN..MAX_EXCLUSIVE).contains(&trunc)).then_some(trunc as i64)
}

fn marker_version(marker: &serde_json::Value) -> Option<i64> {
    marker["version"].as_i64().or_else(|| {
        marker["version"]
            .as_f64()
            .filter(|value| value.is_finite() && value.fract() == 0.0)
            .and_then(finite_i64)
    })
}

fn marker_timestamp(value: &serde_json::Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_f64().and_then(finite_i64))
}

fn parse_target(value: &serde_json::Value) -> Option<CaptureTarget> {
    let raw_transcript = value["transcriptId"].as_str().filter(|id| !id.is_empty())?;
    let started = marker_timestamp(&value["startedAt"])?;
    Some(CaptureTarget {
        transcript: raw_transcript.to_owned().into(),
        started,
        provider: value["provider"].as_str().unwrap_or("").to_owned(),
        model: value["model"].as_str().unwrap_or("").to_owned(),
        memo: value["memo"].as_str().unwrap_or("").to_owned(),
        created_at: value["createdAt"]
            .as_str()
            .filter(|created| !created.is_empty())
            .map(str::to_owned),
    })
}

fn parse_marker_row(row: &serde_json::Value) -> Result<ParsedMarker> {
    let marker: serde_json::Value =
        serde_json::from_str(string(row, "value_json")?).map_err(failure)?;
    let raw_session = string(&marker, "sessionId")?;
    let raw_transcript = string(&marker, "transcriptId")?;
    if raw_session.is_empty() || raw_transcript.is_empty() {
        return Err(failure(
            "Invalid interrupted capture marker; original marker preserved.",
        ));
    }
    let session = SessionId(raw_session.to_owned().into());
    if string(row, "id")? != format!("capture_lifecycle_pending:{}", session.0)
        || marker_version(&marker) != Some(1)
    {
        return Err(failure(
            "Invalid interrupted capture marker; original marker preserved.",
        ));
    }
    let started = marker_timestamp(&marker["startedAt"])
        .ok_or_else(|| failure("Invalid startedAt; original record preserved."))?;
    let primary = parse_target(&marker)
        .ok_or_else(|| failure("Invalid interrupted capture marker; original marker preserved."))?;
    let retained = marker["retainAudio"].as_bool().unwrap_or(true);
    let inherited_array = marker["inheritedCaptures"].as_array();
    // Shipping's markers can inherit earlier captures, whose chunks belong to their own
    // transcripts; anything older is left over from finished captures.
    let earliest = inherited_array
        .into_iter()
        .flatten()
        .filter_map(|capture| marker_timestamp(&capture["startedAt"]))
        .map(|at| at.saturating_sub(5_000))
        .fold(started, i64::min);
    let inherited = inherited_array
        .into_iter()
        .flatten()
        .filter_map(parse_target)
        .filter(|capture| {
            capture.started < primary.started && capture.transcript != primary.transcript
        })
        .collect();
    Ok(ParsedMarker {
        session,
        primary,
        inherited,
        retained,
        earliest,
    })
}

async fn check_transcript(
    runtime: &RuntimeHandle,
    session: &SessionId,
    transcript: &Arc<str>,
) -> Result<TranscriptPresence> {
    let session = session.clone();
    let transcript = transcript.clone();
    runtime
        .submit(move |services| async move {
            let accessible = services
                .executor
                .execute(
                    "SELECT 1 FROM sessions WHERE id = ? AND deleted_at IS NULL AND (locked = 0 OR id IN (SELECT value FROM json_each(?)))".into(),
                    vec![json!(session), super::access::param()],
                )
                .await
                .map_err(failure)?;
            if accessible.is_empty() {
                return Err(ServiceError::Conflict);
            }
            let rows = services
                .executor
                .execute(
                    "SELECT session_id, deleted_at FROM transcripts WHERE id = ?".into(),
                    vec![json!(transcript)],
                )
                .await
                .map_err(failure)?;
            match rows.first() {
                Some(row) => {
                    if row["session_id"].as_str() != Some(session.0.as_ref())
                        || !row["deleted_at"].is_null()
                    {
                        return Err(ServiceError::Conflict);
                    }
                    Ok(TranscriptPresence::Present)
                }
                None => Ok(TranscriptPresence::Missing),
            }
        })?
        .receive()
        .await
}

async fn ensure_transcript_ready(
    runtime: &RuntimeHandle,
    store: &TranscriptStore,
    session: &SessionId,
    target: &CaptureTarget,
) -> Result<()> {
    let presence = check_transcript(runtime, session, &target.transcript).await?;
    if presence == TranscriptPresence::Missing {
        let session = session.clone();
        let transcript = target.transcript.clone();
        let provider = target.provider.clone();
        let model = target.model.clone();
        let started = target.started;
        let memo = target.memo.clone();
        let created_at = target.created_at.clone();
        runtime
            .submit(move |services| async move {
                services
                    .executor
                    .execute_transaction(vec![statement(
                        "INSERT INTO transcripts (id, session_id, owner_user_id, workspace_id, source, provider, model, started_at_ms, memo, created_at) \
                         SELECT ?, id, owner_user_id, workspace_id, 'live_capture', ?, ?, ?, ?, COALESCE(?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
                         FROM sessions WHERE id = ? AND deleted_at IS NULL AND (locked = 0 OR id IN (SELECT value FROM json_each(?))) \
                         AND NOT EXISTS (SELECT 1 FROM transcripts WHERE id = ?)",
                        vec![
                            json!(transcript),
                            json!(provider),
                            json!(model),
                            json!(started),
                            json!(memo),
                            json!(created_at),
                            json!(session),
                            super::access::param(),
                            json!(transcript),
                        ],
                        Some(1),
                    )])
                    .await
                    .map_err(|error| match error {
                        anlg_db_execute::Error::UnexpectedRowsAffected { .. } => {
                            ServiceError::Conflict
                        }
                        error => failure(error),
                    })?;
                Ok(())
            })?
            .receive()
            .await?;
    }
    store.fold(target.transcript.clone())?.receive().await
}

fn resolve_chunk_target<'a>(
    chunk: &RecoveryAudioChunk,
    primary: &'a CaptureTarget,
    inherited: &'a [CaptureTarget],
) -> Option<&'a CaptureTarget> {
    let capture_started = chunk.capture_started_at as i64;
    if capture_started >= primary.started {
        return Some(primary);
    }
    inherited
        .iter()
        .filter(|capture| capture_started >= capture.started.saturating_sub(5_000))
        .max_by_key(|capture| capture.started)
}

pub async fn recover_startup(
    runtime: &RuntimeHandle,
    vault: PathBuf,
    activities: Activities,
    resolver: RecoveryResolver,
) -> Result<Vec<InterruptedCapture>> {
    let _startup = activities.startup()?;
    let sessions = vault.join("sessions");
    tokio::task::spawn_blocking(move || recover_interrupted_captures(&sessions).map_err(failure))
        .await
        .map_err(failure)??;
    let rows = runtime.submit(move |services| async move {
        services.executor.execute("SELECT a.id, a.value_json, COALESCE(s.title, '') AS title FROM app_settings a LEFT JOIN sessions s ON s.id = substr(a.id, 27) WHERE a.id GLOB 'capture_lifecycle_pending:*' ORDER BY a.id LIMIT 1001".into(), Vec::new()).await.map_err(failure)
    })?.receive().await?;
    if rows.len() > 1000 {
        return Err(failure(
            "Too many interrupted captures; recovery requires bounded pages.",
        ));
    }
    let mut reports = Vec::new();
    for row in rows {
        let ParsedMarker {
            session,
            primary,
            inherited,
            retained,
            earliest,
        } = match parse_marker_row(&row) {
            Ok(parsed) => parsed,
            Err(error) => {
                tracing::warn!(
                    id = row["id"].as_str().unwrap_or(""),
                    %error,
                    "skipping invalid interrupted capture marker; original record preserved"
                );
                continue;
            }
        };
        let transcript = primary.transcript.clone();
        let result = async {
            let directory = session_directory(&vault, &session)?;
            let store = TranscriptStore(runtime.clone());
            let mut prepared = HashSet::new();
            if check_transcript(runtime, &session, &primary.transcript).await?
                == TranscriptPresence::Present
            {
                store.fold(primary.transcript.clone())?.receive().await?;
                prepared.insert(primary.transcript.clone());
            }
            let recovery = Recovery::default();
            let mut failed_chunks = HashSet::new();
            let mut deferred_error: Option<ServiceError> = None;
            loop {
                let chunks = page(&directory, earliest.max(0) as u64).await?;
                let pending: Vec<_> = chunks
                    .into_iter()
                    .filter(|chunk| !failed_chunks.contains(&chunk.id))
                    .collect();
                if pending.is_empty() {
                    break;
                }
                let mut progressed = false;
                for chunk in pending {
                    match resolve_chunk_target(&chunk, &primary, &inherited) {
                        Some(target) if target.transcript == primary.transcript => {
                            if !prepared.contains(&target.transcript) {
                                ensure_transcript_ready(runtime, &store, &session, target).await?;
                                prepared.insert(target.transcript.clone());
                            }
                            let gap = chunk_interval(&chunk, target.started);
                            recovery
                                .repair(
                                    &store,
                                    session.clone(),
                                    target.transcript.clone(),
                                    chunk,
                                    vec![gap],
                                    &resolver,
                                )
                                .await?;
                            progressed = true;
                        }
                        Some(target) => {
                            let chunk_id = chunk.id.clone();
                            let outcome = async {
                                if !prepared.contains(&target.transcript) {
                                    ensure_transcript_ready(runtime, &store, &session, target)
                                        .await?;
                                    prepared.insert(target.transcript.clone());
                                }
                                let gap = chunk_interval(&chunk, target.started);
                                recovery
                                    .repair(
                                        &store,
                                        session.clone(),
                                        target.transcript.clone(),
                                        chunk,
                                        vec![gap],
                                        &resolver,
                                    )
                                    .await
                            }
                            .await;
                            match outcome {
                                Ok(()) => progressed = true,
                                Err(error) => {
                                    deferred_error.get_or_insert(error);
                                    failed_chunks.insert(chunk_id);
                                }
                            }
                        }
                        None => {
                            deferred_error.get_or_insert_with(|| {
                                failure(
                                    "Audio from an earlier interrupted recording in this note can't be recovered here.",
                                )
                            });
                            failed_chunks.insert(chunk.id);
                        }
                    }
                }
                if !progressed {
                    break;
                }
            }
            if let Some(error) = deferred_error {
                return Err(error);
            }
            // "Don't save" audio goes only once every chunk is transcribed.
            let unresolved = if retained {
                unreadable_partial(directory).await?
            } else {
                !delete_transcribed(directory).await?
            };
            if unresolved {
                return Err(failure("Unreadable audio from the interrupted recording is kept until you discard it."));
            }
            let extra_transcripts: Vec<Arc<str>> = prepared
                .iter()
                .filter(|id| *id != &primary.transcript)
                .cloned()
                .collect();
            if !extra_transcripts.is_empty() {
                let session = session.clone();
                runtime
                    .submit(move |services| async move {
                        let statements = extra_transcripts
                            .into_iter()
                            .map(|transcript| {
                                statement(
                                    "UPDATE transcripts SET ended_at_ms = COALESCE(ended_at_ms, CAST(unixepoch('now') * 1000 AS INTEGER)) WHERE id = ? AND session_id = ? AND deleted_at IS NULL",
                                    vec![json!(transcript), json!(session)],
                                    None,
                                )
                            })
                            .collect();
                        services
                            .executor
                            .execute_transaction(statements)
                            .await
                            .map_err(failure)?;
                        Ok(())
                    })?
                    .receive()
                    .await?;
            }
            finalize_marker(
                runtime,
                session.clone(),
                primary.transcript.clone(),
                prepared.contains(&primary.transcript),
            )
            .await
        }.await;
        reports.push(InterruptedCapture {
            session,
            transcript,
            title: string(&row, "title")?.to_owned().into(),
            retained,
            result,
        });
    }
    Ok(reports)
}

/// Gives up on an interrupted capture whose recovery can't succeed: its unrecovered chunks go
/// (all audio for "Don't save"), and the marker is finalized so the note can record again.
pub async fn discard(
    runtime: &RuntimeHandle,
    vault: &Path,
    session: SessionId,
    transcript: Arc<str>,
    retained: bool,
) -> Result<()> {
    let directory = session_directory(vault, &session)?;
    tokio::task::spawn_blocking(move || {
        if !retained {
            return delete_capture_audio(&directory);
        }
        // A retained capture keeps its recording; only its recovery chunks and partials go.
        match std::fs::remove_dir_all(directory.join(RECOVERY_DIR)) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    })
    .await
    .map_err(failure)?
    .map_err(failure)?;
    finalize_marker(runtime, session, transcript, false).await
}

/// The recorder's chunk directory, where it leaves a partial chunk it can't decode in place.
const RECOVERY_DIR: &str = "audio-recovery";

/// Whether a nonempty partial chunk the recorder couldn't decode is still waiting; recovery
/// can't transcribe it, so it isn't finished.
async fn unreadable_partial(directory: PathBuf) -> Result<bool> {
    tokio::task::spawn_blocking(move || -> std::io::Result<bool> {
        let entries = match std::fs::read_dir(directory.join(RECOVERY_DIR)) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            entries => entries?,
        };
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let metadata = entry.metadata()?;
            if path
                .extension()
                .is_some_and(|extension| extension == "part")
                && metadata.is_file()
                && metadata.len() > 0
                && anlg_audio_utils::source_from_path(&path).is_err()
            {
                return Ok(true);
            }
        }
        Ok(false)
    })
    .await
    .map_err(failure)?
    .map_err(failure)
}

/// The oldest page of a session's recovery chunks from captures that started at `started` or
/// later. Older chunks belong to finished captures and are released on the way, so they never
/// fill the recorder's 128-chunk page. Callers release what they settle and ask again until a
/// page comes back empty.
pub async fn page(directory: &Path, started: u64) -> Result<Vec<RecoveryAudioChunk>> {
    loop {
        let (stale, current): (Vec<_>, Vec<_>) = Recovery::chunks(directory.to_path_buf())
            .await?
            .into_iter()
            .partition(|chunk| chunk.capture_started_at < started);
        let more = !stale.is_empty() && current.is_empty();
        for chunk in stale {
            release(directory, chunk.id).await?;
        }
        if !more {
            return Ok(current);
        }
    }
}

pub async fn release(directory: &Path, chunk: String) -> Result<()> {
    let directory = directory.to_path_buf();
    tokio::task::spawn_blocking(move || acknowledge_recovery_chunk(&directory, &chunk))
        .await
        .map_err(failure)?
        .map_err(failure)
}

/// Deletes "Don't save" audio once no chunk awaits transcription; false while one still does.
pub async fn delete_transcribed(directory: PathBuf) -> Result<bool> {
    tokio::task::spawn_blocking(move || delete_transcribed_capture_audio(&directory))
        .await
        .map_err(failure)?
        .map_err(failure)
}

pub async fn acknowledge_finalization(
    runtime: &RuntimeHandle,
    session: SessionId,
    transcript: Arc<str>,
) -> Result<()> {
    finalize_marker(runtime, session, transcript, true).await
}

async fn finalize_marker(
    runtime: &RuntimeHandle,
    session: SessionId,
    transcript: Arc<str>,
    require_transcript: bool,
) -> Result<()> {
    runtime.submit(move |services| async move {
        services.executor.execute_transaction(vec![
            statement("UPDATE transcripts SET ended_at_ms = COALESCE(ended_at_ms, CAST(unixepoch('now') * 1000 AS INTEGER)) WHERE id = ? AND session_id = ? AND deleted_at IS NULL",
                vec![json!(transcript), json!(session)], require_transcript.then_some(1)),
            statement("DELETE FROM app_settings WHERE id = ? AND json_valid(value_json) AND json_extract(value_json, '$.transcriptId') = ?",
                vec![json!(format!("capture_lifecycle_pending:{}", session.0)), json!(transcript)], Some(1)),
        ]).await.map_err(failure)?;
        Ok(())
    })?.receive().await
}

pub fn chunk_interval(chunk: &RecoveryAudioChunk, started_at: i64) -> Interval {
    let offset = chunk.capture_started_at as i64 - started_at;
    Interval {
        start: chunk.start_ms as i64 + offset,
        end: chunk.end_ms as i64 + offset,
    }
}

struct BatchAdapter(CancellationToken);

impl BatchRuntime for BatchAdapter {
    fn emit(&self, _: BatchEvent) {}
    fn is_cancelled(&self) -> bool {
        self.0.is_cancelled()
    }
}

#[derive(Clone)]
pub struct Recovery {
    pub cancellation: CancellationToken,
    generation: Arc<AtomicU64>,
    serial: Arc<Mutex<()>>,
}

impl Default for Recovery {
    fn default() -> Self {
        Self {
            cancellation: CancellationToken::new(),
            generation: Arc::new(AtomicU64::new(0)),
            serial: Arc::new(Mutex::new(())),
        }
    }
}

impl Recovery {
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }
    pub fn incident(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    pub async fn stop(&self) {
        self.cancellation.cancel();
        let _guard = self.serial.lock().await;
    }

    pub async fn chunks(directory: PathBuf) -> Result<Vec<RecoveryAudioChunk>> {
        tokio::task::spawn_blocking(move || list_recovery_chunks(&directory))
            .await
            .map_err(failure)?
            .map_err(failure)
    }

    pub async fn repair(
        &self,
        store: &TranscriptStore,
        session: SessionId,
        transcript: Arc<str>,
        chunk: RecoveryAudioChunk,
        gaps: Vec<Interval>,
        resolver: &RecoveryResolver,
    ) -> Result<()> {
        let _guard = self.serial.lock().await;
        let generation = self.generation.load(Ordering::Acquire);
        self.check(generation)?;
        let snapshot = store.snapshot(transcript.clone())?.receive().await?;
        let offset =
            chunk.capture_started_at as i64 - snapshot.started_at + chunk.audio_start_ms as i64;
        let before = snapshot.words;
        let path = PathBuf::from(&chunk.path);
        let params = tokio::select! {
            _ = self.cancellation.cancelled() => return Err(ServiceError::Cancelled),
            result = resolver(session.clone(), path.clone()) => result?,
        };
        if params.session_id != session.0.as_ref()
            || std::path::Path::new(&params.file_path) != path
        {
            return Err(ServiceError::Conflict);
        }
        let output = tokio::select! {
            _ = self.cancellation.cancelled() => return Err(ServiceError::Cancelled),
            result = run_batch(Arc::new(BatchAdapter(self.cancellation.clone())), params) => result.map_err(failure)?,
        };
        self.check(generation)?;
        let mut words = Vec::new();
        let mut hints = Vec::new();
        let mut text_bytes = 0;
        for channel in output.response.results.channels {
            let Some(alternative) = channel.alternatives.into_iter().next() else {
                continue;
            };
            for word in alternative.words {
                if !word.start.is_finite()
                    || !word.end.is_finite()
                    || word.start < 0.
                    || word.end < word.start
                {
                    return Err(failure("Provider returned invalid recovery timestamps."));
                }
                let text = format!("{} ", word.punctuated_word.unwrap_or(word.word));
                text_bytes += text.len();
                if words.len() >= MAX_WORDS || text_bytes > MAX_TEXT {
                    return Err(failure(
                        "Recovery transcript exceeds safe bounds; chunk retained.",
                    ));
                }
                let id = uuid::Uuid::new_v4().to_string();
                if let Some(speaker) = word.speaker {
                    hints.push(json!({
                        "id": format!("{id}:provider_speaker_index"), "word_id": id,
                        "type": "provider_speaker_index",
                        "value": json!({"channel": word.channel, "speaker_index": speaker}).to_string(),
                    }));
                }
                words.push(Word {
                    id,
                    text,
                    start_ms: (word.start * 1000.) as i64 + offset,
                    end_ms: (word.end * 1000.) as i64 + offset,
                    channel: word.channel,
                    extra: BTreeMap::new(),
                });
            }
        }
        if words.is_empty() && gaps.iter().any(|gap| gap.end > gap.start) {
            return Err(failure(
                "Recovery provider returned no words; chunk retained for retry.",
            ));
        }
        self.check(generation)?;
        store
            .repair(transcript, before, words, hints, gaps)?
            .receive()
            .await?;
        self.check(generation)?;
        let directory = path
            .parent()
            .and_then(|path| path.parent())
            .ok_or_else(|| failure("Invalid recovery chunk path."))?;
        release(directory, chunk.id).await
    }

    fn check(&self, generation: u64) -> Result<()> {
        if self.cancellation.is_cancelled() || self.generation.load(Ordering::Acquire) != generation
        {
            Err(ServiceError::Cancelled)
        } else {
            Ok(())
        }
    }
}
