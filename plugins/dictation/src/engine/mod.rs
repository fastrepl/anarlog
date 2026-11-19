//! Dictation operations: one microphone, one operation per surface, each bound to an account,
//! a captured provider policy and a fresh id. Every step checks that identity, so a cancelled,
//! retired or superseded dictation can neither upload nor deliver text.
//!
//! Early audio, explicit readiness, the finalize deadline and phrase fallback follow Char's
//! dictation composer (char/chat-context-react/src/composer/{use-dictation,dictation-audio,
//! dictation-stream}.ts, d4608723), adapted to native capture and Anarlog's provider adapters.
mod capture;
mod phrases;
mod stream;
pub(crate) mod wav;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::ducking::OutputDucker;
use crate::lease::MicLease;
use crate::preview::PreviewConfig;
use crate::recovery::{AUDIO, RecoveryStore, SUFFIX};

const CHUNK_SIZE: usize = 1_600;
const MAX_RECORDING_SECONDS: u64 = 300;
const EARLY_FRAMES: usize = 300;
const MAX_TEXT_BYTES: usize = 100_000;
const RETIRED_IDS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DictationSurface {
    System,
    Chat,
}

/// The transcription route captured when the dictation starts. Live streaming, the batch
/// fallback and a recovery retry must all stay on it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DictationPolicy {
    pub provider: String,
    pub model: Option<String>,
    pub local_only: bool,
}

#[derive(Clone, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BeginDictation {
    pub operation_id: String,
    pub surface: DictationSurface,
    pub account_id: String,
    pub microphone_device: Option<String>,
    pub policy: DictationPolicy,
    pub stream: Option<PreviewConfig>,
    pub recovery: bool,
    pub ducking: bool,
}

#[derive(Clone, Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DictationTicket {
    pub operation_id: String,
    pub generation: u64,
    pub recovery: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CaptureEndReason {
    Limit,
    Failed,
}

#[derive(Clone, Debug, Serialize, specta::Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DictationUpdate {
    Ready,
    Amplitude { amplitude: f64 },
    Transcript { text: String, partial: String },
    PreviewUnavailable,
    CaptureEnded { reason: CaptureEndReason },
}

#[derive(Clone, Debug, PartialEq, Serialize, specta::Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DictationFinish {
    #[serde(rename_all = "camelCase")]
    Final { text: String, duration_ms: u64 },
    #[serde(rename_all = "camelCase")]
    Suffix {
        committed_text: String,
        audio_path: String,
        duration_ms: u64,
    },
    #[serde(rename_all = "camelCase")]
    Full {
        audio_path: String,
        duration_ms: u64,
    },
    #[serde(rename_all = "camelCase")]
    Silent { duration_ms: u64 },
}

impl DictationFinish {
    fn duration_ms(&self) -> u64 {
        match self {
            Self::Final { duration_ms, .. }
            | Self::Suffix { duration_ms, .. }
            | Self::Full { duration_ms, .. }
            | Self::Silent { duration_ms } => *duration_ms,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DictationErrorCode {
    Busy,
    Stale,
    Cancelled,
    Policy,
    Capture,
    Recording,
    RecoveryUnavailable,
    RecoveryNotFound,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, specta::Type)]
pub struct DictationError {
    pub code: DictationErrorCode,
    pub message: String,
}

impl DictationError {
    fn new(code: DictationErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn busy() -> Self {
        Self::new(DictationErrorCode::Busy, "Voice input is already active.")
    }

    fn stale() -> Self {
        Self::new(DictationErrorCode::Stale, "This dictation has ended.")
    }

    fn cancelled() -> Self {
        Self::new(
            DictationErrorCode::Cancelled,
            "This dictation was cancelled.",
        )
    }

    pub(crate) fn policy(message: &str) -> Self {
        Self::new(DictationErrorCode::Policy, message)
    }

    pub(crate) fn invalid(message: &str) -> Self {
        Self::new(DictationErrorCode::Invalid, message)
    }

    pub(crate) fn recording(error: std::io::Error) -> Self {
        Self::new(DictationErrorCode::Recording, error.to_string())
    }

    pub(crate) fn recovery_not_found() -> Self {
        Self::new(
            DictationErrorCode::RecoveryNotFound,
            "This saved dictation is no longer available.",
        )
    }
}

impl std::fmt::Display for DictationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

#[derive(Clone, Copy)]
pub struct Limits {
    pub max_samples: u64,
    pub early_frames: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_samples: MAX_RECORDING_SECONDS * u64::from(wav::SAMPLE_RATE),
            early_frames: EARLY_FRAMES,
        }
    }
}

impl Limits {
    fn recording_bytes(&self) -> u64 {
        self.max_samples * 2 + 44
    }
}

#[derive(Clone, Debug, Deserialize, specta::Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RetireReason {
    Meeting,
    #[serde(rename_all = "camelCase")]
    Account {
        account_id: String,
    },
}

