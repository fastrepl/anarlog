use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use anlg_listener_core::actors::recorder::{
    DELETE_ON_STOP, RecoveryAudioChunk, acknowledge_recovery_chunk, delete_capture_audio,
    delete_transcribed_capture_audio, list_recovery_chunks_after, recover_interrupted_captures,
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

#[derive(Clone)]
pub(super) struct CaptureTarget {
    pub(super) transcript: Arc<str>,
    pub(super) started: i64,
    pub(super) provider: String,
    pub(super) model: String,
    pub(super) memo: String,
    pub(super) created_at: Option<String>,
    pub(super) owner_user_id: String,
    pub(super) retained: bool,
}

struct ParsedMarker {
    session: SessionId,
    primary: CaptureTarget,
    inherited: Vec<CaptureTarget>,
    retained: bool,
    earliest: i64,
}

pub(super) fn target_marker(target: &CaptureTarget) -> serde_json::Value {
    json!({
        "transcriptId": target.transcript.as_ref(),
        "startedAt": target.started,
        "createdAt": target.created_at.as_deref().unwrap_or_default(),
        "ownerUserId": target.owner_user_id.as_str(),
        "memo": target.memo.as_str(),
        "retainAudio": target.retained,
        "provider": target.provider.as_str(),
        "model": target.model.as_str(),
    })
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

pub(super) fn parse_target(value: &serde_json::Value) -> Option<CaptureTarget> {
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
        owner_user_id: value["ownerUserId"].as_str().unwrap_or("").to_owned(),
        retained: value["retainAudio"].as_bool().unwrap_or(true),
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
        .filter(|capture| capture.transcript != primary.transcript)
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

pub(super) async fn ensure_transcript_ready(
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

pub(super) fn resolve_chunk_target<'a>(
    chunk: &RecoveryAudioChunk,
    primary: &'a CaptureTarget,
    inherited: &'a [CaptureTarget],
) -> Option<&'a CaptureTarget> {
    let capture_started = chunk.capture_started_at as i64;
    // A chunk's `capture_started_at` is the recorder's start time, which is sampled just after
    // the marker's `startedAt`. They usually match but can differ, and a backward clock
    // adjustment can make an inherited attempt's timestamps land after the current one.
    // Classify by the closest exact or near match first so an inherited chunk is never handed
    // to the current capture and released without being transcribed into its own transcript.
    if capture_started == primary.started {
        return Some(primary);
    }
    if let Some(target) = inherited
        .iter()
        .find(|capture| capture.started == capture_started)
    {
        return Some(target);
    }
    const TOLERANCE: i64 = 5_000;
    let nearest_inherited = inherited
        .iter()
        .filter(|capture| (capture_started - capture.started).abs() <= TOLERANCE)
        .min_by_key(|capture| (capture_started - capture.started).abs());
    if let Some(target) = nearest_inherited {
        let inherited_distance = (capture_started - target.started).abs();
        let primary_distance = (capture_started - primary.started).abs();
        if capture_started < primary.started || inherited_distance < primary_distance {
            return Some(target);
        }
    }
    if capture_started >= primary.started {
        return Some(primary);
    }
    inherited
        .iter()
        .filter(|capture| capture_started >= capture.started)
        .max_by_key(|capture| capture.started)
        .or_else(|| {
            inherited
                .iter()
                .filter(|capture| capture_started >= capture.started.saturating_sub(TOLERANCE))
                .min_by_key(|capture| capture.started)
        })
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
                .cloned()
                .chain(inherited.iter().map(|target| target.transcript.clone()))
                .filter(|id| id != &primary.transcript)
                .collect::<HashSet<_>>()
                .into_iter()
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

async fn pending_marker(
    runtime: &RuntimeHandle,
    session: &SessionId,
) -> Result<Option<ParsedMarker>> {
    let session = session.clone();
    runtime
        .submit(move |services| async move {
            let rows = services
                .executor
                .execute(
                    "SELECT id, value_json FROM app_settings WHERE id = ?".into(),
                    vec![json!(format!("capture_lifecycle_pending:{}", session.0))],
                )
                .await
                .map_err(failure)?;
            let Some(row) = rows.first() else {
                return Ok(None);
            };
            Ok(parse_marker_row(row).ok())
        })?
        .receive()
        .await
}

fn recovery_marker(
    session: &SessionId,
    target: &CaptureTarget,
    inherited: &[CaptureTarget],
    retained: bool,
) -> serde_json::Value {
    let mut marker = json!({
        "version": 1,
        "chunkedAudio": true,
        "retainAudio": retained,
        "phase": "capturing",
        "sessionId": session,
        "transcriptId": target.transcript.as_ref(),
        "startedAt": target.started,
        "createdAt": target.created_at.as_deref().unwrap_or_default(),
        "audioOffsetMs": 0,
        "preserveExistingTranscript": true,
        "ownerUserId": target.owner_user_id.as_str(),
        "memo": target.memo.as_str(),
        "provider": target.provider.as_str(),
        "model": target.model.as_str(),
    });
    if !inherited.is_empty() {
        marker["inheritedCaptures"] =
            json!(inherited.iter().map(target_marker).collect::<Vec<_>>());
    }
    marker
}

async fn replace_marker(
    runtime: &RuntimeHandle,
    session: &SessionId,
    marker: serde_json::Value,
    discarded_transcript: Option<&Arc<str>>,
) -> Result<()> {
    let session = session.clone();
    let value_json = marker.to_string();
    let discarded_transcript = discarded_transcript.cloned();
    runtime
        .submit(move |services| async move {
            let mut statements = vec![statement(
                "INSERT INTO app_settings (id, value_json, updated_at) VALUES (?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
                vec![json!(format!("capture_lifecycle_pending:{}", session.0)), json!(value_json)],
                Some(1),
            )];
            if let Some(transcript) = discarded_transcript {
                // The discarded attempt keeps its saved transcript; close it so later recovery
                // does not leave it unfinished.
                statements.push(statement(
                    "UPDATE transcripts SET ended_at_ms = COALESCE(ended_at_ms, CAST(unixepoch('now') * 1000 AS INTEGER)) WHERE id = ? AND session_id = ? AND deleted_at IS NULL",
                    vec![json!(transcript), json!(&session.0)],
                    None,
                ));
            }
            services.executor.execute_transaction(statements).await.map_err(failure)?;
            Ok(())
        })?
        .receive()
        .await
}

async fn discard_target_chunks(
    directory: &Path,
    primary: &CaptureTarget,
    inherited: &[CaptureTarget],
) -> Result<()> {
    let recovery_dir = directory.join(RECOVERY_DIR);
    let stop_marker = directory.join(DELETE_ON_STOP);
    let primary = primary.clone();
    let inherited = inherited.to_vec();
    tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        // Scan the whole directory, not the recorder's bounded 128-chunk page: chunks owned by
        // preserved attempts can fill that page and hide the discarded attempt's later chunks.
        let mut paths = Vec::new();
        match std::fs::read_dir(&recovery_dir) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
            Ok(entries) => {
                for entry in entries {
                    let path = entry?.path();
                    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                        continue;
                    };
                    let Some((capture_started_at, _)) = parse_chunk_name(name) else {
                        continue;
                    };
                    let chunk = RecoveryAudioChunk {
                        id: name.to_owned(),
                        path: path.to_string_lossy().into_owned(),
                        capture_started_at,
                        start_ms: 0,
                        end_ms: 0,
                        audio_start_ms: 0,
                    };
                    if resolve_chunk_target(&chunk, &primary, &inherited)
                        .is_some_and(|target| target.transcript == primary.transcript)
                    {
                        paths.push(path);
                    }
                }
            }
        }
        for path in paths {
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        // A promoted capture owns the shared archive now, so a discarded "Don't save" attempt
        // must not make a later cleanup delete it.
        match std::fs::remove_file(stop_marker) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        Ok(())
    })
    .await
    .map_err(failure)?
    .map_err(failure)
}

