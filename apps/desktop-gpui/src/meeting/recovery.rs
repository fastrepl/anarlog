use std::collections::BTreeMap;
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

struct ParsedMarker {
    session: SessionId,
    transcript: Arc<str>,
    started: i64,
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
    let transcript: Arc<str> = raw_transcript.to_owned().into();
    if string(row, "id")? != format!("capture_lifecycle_pending:{}", session.0)
        || marker_version(&marker) != Some(1)
    {
        return Err(failure(
            "Invalid interrupted capture marker; original marker preserved.",
        ));
    }
    let started = marker_timestamp(&marker["startedAt"])
        .ok_or_else(|| failure("Invalid startedAt; original record preserved."))?;
    let retained = marker["retainAudio"].as_bool().unwrap_or(true);
    // Shipping's markers can inherit earlier captures, whose chunks belong to their own
    // transcripts; anything older is left over from finished captures.
    let earliest = marker["inheritedCaptures"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|capture| marker_timestamp(&capture["startedAt"]))
        .map(|at| at.saturating_sub(5_000))
        .fold(started, i64::min);
    Ok(ParsedMarker {
        session,
        transcript,
        started,
        retained,
        earliest,
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
            transcript,
            started,
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
        let result = async {
            let directory = session_directory(&vault, &session)?;
            let store = TranscriptStore(runtime.clone());
            store.fold(transcript.clone())?.receive().await?;
            let recovery = Recovery::default();
            loop {
                let chunks = page(&directory, earliest.max(0) as u64).await?;
                if chunks.is_empty() { break; }
                for chunk in chunks {
                    if (chunk.capture_started_at as i64) < started {
                        return Err(failure("Audio from an earlier interrupted recording in this note can't be recovered here."));
                    }
                    let gap = chunk_interval(&chunk, started);
                    recovery.repair(&store, session.clone(), transcript.clone(), chunk, vec![gap], &resolver).await?;
                }
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
            acknowledge_finalization(runtime, session.clone(), transcript.clone()).await
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