type CaptureTask =
    tauri::async_runtime::JoinHandle<Result<capture::Captured, capture::CaptureError>>;

enum State {
    Capturing {
        stop: CancellationToken,
        attach: mpsc::UnboundedSender<PreviewConfig>,
        stream_attached: bool,
        task: CaptureTask,
    },
    Finishing,
    Finished(DictationFinish),
    Completed {
        text: String,
        duration_ms: u64,
    },
}

struct Operation {
    surface: DictationSurface,
    account_id: String,
    policy: DictationPolicy,
    directory: PathBuf,
    recovery: bool,
    cancel: CancellationToken,
    state: State,
}

#[derive(Default)]
struct Inner {
    generation: u64,
    operations: HashMap<String, Operation>,
    /// Operations opening the microphone, by id, with their account.
    opening: HashMap<String, String>,
    retired: VecDeque<String>,
}

impl Inner {
    fn retire(&mut self, id: &str) {
        if self.retired.iter().any(|retired| retired == id) {
            return;
        }
        if self.retired.len() >= RETIRED_IDS {
            self.retired.pop_front();
        }
        self.retired.push_back(id.to_string());
    }

    fn is_retired(&self, id: &str) -> bool {
        self.retired.iter().any(|retired| retired == id)
    }
}

pub struct Engine {
    inner: Mutex<Inner>,
    lease: MicLease,
    recovery: Option<RecoveryStore>,
    ducker: Arc<dyn OutputDucker>,
    limits: Limits,
}

impl Engine {
    pub fn new(
        recovery_root: Option<PathBuf>,
        ducker: Arc<dyn OutputDucker>,
        limits: Limits,
    ) -> Self {
        Self {
            inner: Mutex::default(),
            lease: MicLease::default(),
            recovery: recovery_root
                .map(|root| RecoveryStore::new(root, limits.recording_bytes() * 2)),
            ducker,
            limits,
        }
    }

    pub fn recovery(&self) -> Result<&RecoveryStore, DictationError> {
        self.recovery.as_ref().ok_or_else(|| {
            DictationError::new(
                DictationErrorCode::RecoveryUnavailable,
                "Dictation recovery is unavailable.",
            )
        })
    }

    pub fn begin(
        &self,
        request: BeginDictation,
        audio: Arc<dyn anlg_audio::AudioProvider>,
        updates: Channel<DictationUpdate>,
    ) -> Result<DictationTicket, DictationError> {
        let id = request.operation_id.clone();
        if !is_operation_id(&id) || request.account_id.trim().is_empty() {
            return Err(DictationError::invalid("Invalid dictation request."));
        }
        if let Some(stream) = &request.stream {
            check_stream(&request.policy, stream)?;
        }

        let (generation, guard, superseded) = {
            let mut inner = self.lock();
            if inner.is_retired(&id) {
                return Err(DictationError::cancelled());
            }
            if inner.operations.contains_key(&id) || inner.opening.contains_key(&id) {
                return Err(DictationError::invalid("This dictation already started."));
            }
            if !inner.opening.is_empty()
                || inner
                    .operations
                    .values()
                    .any(|operation| matches!(operation.state, State::Capturing { .. }))
            {
                return Err(DictationError::busy());
            }
            let guard = self.lease.try_acquire().ok_or_else(DictationError::busy)?;
            // A new press on the same surface starts over whatever it was still finishing; an
            // operation of another account cannot outlive the switch to this one.
            let superseded: Vec<String> = inner
                .operations
                .iter()
                .filter(|(_, operation)| {
                    operation.surface == request.surface
                        || operation.account_id != request.account_id
                })
                .map(|(id, _)| id.clone())
                .collect();
            let superseded: Vec<(String, Operation)> = superseded
                .into_iter()
                .filter_map(|id| {
                    inner.retire(&id);
                    inner
                        .operations
                        .remove(&id)
                        .map(|operation| (id, operation))
                })
                .collect();
            inner.opening.insert(id.clone(), request.account_id.clone());
            inner.generation += 1;
            (inner.generation, guard, superseded)
        };
        for (superseded_id, operation) in superseded {
            // Never capturing (checked above); `finish` cleans up one that is finishing.
            operation.cancel.cancel();
            if !matches!(operation.state, State::Finishing) {
                self.remove_files(&superseded_id, &operation.directory, operation.recovery);
            }
        }

        let opened = self.open(&request, audio);
        let duck = opened
            .as_ref()
            .ok()
            .and_then(|_| request.ducking.then(|| self.ducker.duck()).flatten());
        let mut inner = self.lock();
        inner.opening.remove(&id);
        let (directory, recovery, microphone, writer) = opened?;
        if inner.is_retired(&id) {
            drop(inner);
            drop((microphone, writer, guard, duck));
            self.remove_files(&id, &directory, recovery);
            return Err(DictationError::cancelled());
        }

        let stop = CancellationToken::new();
        let cancel = CancellationToken::new();
        let (attach, attached) = mpsc::unbounded_channel();
        let stream_attached = request.stream.is_some();
        let task = tauri::async_runtime::spawn(capture::run(
            capture::Capture {
                microphone,
                guard,
                duck,
                writer,
                stream: request.stream,
                updates,
                limits: self.limits,
            },
            stop.clone(),
            cancel.clone(),
            attached,
        ));
        inner.operations.insert(
            id.clone(),
            Operation {
                surface: request.surface,
                account_id: request.account_id,
                policy: request.policy,
                directory,
                recovery,
                cancel,
                state: State::Capturing {
                    stop,
                    attach,
                    stream_attached,
                    task,
                },
            },
        );
        Ok(DictationTicket {
            operation_id: id,
            generation,
            recovery,
        })
    }

