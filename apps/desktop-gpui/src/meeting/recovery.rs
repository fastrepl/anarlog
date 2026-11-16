use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use anlg_listener_core::actors::recorder::{
    RecoveryAudioChunk, acknowledge_recovery_chunk, delete_capture_audio, list_recovery_chunks,
    recover_interrupted_captures,
};
use anlg_listener2_core::{BatchEvent, BatchParams, BatchRuntime, run_batch};
use desktop_runtime::{CancellationToken, Result, RuntimeHandle, ServiceError, SessionId};
use futures::future::BoxFuture;
use serde_json::json;
use tokio::sync::Mutex;

use super::model::{Interval, MAX_TEXT, MAX_WORDS, Word, failure};
use super::retention::{Activities, session_directory};
use super::store::TranscriptStore;
use super::store::{integer, statement, string};

pub type RecoveryResolver =
    Arc<dyn Fn(SessionId, PathBuf) -> BoxFuture<'static, Result<BatchParams>> + Send + Sync>;

pub struct InterruptedCapture {
    pub session: SessionId,
    pub transcript: Arc<str>,
    pub result: Result<()>,
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
        services.executor.execute("SELECT id, value_json FROM app_settings WHERE id GLOB 'capture_lifecycle_pending:*' ORDER BY id LIMIT 1001".into(), Vec::new()).await.map_err(failure)
    })?.receive().await?;
    if rows.len() > 1000 {
        return Err(failure(
            "Too many interrupted captures; recovery requires bounded pages.",
        ));
    }
    let mut reports = Vec::new();
    for row in rows {
        let marker: serde_json::Value =
            serde_json::from_str(string(&row, "value_json")?).map_err(failure)?;
        let session = SessionId(string(&marker, "sessionId")?.to_owned().into());
        let transcript: Arc<str> = string(&marker, "transcriptId")?.to_owned().into();
        if string(&row, "id")? != format!("capture_lifecycle_pending:{}", session.0)
            || integer(&marker, "version")? != 1
        {
            return Err(failure(
                "Invalid interrupted capture marker; original marker preserved.",
            ));
        }
        let started = integer(&marker, "startedAt")?;
        let retained = marker["retainAudio"].as_bool().unwrap_or(true);
        let result = async {
            let directory = session_directory(&vault, &session)?;
            let store = TranscriptStore(runtime.clone());
            store.fold(transcript.clone())?.receive().await?;
            if !retained {
                tokio::task::spawn_blocking(move || delete_capture_audio(&directory).map_err(failure)).await.map_err(failure)??;
                return Err(failure("Interrupted recording: Don't save audio was deleted. Transcript may be incomplete; acknowledge this before starting again."));
            }
            let recovery = Recovery::default();
            let chunks = Recovery::chunks(directory).await?;
            let mut repaired = false;
            for chunk in chunks.into_iter().filter(|chunk| chunk.capture_started_at as i64 >= started) {
                let gap = chunk_interval(&chunk, started);
                recovery.repair(&store, session.clone(), transcript.clone(), chunk, vec![gap], &resolver).await?;
                repaired = true;
            }
            if !repaired { return Err(failure("Interrupted capture has no recoverable chunks; marker retained for review.")); }
            acknowledge_finalization(runtime, session.clone(), transcript.clone()).await
        }.await;
        reports.push(InterruptedCapture {
            session,
            transcript,
            result,
        });
    }
    Ok(reports)
}

pub async fn acknowledge_finalization(
    runtime: &RuntimeHandle,
    session: SessionId,
    transcript: Arc<str>,
) -> Result<()> {
    runtime.submit(move |services| async move {
        services.executor.execute_transaction(vec![
            statement("UPDATE transcripts SET ended_at_ms = COALESCE(ended_at_ms, CAST(unixepoch('now') * 1000 AS INTEGER)) WHERE id = ? AND session_id = ? AND deleted_at IS NULL",
                vec![json!(transcript), json!(session)], Some(1)),
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
            .ok_or_else(|| failure("Invalid recovery chunk path."))?
            .to_path_buf();
        tokio::task::spawn_blocking(move || acknowledge_recovery_chunk(&directory, &chunk.id))
            .await
            .map_err(failure)?
            .map_err(failure)
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