/// Parses a recovery chunk or partial file name into its capture start time.
fn parse_chunk_name(name: &str) -> Option<(u64, Option<u64>)> {
    let stem = name
        .strip_suffix(".mp3")
        .or_else(|| name.strip_suffix(".part"))?;
    let mut parts = stem.split('-');
    let capture_started_at = parts.next()?.parse::<u64>().ok()?;
    let start_ms = parts.next().and_then(|value| value.parse::<u64>().ok());
    Some((capture_started_at, start_ms))
}

/// Gives up on an interrupted capture whose recovery can't succeed: its unrecovered chunks go
/// (all audio for "Don't save"), and the marker is finalized so the note can record again.
pub async fn discard(
    runtime: &RuntimeHandle,
    vault: &Path,
    session: SessionId,
    transcript: Arc<str>,
    retained: bool,
    activities: Activities,
) -> Result<()> {
    let _activity = match activities.acquire(session.clone()) {
        Ok(activity) => activity,
        Err(ServiceError::Busy) => {
            return Err(failure(
                "This note is recording; stop it before discarding the interrupted audio.",
            ));
        }
        Err(error) => return Err(error),
    };
    let Some(marker) = pending_marker(runtime, &session).await? else {
        return Err(failure(
            "This recording's recovery state changed; its audio was left untouched.",
        ));
    };
    // Capture start replaces the marker for a new attempt, so only discard the audio while the
    // action still names the pending capture.
    if marker.session.0.as_ref() != session.0.as_ref()
        || marker.primary.transcript.as_ref() != transcript.as_ref()
        || marker.retained != retained
    {
        return Err(failure(
            "This recording's recovery state changed; its audio was left untouched.",
        ));
    }
    let directory = session_directory(vault, &session)?;
    if marker.inherited.is_empty() {
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
        return finalize_marker(runtime, session, transcript, false).await;
    }
    discard_target_chunks(&directory, &marker.primary, &marker.inherited).await?;
    let mut inherited = marker.inherited;
    // `prepare` sorts inherited targets oldest first. Promote the newest remaining target so the
    // timestamp fallback in `resolve_chunk_target` does not drop still-pending newer targets.
    let promoted = inherited.pop().expect("inherited is non-empty here");
    // The shared archive holds every pending attempt's audio, so keep it whenever any of them —
    // the discarded, promoted, or still-inherited attempts — was retained.
    let retained = retained || promoted.retained || inherited.iter().any(|target| target.retained);
    let replacement = recovery_marker(&session, &promoted, &inherited, retained);
    replace_marker(
        runtime,
        &session,
        replacement,
        Some(&marker.primary.transcript),
    )
    .await
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

/// Removes the recorder's `.delete-audio-on-stop` marker so startup cleanup leaves the shared
/// archive alone. Returns true when the marker was present.
pub async fn remove_delete_on_stop(directory: PathBuf) -> Result<bool> {
    tokio::task::spawn_blocking(move || {
        let marker = directory.join(DELETE_ON_STOP);
        match std::fs::remove_file(&marker) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    })
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
        Self::chunks_after(directory, None).await
    }

    /// The oldest page of recovery chunks strictly after `after` (a `(capture_started_at,
    /// start_ms)` cursor). Advancing the cursor past examined entries keeps a full page of failed
    /// chunks from blocking later ones.
    pub async fn chunks_after(
        directory: PathBuf,
        after: Option<(u64, u64)>,
    ) -> Result<Vec<RecoveryAudioChunk>> {
        tokio::task::spawn_blocking(move || list_recovery_chunks_after(&directory, after))
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