    pub fn attach_stream(&self, id: &str, stream: PreviewConfig) -> Result<(), DictationError> {
        let mut inner = self.lock();
        let operation = inner
            .operations
            .get_mut(id)
            .ok_or_else(DictationError::stale)?;
        check_stream(&operation.policy, &stream)?;
        let State::Capturing {
            attach,
            stream_attached,
            ..
        } = &mut operation.state
        else {
            return Err(DictationError::stale());
        };
        if *stream_attached {
            return Err(DictationError::invalid("A stream is already attached."));
        }
        *stream_attached = true;
        attach.send(stream).map_err(|_| DictationError::stale())
    }

    pub async fn finish(&self, id: &str) -> Result<DictationFinish, DictationError> {
        let (task, directory, recovery) = {
            let mut inner = self.lock();
            let operation = inner
                .operations
                .get_mut(id)
                .ok_or_else(DictationError::stale)?;
            if !matches!(operation.state, State::Capturing { .. }) {
                return Err(DictationError::stale());
            }
            let State::Capturing { stop, task, .. } =
                std::mem::replace(&mut operation.state, State::Finishing)
            else {
                unreachable!()
            };
            stop.cancel();
            (task, operation.directory.clone(), operation.recovery)
        };

        let result = match task.await {
            Ok(Ok(captured)) => outcome(captured, &directory),
            Ok(Err(capture::CaptureError::Cancelled)) => Err(DictationError::stale()),
            Ok(Err(capture::CaptureError::Recording(message))) => {
                Err(DictationError::new(DictationErrorCode::Recording, message))
            }
            Err(error) => Err(DictationError::new(
                DictationErrorCode::Recording,
                error.to_string(),
            )),
        };

        let mut inner = self.lock();
        let finish = match (inner.operations.get_mut(id), result) {
            (Some(operation), Ok(finish)) => {
                operation.state = State::Finished(finish.clone());
                return Ok(finish);
            }
            // Cancelled or retired while finishing: whoever removed it left the files to us.
            (None, _) => Err(DictationError::stale()),
            (Some(_), Err(error)) => {
                inner.operations.remove(id);
                inner.retire(id);
                Err(error)
            }
        };
        drop(inner);
        self.remove_files(id, &directory, recovery);
        finish
    }

    /// The one final text: stream text as is, committed words plus the suffix retry, or the full
    /// retry in place of anything the stream showed.
    pub fn complete(&self, id: &str, batch_text: Option<String>) -> Result<String, DictationError> {
        if batch_text
            .as_ref()
            .is_some_and(|text| text.len() > MAX_TEXT_BYTES || text.contains('\0'))
        {
            return Err(DictationError::invalid("Transcript is too long."));
        }
        let mut inner = self.lock();
        let operation = inner
            .operations
            .get_mut(id)
            .ok_or_else(DictationError::stale)?;
        let State::Finished(finish) = &operation.state else {
            return Err(DictationError::stale());
        };
        let text = match (finish, batch_text) {
            (DictationFinish::Final { text, .. }, None) => text.trim().to_string(),
            (DictationFinish::Suffix { committed_text, .. }, Some(batch)) => {
                dictation_core::join_transcript(committed_text.trim(), &batch)
            }
            (DictationFinish::Full { .. }, Some(batch)) => batch.trim().to_string(),
            (DictationFinish::Silent { .. }, None) => String::new(),
            _ => {
                return Err(DictationError::invalid(
                    "Batch text does not match how this dictation finished.",
                ));
            }
        };
        operation.state = State::Completed {
            text: text.clone(),
            duration_ms: finish.duration_ms(),
        };
        Ok(text)
    }

    /// Ends an operation. With `keep_for_recovery` and a recovery-enabled operation, its text (or
    /// its audio when it never completed) stays in the recovery store; otherwise every file it
    /// owns is deleted.
    pub fn release(
        &self,
        id: &str,
        keep_for_recovery: bool,
    ) -> Result<Option<String>, DictationError> {
        let operation = {
            let mut inner = self.lock();
            match inner.operations.get(id).map(|operation| &operation.state) {
                None => return Err(DictationError::stale()),
                Some(State::Capturing { .. } | State::Finishing) => {
                    return Err(DictationError::invalid(
                        "This dictation is still recording.",
                    ));
                }
                Some(_) => {}
            }
            inner.retire(id);
            inner.operations.remove(id).expect("operation exists")
        };
        if !keep_for_recovery {
            self.discard_files(id, &operation);
            return Ok(None);
        }
        let store = match (&self.recovery, operation.recovery) {
            (Some(store), true) => store,
            _ => {
                self.discard_files(id, &operation);
                return Err(DictationError::new(
                    DictationErrorCode::RecoveryUnavailable,
                    "Recovery was not enabled for this dictation.",
                ));
            }
        };
        let (duration_ms, text) = match operation.state {
            State::Completed { text, duration_ms } => (duration_ms, Some(text)),
            State::Finished(finish) => (finish.duration_ms(), None),
            _ => unreachable!(),
        };
        store.keep(id, duration_ms, text)?;
        Ok(Some(id.to_string()))
    }

    /// The account of an operation whose final text is ready and not yet released: the only
    /// time its captured field takes a delivery. Cancelling, superseding or retiring it ends that.
    pub fn deliverable(&self, id: &str) -> Option<String> {
        self.lock()
            .operations
            .get(id)
            .filter(|operation| matches!(operation.state, State::Completed { .. }))
            .map(|operation| operation.account_id.clone())
    }

    pub async fn cancel(&self, id: &str) {
        let operation = {
            let mut inner = self.lock();
            inner.retire(id);
            inner.operations.remove(id)
        };
        if let Some(operation) = operation {
            self.stop(id, operation).await;
        }
    }

    pub async fn retire(&self, reason: RetireReason) {
        let operations: Vec<(String, Operation)> = {
            let mut inner = self.lock();
            let opening: Vec<String> = inner
                .opening
                .iter()
                .filter(|(_, account)| match &reason {
                    RetireReason::Meeting => true,
                    RetireReason::Account { account_id } => *account == account_id,
                })
                .map(|(id, _)| id.clone())
                .collect();
            for id in opening {
                inner.retire(&id);
            }
            let ids: Vec<String> = inner
                .operations
                .iter()
                .filter(|(_, operation)| match &reason {
                    RetireReason::Meeting => true,
                    RetireReason::Account { account_id } => &operation.account_id == account_id,
                })
                .map(|(id, _)| id.clone())
                .collect();
            ids.into_iter()
                .filter_map(|id| {
                    inner.retire(&id);
                    inner
                        .operations
                        .remove(&id)
                        .map(|operation| (id, operation))
                })
                .collect()
        };
        for (id, operation) in operations {
            self.stop(&id, operation).await;
        }
        if let (RetireReason::Account { account_id }, Some(store)) = (&reason, &self.recovery) {
            store.purge_account(account_id);
        }
    }

    fn open(
        &self,
        request: &BeginDictation,
        audio: Arc<dyn anlg_audio::AudioProvider>,
    ) -> Result<(PathBuf, bool, anlg_audio::CaptureStream, wav::WavWriter), DictationError> {
        let id = &request.operation_id;
        let reserved = match (&self.recovery, request.recovery) {
            (Some(store), true) => store.reserve(
                id,
                &request.account_id,
                request.surface,
                &request.policy,
                now_ms(),
            )?,
            _ => None,
        };
        let recovery = reserved.is_some();
        let directory = match reserved {
            Some(directory) => directory,
            None => tempfile::Builder::new()
                .prefix("anarlog-dictation-")
                .tempdir()
                .map_err(DictationError::recording)?
                .keep(),
        };
        let cleanup = |error: DictationError| {
            if recovery {
                if let Some(store) = &self.recovery {
                    store.remove(id);
                }
            } else {
                let _ = std::fs::remove_dir_all(&directory);
            }
            error
        };
        let writer = wav::WavWriter::create(&directory.join(AUDIO))
            .map_err(|error| cleanup(DictationError::recording(error)))?;
        let microphone = audio
            .open_mic_capture(
                request.microphone_device.clone(),
                wav::SAMPLE_RATE,
                CHUNK_SIZE,
            )
            .map_err(|error| {
                cleanup(DictationError::new(
                    DictationErrorCode::Capture,
                    error.to_string(),
                ))
            })?;
        Ok((directory, recovery, microphone, writer))
    }

    async fn stop(&self, id: &str, operation: Operation) {
        operation.cancel.cancel();
        match operation.state {
            State::Capturing { task, .. } => {
                let _ = task.await;
            }
            // `finish` owns the task and deletes the files once it sees the operation is gone.
            State::Finishing => return,
            _ => {}
        }
        self.remove_files(id, &operation.directory, operation.recovery);
    }

    fn discard_files(&self, id: &str, operation: &Operation) {
        self.remove_files(id, &operation.directory, operation.recovery);
    }

    fn remove_files(&self, id: &str, directory: &Path, recovery: bool) {
        if recovery {
            if let Some(store) = &self.recovery {
                store.remove(id);
            }
        } else {
            let _ = std::fs::remove_dir_all(directory);
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|error| error.into_inner())
    }
}

fn outcome(
    captured: capture::Captured,
    directory: &Path,
) -> Result<DictationFinish, DictationError> {
    let duration_ms = wav::duration_ms(captured.samples);
    let audio_path = directory.join(AUDIO).to_string_lossy().into_owned();
    let voiced = captured.phrases.voiced();
    let Some(report) = captured.stream else {
        return Ok(if voiced {
            DictationFinish::Full {
                audio_path,
                duration_ms,
            }
        } else {
            DictationFinish::Silent { duration_ms }
        });
    };
    if report.complete {
        let text = report.text.trim().to_string();
        return Ok(if text.is_empty() {
            DictationFinish::Silent { duration_ms }
        } else {
            DictationFinish::Final { text, duration_ms }
        });
    }
    if let Some((committed_text, boundary)) = report.committed
        && !committed_text.trim().is_empty()
        && let Some(suffix) = captured.phrases.suffix(boundary)
    {
        if suffix.is_empty() {
            return Ok(DictationFinish::Final {
                text: committed_text.trim().to_string(),
                duration_ms,
            });
        }
        let path = directory.join(SUFFIX);
        wav::write_all(&path, &suffix).map_err(DictationError::recording)?;
        return Ok(DictationFinish::Suffix {
            committed_text: committed_text.trim().to_string(),
            audio_path: path.to_string_lossy().into_owned(),
            duration_ms,
        });
    }
    Ok(if voiced || !report.text.trim().is_empty() {
        DictationFinish::Full {
            audio_path,
            duration_ms,
        }
    } else {
        DictationFinish::Silent { duration_ms }
    })
}

/// Live streaming must stay on the captured provider, and a local-only dictation never streams
/// anywhere but this machine.
fn check_stream(policy: &DictationPolicy, stream: &PreviewConfig) -> Result<(), DictationError> {
    if stream.provider != policy.provider {
        return Err(DictationError::policy(
            "The live stream does not match the selected transcription provider.",
        ));
    }
    if policy.local_only && !is_loopback(&stream.base_url) {
        return Err(DictationError::policy(
            "On-device dictation cannot stream to a remote server.",
        ));
    }
    Ok(())
}

fn is_loopback(base_url: &str) -> bool {
    let Ok(url) = url::Url::parse(base_url) else {
        return false;
    };
    match url.host() {
        Some(url::Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    }
}

pub(crate) fn is_operation_id(id: &str) -> bool {
    id.len() == 36
        && id
            .chars()
            .all(|character| character == '-' || character.is_ascii_hexdigit())
}

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub(crate) fn private_dir(path: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(path)?;
    #[cfg(unix)]
    std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o700))?;
    Ok(())
}

pub(crate) fn private_file(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}
