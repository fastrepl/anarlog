use std::collections::{HashSet, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use anlg_audio_utils::Source;
use anlg_mp3::StereoStreamEncoder;
use ractor::ActorProcessingErr;

use super::super::SAMPLE_RATE;

const CHUNK_SAMPLES: u64 = SAMPLE_RATE as u64 * 60;
const DISK_RESERVE_BYTES: u64 = 256 * 1024 * 1024;
const RECOVERY_BUDGET_BYTES: u64 = 256 * 1024 * 1024;
const AUDIO_BUFFER_LIMIT_BYTES: usize = 64 * 1024 * 1024;
const STORAGE_CHECK_INTERVAL: Duration = Duration::from_secs(5);
const WRITE_INTERVAL: Duration = Duration::from_secs(1);
const RETRY_INTERVAL: Duration = Duration::from_secs(5);
const FINISH_DEADLINE: Duration = Duration::from_secs(10);
const BUFFER_FULL_ERROR: &str = "Audio buffer is full while storage is unavailable";
const DISK_FULL_ERROR: &str = "Disk is full: audio saving stopped";
const UNSAVED_AUDIO_ERROR: &str = "Buffered audio could not be saved before recording stopped";
const PERSISTENCE_STOPPED_ERROR: &str = "Audio persistence stopped unexpectedly";
const RECOVERY_DIR: &str = "audio-recovery";
pub const DELETE_ON_STOP: &str = ".delete-audio-on-stop";

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RecoveryAudioChunk {
    pub id: String,
    pub path: String,
    pub capture_started_at: u64,
    pub start_ms: u64,
    pub audio_start_ms: u64,
    pub end_ms: u64,
}

/// Encoding stays on the writer thread; encoded segments are handed to the
/// persistence thread, which owns every audio file.
struct Encoder {
    encoder: StereoStreamEncoder,
}

impl Encoder {
    fn new() -> Result<Self, ActorProcessingErr> {
        Ok(Self {
            encoder: StereoStreamEncoder::new(SAMPLE_RATE)?,
        })
    }

    fn encode(&mut self, mic: &[f32], speaker: &[f32]) -> Result<Vec<u8>, ActorProcessingErr> {
        let mut bytes = Vec::new();
        self.encoder.encode_f32(mic, speaker, &mut bytes)?;
        Ok(bytes)
    }

    fn finish(mut self) -> Result<Vec<u8>, ActorProcessingErr> {
        let mut bytes = Vec::new();
        self.encoder.flush(&mut bytes)?;
        Ok(bytes)
    }
}

pub(super) enum StorageHealth {
    Delayed(String),
    Resumed,
    DiskLow,
    DiskOk,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DiskSpace {
    Ample,
    Low,
}

pub(super) struct PersistConfig {
    pub check: fn(&Path) -> std::io::Result<DiskSpace>,
    pub sync: fn(&File) -> std::io::Result<()>,
    pub retry_interval: Duration,
    pub finish_deadline: Duration,
    pub buffer_limit_bytes: usize,
    pub on_health: Arc<dyn Fn(StorageHealth) + Send + Sync>,
}

impl PersistConfig {
    pub fn new(on_health: Arc<dyn Fn(StorageHealth) + Send + Sync>) -> Self {
        Self {
            check: check_storage,
            sync: File::sync_all,
            retry_interval: RETRY_INTERVAL,
            finish_deadline: FINISH_DEADLINE,
            buffer_limit_bytes: AUDIO_BUFFER_LIMIT_BYTES,
            on_health,
        }
    }
}

#[derive(Clone, Copy)]
enum Target {
    Chunk,
    Archive,
}

enum PersistOp {
    OpenChunk {
        partial: PathBuf,
    },
    Append {
        target: Target,
        bytes: Vec<u8>,
    },
    CloseChunk {
        ready: PathBuf,
    },
    Finish,
    #[cfg(test)]
    Barrier(Sender<()>),
}

/// One audio file plus the bytes that are not durable yet. `unsynced` holds
/// everything received since the last successful sync, `written` how much of it
/// reached the file descriptor, and `dirty` that the file content cannot be
/// trusted and has to be rewritten from memory before the next sync.
struct TargetState {
    path: PathBuf,
    append: bool,
    file: Option<File>,
    synced_len: u64,
    unsynced: Vec<u8>,
    written: usize,
    dirty: bool,
}

impl TargetState {
    fn new(path: PathBuf, append: bool) -> Self {
        Self {
            path,
            append,
            file: None,
            synced_len: 0,
            unsynced: Vec::new(),
            written: 0,
            dirty: false,
        }
    }

    fn open(&mut self) -> std::io::Result<&mut File> {
        if self.file.is_none() {
            let file = OpenOptions::new()
                .create(true)
                .write(true)
                .append(self.append)
                .truncate(!self.append)
                .open(&self.path)?;
            if self.append {
                self.synced_len = file.metadata()?.len();
            }
            self.file = Some(file);
        }
        Ok(self.file.as_mut().expect("file is open"))
    }

    fn clean(&self) -> bool {
        !self.dirty && self.written == self.unsynced.len()
    }

    fn write_pending(&mut self) -> std::io::Result<()> {
        if self.clean() {
            return Ok(());
        }
        let result = self.write_now();
        if result.is_err() {
            self.dirty = true;
        }
        result
    }

    fn write_now(&mut self) -> std::io::Result<()> {
        self.open()?;
        let Self {
            file,
            synced_len,
            unsynced,
            written,
            dirty,
            ..
        } = self;
        let file = file.as_mut().expect("file is open");
        if *dirty {
            file.set_len(*synced_len)?;
            file.seek(SeekFrom::Start(*synced_len))?;
            file.write_all(unsynced)?;
        } else {
            file.write_all(&unsynced[*written..])?;
        }
        *written = unsynced.len();
        *dirty = false;
        Ok(())
    }

    fn sync(&mut self, sync: fn(&File) -> std::io::Result<()>) -> std::io::Result<()> {
        let result = self.open().and_then(|file| sync(file));
        if result.is_err() {
            self.dirty = true;
        }
        result
    }

    fn drop_synced(&mut self, buffered: &AtomicUsize) {
        let len = self.unsynced.len();
        self.synced_len += len as u64;
        self.unsynced.clear();
        self.written = 0;
        buffered.fetch_sub(len, Ordering::Relaxed);
    }
}

struct PendingChunk {
    state: TargetState,
    ready: Option<PathBuf>,
}

/// Owns every audio file. Keeps encoded bytes in memory while storage is
/// unhealthy and writes them in order once it recovers, so a slow or full disk
/// delays saving instead of losing audio.
struct Persister {
    session_dir: PathBuf,
    config: PersistConfig,
    buffered: Arc<AtomicUsize>,
    aborted: Arc<OnceLock<String>>,
    archive: Option<TargetState>,
    chunks: VecDeque<PendingChunk>,
    healthy: bool,
    disk_space: DiskSpace,
    finishing: bool,
    last_check: Option<Instant>,
    last_write: Instant,
    last_retry: Instant,
}

impl Persister {
    fn new(
        session_dir: PathBuf,
        retain_audio: bool,
        config: PersistConfig,
        buffered: Arc<AtomicUsize>,
        aborted: Arc<OnceLock<String>>,
    ) -> Self {
        let archive = retain_audio.then(|| TargetState::new(session_dir.join("audio.mp3"), true));
        Self {
            session_dir,
            config,
            buffered,
            aborted,
            archive,
            chunks: VecDeque::new(),
            healthy: true,
            disk_space: DiskSpace::Ample,
            finishing: false,
            last_check: None,
            last_write: Instant::now() - WRITE_INTERVAL,
            last_retry: Instant::now(),
        }
    }

    fn run(mut self, ops: Receiver<PersistOp>) -> Result<(), String> {
        loop {
            if let Some(error) = self.abort_message().map(str::to_owned) {
                match ops.recv_timeout(self.config.retry_interval) {
                    Ok(PersistOp::Append { bytes, .. }) => {
                        self.buffered.fetch_sub(bytes.len(), Ordering::Relaxed);
                    }
                    #[cfg(test)]
                    Ok(PersistOp::Barrier(reply)) => {
                        let _ = reply.send(());
                    }
                    Ok(PersistOp::Finish) | Err(RecvTimeoutError::Disconnected) => {
                        return Err(error);
                    }
                    Ok(PersistOp::OpenChunk { .. } | PersistOp::CloseChunk { .. })
                    | Err(RecvTimeoutError::Timeout) => {}
                }
                continue;
            }
            let timeout = if self.healthy {
                WRITE_INTERVAL
            } else {
                self.config.retry_interval
            };
            match ops.recv_timeout(timeout) {
                Ok(PersistOp::OpenChunk { partial }) => {
                    self.chunks.push_back(PendingChunk {
                        state: TargetState::new(partial, false),
                        ready: None,
                    });
                    self.pump(false);
                }
                Ok(PersistOp::Append { target, bytes }) => {
                    self.append(target, bytes);
                    self.pump(false);
                }
                Ok(PersistOp::CloseChunk { ready }) => {
                    if let Some(chunk) = self
                        .chunks
                        .iter_mut()
                        .rev()
                        .find(|chunk| chunk.ready.is_none())
                    {
                        chunk.ready = Some(ready);
                    }
                    self.pump(true);
                }
                #[cfg(test)]
                Ok(PersistOp::Barrier(reply)) => loop {
                    if self.abort_message().is_some() {
                        let _ = reply.send(());
                        break;
                    }
                    if self.pump(true) {
                        match self.sync_barrier() {
                            Ok(()) => {
                                let _ = reply.send(());
                                break;
                            }
                            Err(error) => self.handle_error(&error),
                        }
                    }
                    std::thread::sleep(self.config.retry_interval);
                },
                Ok(PersistOp::Finish) | Err(RecvTimeoutError::Disconnected) => {
                    return self.finish();
                }
                Err(RecvTimeoutError::Timeout) => {
                    self.pump(false);
                }
            }
        }
    }

    fn append(&mut self, target: Target, bytes: Vec<u8>) {
        let state = match target {
            Target::Chunk => self.chunks.back_mut().map(|chunk| &mut chunk.state),
            Target::Archive => self.archive.as_mut(),
        };
        let Some(state) = state else {
            self.buffered.fetch_sub(bytes.len(), Ordering::Relaxed);
            return;
        };
        state.unsynced.extend_from_slice(&bytes);
    }

    /// Writes and publishes what it can, and returns whether everything
    /// received so far is on disk.
    fn pump(&mut self, force: bool) -> bool {
        if self.abort_message().is_some() {
            return false;
        }
        if !self.healthy {
            if !force && self.last_retry.elapsed() < self.config.retry_interval {
                return false;
            }
            self.last_retry = Instant::now();
        } else if !force && self.last_write.elapsed() < WRITE_INTERVAL && !self.has_ready_chunk() {
            return self.persisted();
        }
        match self.attempt() {
            Ok(()) => {
                self.last_write = Instant::now();
                if !self.healthy {
                    self.healthy = true;
                    (self.config.on_health)(StorageHealth::Resumed);
                }
                self.persisted()
            }
            Err(error) => {
                self.handle_error(&error);
                false
            }
        }
    }

    fn abort_message(&self) -> Option<&str> {
        self.aborted.get().map(String::as_str)
    }

    fn handle_error(&mut self, error: &std::io::Error) {
        if matches!(
            error.kind(),
            std::io::ErrorKind::StorageFull | std::io::ErrorKind::QuotaExceeded
        ) {
            self.aborted.get_or_init(|| DISK_FULL_ERROR.to_owned());
            let mut released = self
                .chunks
                .iter()
                .map(|chunk| chunk.state.unsynced.len())
                .sum::<usize>();
            self.chunks.clear();
            if let Some(archive) = self.archive.take() {
                released += archive.unsynced.len();
            }
            if released > 0 {
                self.buffered.fetch_sub(released, Ordering::Relaxed);
            }
            self.healthy = false;
        } else {
            self.mark_unhealthy(error);
        }
    }

    fn mark_unhealthy(&mut self, error: &std::io::Error) {
        if self.healthy {
            self.healthy = false;
            (self.config.on_health)(StorageHealth::Delayed(error.to_string()));
        }
        self.last_retry = Instant::now();
    }

    #[cfg(test)]
    fn sync_barrier(&mut self) -> std::io::Result<()> {
        self.sync_archive()?;
        let sync = self.config.sync;
        for chunk in &mut self.chunks {
            chunk.state.write_pending()?;
            chunk.state.sync(sync)?;
        }
        Ok(())
    }

    fn attempt(&mut self) -> std::io::Result<()> {
        if let Err(error) = self.check() {
            if let Some(archive) = &mut self.archive
                && !archive.unsynced.is_empty()
            {
                archive.dirty = true;
            }
            for chunk in &mut self.chunks {
                if !chunk.state.unsynced.is_empty() {
                    chunk.state.dirty = true;
                }
            }
            return Err(error);
        }
        if let Some(archive) = &mut self.archive {
            archive.write_pending()?;
        }
        for chunk in &mut self.chunks {
            chunk.state.write_pending()?;
        }
        while self
            .chunks
            .front()
            .is_some_and(|chunk| chunk.ready.is_some())
        {
            self.publish_front()?;
        }
        if self.finishing && self.chunks.is_empty() {
            self.sync_archive()?;
        }
        Ok(())
    }

    fn check(&mut self) -> std::io::Result<DiskSpace> {
        if self.healthy
            && self
                .last_check
                .is_some_and(|checked| checked.elapsed() < STORAGE_CHECK_INTERVAL)
        {
            return Ok(self.disk_space);
        }
        let disk_space = (self.config.check)(&self.session_dir)?;
        self.last_check = Some(Instant::now());
        if disk_space != self.disk_space {
            let event = match disk_space {
                DiskSpace::Ample => StorageHealth::DiskOk,
                DiskSpace::Low => StorageHealth::DiskLow,
            };
            (self.config.on_health)(event);
            self.disk_space = disk_space;
        }
        Ok(disk_space)
    }

    /// The archive is durable before the chunk that covers the same audio, so a
    /// published chunk never describes audio the archive is missing.
    fn publish_front(&mut self) -> std::io::Result<()> {
        self.sync_archive()?;
        let sync = self.config.sync;
        let chunk = self.chunks.front_mut().expect("a chunk is ready");
        chunk.state.write_pending()?;
        chunk.state.sync(sync)?;
        let ready = chunk.ready.clone().expect("a chunk is ready");
        if let Err(error) = std::fs::rename(&chunk.state.path, &ready) {
            chunk.state.dirty = true;
            return Err(error);
        }
        self.buffered
            .fetch_sub(chunk.state.unsynced.len(), Ordering::Relaxed);
        self.chunks.pop_front();
        Ok(())
    }

    fn sync_archive(&mut self) -> std::io::Result<()> {
        let sync = self.config.sync;
        let Some(archive) = &mut self.archive else {
            return Ok(());
        };
        if archive.unsynced.is_empty() {
            return Ok(());
        }
        archive.write_pending()?;
        archive.sync(sync)?;
        archive.drop_synced(&self.buffered);
        Ok(())
    }

    fn has_ready_chunk(&self) -> bool {
        self.chunks.iter().any(|chunk| chunk.ready.is_some())
    }

    fn persisted(&self) -> bool {
        self.healthy
            && !self.has_ready_chunk()
            && self.chunks.iter().all(|chunk| chunk.state.clean())
            && self.archive.as_ref().is_none_or(TargetState::clean)
    }

    fn saved(&self) -> bool {
        self.chunks.is_empty()
            && self
                .archive
                .as_ref()
                .is_none_or(|archive| archive.unsynced.is_empty())
    }

    fn finish(mut self) -> Result<(), String> {
        if let Some(error) = self.abort_message() {
            return Err(error.to_owned());
        }
        self.finishing = true;
        let deadline = Instant::now() + self.config.finish_deadline;
        loop {
            self.pump(true);
            if let Some(error) = self.abort_message() {
                return Err(error.to_owned());
            }
            if self.saved() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(UNSAVED_AUDIO_ERROR.to_owned());
            }
            std::thread::sleep(self.config.retry_interval);
        }
    }
}

pub(super) struct ChunkedSink {
    dir: PathBuf,
    capture_started_at: u64,
    start_ms: u64,
    chunk_samples: u64,
    audio_start_ms: u64,
    history: VecDeque<(Vec<f32>, Vec<f32>)>,
    history_samples: usize,
    chunk: Option<Encoder>,
    archive: Option<Encoder>,
    ops: Option<Sender<PersistOp>>,
    persistence: Option<std::thread::JoinHandle<Result<(), String>>>,
    buffered: Arc<AtomicUsize>,
    aborted: Arc<OnceLock<String>>,
    buffer_limit_bytes: usize,
    pub recovered_audio: bool,
}

impl ChunkedSink {
    pub fn new(
        session_dir: &Path,
        capture_started_at: u64,
        offset_ms: u64,
        retain_audio: bool,
        config: PersistConfig,
    ) -> Result<Self, ActorProcessingErr> {
        std::fs::create_dir_all(session_dir)?;
        recover_partial_chunks(session_dir)?;
        if retain_audio && session_dir.join(DELETE_ON_STOP).exists() {
            // Earlier zero-retention audio stays until its transcript is recovered.
            if has_pending_recovery_audio(session_dir)? {
                std::fs::remove_file(session_dir.join(DELETE_ON_STOP))?;
            } else {
                delete_capture_audio(session_dir)?;
            }
        }
        if !retain_audio {
            File::create(session_dir.join(DELETE_ON_STOP))?.sync_all()?;
        }
        let mut recovered_audio = false;
        // Keep the existing interrupted-WAV recovery path for older recordings.
        if retain_audio
            && !session_dir.join("audio.mp3").exists()
            && (session_dir.join("audio.wav").exists() || session_dir.join("audio.ogg").exists())
        {
            let mut legacy = super::disk::create_disk_sink(session_dir)?;
            recovered_audio = legacy.recovered_audio;
            super::disk::finalize_disk_sink(&mut legacy)?;
        }
        let dir = session_dir.join(RECOVERY_DIR);
        std::fs::create_dir_all(&dir)?;
        let archive = retain_audio.then(Encoder::new).transpose()?;
        let buffered = Arc::new(AtomicUsize::new(0));
        let aborted = Arc::new(OnceLock::new());
        let buffer_limit_bytes = config.buffer_limit_bytes;
        let (ops, receiver) = std::sync::mpsc::channel();
        let persister = Persister::new(
            session_dir.to_path_buf(),
            retain_audio,
            config,
            buffered.clone(),
            aborted.clone(),
        );
        let persistence = std::thread::spawn(move || persister.run(receiver));
        Ok(Self {
            dir,
            capture_started_at,
            start_ms: offset_ms,
            chunk_samples: 0,
            audio_start_ms: offset_ms,
            history: VecDeque::new(),
            history_samples: 0,
            chunk: None,
            archive,
            ops: Some(ops),
            persistence: Some(persistence),
            buffered,
            aborted,
            buffer_limit_bytes,
            recovered_audio,
        })
    }

    fn partial_path(&self) -> PathBuf {
        self.dir.join(format!(
            "{}-{}-{}.part",
            self.capture_started_at, self.start_ms, self.audio_start_ms
        ))
    }

    fn send(&self, op: PersistOp) -> Result<(), ActorProcessingErr> {
        if let Some(error) = self.aborted.get() {
            return Err(std::io::Error::other(error.clone()).into());
        }
        self.ops
            .as_ref()
            .ok_or_else(|| std::io::Error::other(PERSISTENCE_STOPPED_ERROR))?
            .send(op)
            .map_err(|_| std::io::Error::other(PERSISTENCE_STOPPED_ERROR))?;
        Ok(())
    }

    /// Encoded audio waits in memory until the persistence thread saves it.
    /// Only a full buffer loses audio; flush tails at close are always kept.
    fn push(
        &self,
        target: Target,
        bytes: Vec<u8>,
        bounded: bool,
    ) -> Result<(), ActorProcessingErr> {
        if let Some(error) = self.aborted.get() {
            return Err(std::io::Error::other(error.clone()).into());
        }
        if bytes.is_empty() {
            return Ok(());
        }
        if bounded && self.buffered.load(Ordering::Relaxed) + bytes.len() > self.buffer_limit_bytes
        {
            return Err(std::io::Error::other(BUFFER_FULL_ERROR).into());
        }
        let len = bytes.len();
        self.buffered.fetch_add(len, Ordering::Relaxed);
        if let Err(error) = self.send(PersistOp::Append { target, bytes }) {
            self.buffered.fetch_sub(len, Ordering::Relaxed);
            return Err(error);
        }
        Ok(())
    }

    #[cfg(test)]
    fn barrier(&self) -> Result<(), ActorProcessingErr> {
        let (reply, done) = std::sync::mpsc::channel();
        self.send(PersistOp::Barrier(reply))?;
        done.recv()
            .map_err(|_| std::io::Error::other(PERSISTENCE_STOPPED_ERROR))?;
        Ok(())
    }

    pub fn write(&mut self, mic: &[f32], speaker: &[f32]) -> Result<(), ActorProcessingErr> {
        if self.chunk.is_none() {
            self.audio_start_ms = self
                .start_ms
                .saturating_sub(self.history_samples as u64 * 1000 / SAMPLE_RATE as u64);
            self.send(PersistOp::OpenChunk {
                partial: self.partial_path(),
            })?;
            let mut chunk = Encoder::new()?;
            let mut history = Vec::new();
            for (mic, speaker) in &self.history {
                history.push(chunk.encode(mic, speaker)?);
            }
            self.chunk = Some(chunk);
            for bytes in history {
                self.push(Target::Chunk, bytes, true)?;
            }
        }
        // Frame-sized input and encoded output are the only in-memory audio.
        if let Some(archive) = &mut self.archive {
            let bytes = archive.encode(mic, speaker)?;
            self.push(Target::Archive, bytes, true)?;
        }
        let bytes = self.chunk.as_mut().unwrap().encode(mic, speaker)?;
        self.push(Target::Chunk, bytes, true)?;
        let frames = mic.len().max(speaker.len()) as u64;
        self.chunk_samples += frames;
        let limit = SAMPLE_RATE as usize * 2;
        let mic = mic[mic.len().saturating_sub(limit)..].to_vec();
        let speaker = speaker[speaker.len().saturating_sub(limit)..].to_vec();
        self.history_samples += mic.len().max(speaker.len());
        self.history.push_back((mic, speaker));
        while self.history_samples > limit {
            let (mic, speaker) = self.history.pop_front().unwrap();
            self.history_samples -= mic.len().max(speaker.len());
        }
        if self.chunk_samples >= CHUNK_SAMPLES {
            self.close_chunk()?;
        }
        Ok(())
    }

    // A chunk is published (renamed from .part) only after it is durable. The
    // persistence thread syncs and renames, so a slow disk never stalls
    // encoding, and an unpublished .part is recovered at the next startup.
    fn close_chunk(&mut self) -> Result<(), ActorProcessingErr> {
        let Some(chunk) = self.chunk.take() else {
            return Ok(());
        };
        self.push(Target::Chunk, chunk.finish()?, false)?;
        let end_ms = self.start_ms + self.chunk_samples * 1000 / SAMPLE_RATE as u64;
        let ready = self.dir.join(format!(
            "{}-{}-{}-{}.mp3",
            self.capture_started_at, self.start_ms, end_ms, self.audio_start_ms
        ));
        let closed = self.send(PersistOp::CloseChunk { ready });
        self.start_ms = end_ms;
        self.chunk_samples = 0;
        closed
    }

    pub fn finish(mut self) -> Result<(), ActorProcessingErr> {
        let chunks = self.close_chunk();
        let archive = self
            .archive
            .take()
            .map(|archive| self.push(Target::Archive, archive.finish()?, false))
            .transpose();
        let sent = self.send(PersistOp::Finish);
        self.ops.take();
        let saved = self
            .persistence
            .take()
            .map(|persistence| {
                persistence
                    .join()
                    .map_err(|_| std::io::Error::other("audio persistence thread panicked"))?
                    .map_err(std::io::Error::other)
            })
            .transpose();
        chunks?;
        archive?;
        sent?;
        saved?;
        Ok(())
    }
}

fn check_storage(session_dir: &Path) -> std::io::Result<DiskSpace> {
    let canonical = session_dir.canonicalize()?;
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let available = disks
        .list()
        .iter()
        .filter(|disk| {
            disk.mount_point()
                .canonicalize()
                .is_ok_and(|mount| canonical.starts_with(mount))
        })
        .max_by_key(|disk| disk.mount_point().components().count())
        .map(|disk| disk.available_space());
    let used = std::fs::read_dir(session_dir.join(RECOVERY_DIR))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| entry.metadata().ok())
        .map(|meta| meta.len())
        .sum::<u64>();
    storage_budget(available, used)
}

fn storage_budget(available: Option<u64>, used: u64) -> std::io::Result<DiskSpace> {
    if used >= RECOVERY_BUDGET_BYTES {
        return Err(std::io::Error::other(
            "Audio recovery storage is full; unresolved audio has been preserved",
        ));
    }
    Ok(
        if available.is_some_and(|bytes| bytes < DISK_RESERVE_BYTES) {
            DiskSpace::Low
        } else {
            DiskSpace::Ample
        },
    )
}

pub fn list_recovery_chunks(session_dir: &Path) -> std::io::Result<Vec<RecoveryAudioChunk>> {
    let dir = session_dir.join(RECOVERY_DIR);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut chunks = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let id = entry.file_name().to_string_lossy().into_owned();
        let Some(stem) = id.strip_suffix(".mp3") else {
            continue;
        };
        let parts: Vec<_> = stem.split('-').collect();
        if parts.len() != 4 {
            continue;
        }
        let (Ok(capture_started_at), Ok(start_ms), Ok(end_ms), Ok(audio_start_ms)) = (
            parts[0].parse::<u64>(),
            parts[1].parse::<u64>(),
            parts[2].parse::<u64>(),
            parts[3].parse::<u64>(),
        ) else {
            continue;
        };
        chunks.push(RecoveryAudioChunk {
            id,
            path: entry.path().to_string_lossy().into_owned(),
            capture_started_at,
            start_ms,
            end_ms,
            audio_start_ms,
        });
        // A caller fetches only a bounded page of metadata, never the audio.
        chunks.sort_unstable_by_key(|chunk| (chunk.capture_started_at, chunk.start_ms));
        chunks.truncate(128);
    }
    Ok(chunks)
}

pub fn acknowledge_recovery_chunk(session_dir: &Path, id: &str) -> std::io::Result<()> {
    if !id.ends_with(".mp3")
        || !id
            .bytes()
            .all(|c| c.is_ascii_digit() || matches!(c, b'-' | b'.' | b'm' | b'p'))
    {
        return Err(std::io::Error::other("Invalid recovery chunk"));
    }
    match std::fs::remove_file(session_dir.join(RECOVERY_DIR).join(id)) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

pub fn delete_capture_audio(session_dir: &Path) -> std::io::Result<()> {
    if !session_dir.try_exists()? {
        return Ok(());
    }
    for entry in std::fs::read_dir(session_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == RECOVERY_DIR {
            std::fs::remove_dir_all(entry.path())?;
        } else if (matches!(name.as_ref(), "audio.mp3.tmp" | "audio.wav.tmp")
            || ((name.starts_with("audio.") || name.starts_with("audio_"))
                && matches!(
                    entry.path().extension().and_then(|ext| ext.to_str()),
                    Some("wav" | "mp3" | "ogg" | "opus" | "m4a" | "flac")
                )))
            && entry.file_type()?.is_file()
        {
            std::fs::remove_file(entry.path())?;
        }
    }
    match std::fs::remove_file(session_dir.join(DELETE_ON_STOP)) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

/// Deletes zero-retention audio only after every recovery chunk was acknowledged.
/// Returns `false` while chunks still wait for transcription.
/// Call only after the session's writer has stopped.
pub fn delete_transcribed_capture_audio(session_dir: &Path) -> std::io::Result<bool> {
    recover_partial_chunks(session_dir)?;
    if has_pending_recovery_audio(session_dir)? {
        return Ok(false);
    }
    delete_capture_audio(session_dir)?;
    Ok(true)
}

/// Published chunks or unfinished audio that could not be decoded yet.
fn has_pending_recovery_audio(session_dir: &Path) -> std::io::Result<bool> {
    if !list_recovery_chunks(session_dir)?.is_empty() {
        return Ok(true);
    }
    let entries = match std::fs::read_dir(session_dir.join(RECOVERY_DIR)) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && is_recovery_partial_name(&entry.file_name().to_string_lossy())
            && entry.metadata()?.len() > 0
            && partial_is_unreadable(&entry.path())
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn is_recovery_partial_name(name: &str) -> bool {
    name.strip_suffix(".part").is_some_and(|stem| {
        let parts: Vec<_> = stem.split('-').collect();
        parts.len() == 3 && parts.iter().all(|part| part.parse::<u64>().is_ok())
    })
}

fn partial_is_unreadable(path: &Path) -> bool {
    anlg_audio_utils::source_from_path(path).is_err()
}

// Call only before a writer starts or during application startup. Active .part
// files must stay invisible to recovery workers until their writer closes them.
fn recover_partial_chunks(session_dir: &Path) -> std::io::Result<()> {
    let dir = session_dir.join(RECOVERY_DIR);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(stem) = name.strip_suffix(".part") else {
            continue;
        };
        let parts: Vec<_> = stem.split('-').collect();
        let [capture, start, audio_start] = parts.as_slice() else {
            continue;
        };
        let (Ok(capture), Ok(start), Ok(audio_start)) = (
            capture.parse::<u64>(),
            start.parse::<u64>(),
            audio_start.parse::<u64>(),
        ) else {
            continue;
        };
        let source = match anlg_audio_utils::source_from_path(entry.path()) {
            Ok(source) => source,
            Err(error) => {
                tracing::warn!(?error, path = ?entry.path(), "partial_audio_unreadable");
                continue;
            }
        };
        let rate = u32::from(source.sample_rate()) as u64;
        let channels = u16::from(source.channels()) as u64;
        // Decode a stream to measure its playable tail without loading it into RAM.
        let end = audio_start.saturating_add(source.count() as u64 * 1000 / rate / channels);
        if end <= start {
            continue;
        }
        std::fs::rename(
            entry.path(),
            dir.join(format!("{capture}-{start}-{end}-{audio_start}.mp3")),
        )?;
    }
    Ok(())
}

pub fn recover_interrupted_captures(sessions_dir: &Path) -> std::io::Result<()> {
    recover_interrupted_captures_except(sessions_dir, &HashSet::new(), &mut |_, _, _| {})
        .map(|_| ())
}

pub(crate) fn recover_interrupted_captures_except(
    sessions_dir: &Path,
    active_sessions: &HashSet<String>,
    on_cleanup: &mut impl FnMut(&str, bool, &std::io::Result<()>),
) -> std::io::Result<bool> {
    if !sessions_dir.try_exists()? {
        return Ok(false);
    }
    let mut first_error = None;
    let mut deferred = false;
    for entry in std::fs::read_dir(sessions_dir)? {
        let result = (|| {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                return Ok(());
            }
            let dir = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if active_sessions.contains(&name) {
                deferred |= dir.join(DELETE_ON_STOP).try_exists()?;
                return Ok(());
            }
            if dir.join(DELETE_ON_STOP).try_exists()? {
                let result = delete_transcribed_capture_audio(&dir);
                let deleting = !matches!(result, Ok(false));
                let result = result.map(|_| ());
                on_cleanup(&name, deleting, &result);
                result
            } else if uuid::Uuid::parse_str(&name).is_ok() {
                let result = recover_partial_chunks(&dir);
                on_cleanup(&name, false, &result);
                result
            } else {
                deferred |= recover_interrupted_captures_except(&dir, active_sessions, on_cleanup)?;
                Ok(())
            }
        })();
        if let Err(error) = result {
            tracing::warn!(?error, "interrupted_capture_cleanup_failed");
            first_error.get_or_insert(error);
        }
    }
    first_error.map_or(Ok(deferred), Err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    static RETAINED_SYNCS: AtomicUsize = AtomicUsize::new(0);
    static BARRIER_SYNCS: AtomicUsize = AtomicUsize::new(0);
    static OUTAGE: AtomicBool = AtomicBool::new(false);
    static FAILED_SYNCS: AtomicUsize = AtomicUsize::new(0);

    fn check_ok(_: &Path) -> std::io::Result<DiskSpace> {
        Ok(DiskSpace::Ample)
    }

    fn check_outage(_: &Path) -> std::io::Result<DiskSpace> {
        if OUTAGE.load(Ordering::SeqCst) {
            Err(std::io::Error::other("storage unavailable"))
        } else {
            Ok(DiskSpace::Ample)
        }
    }

    fn check_overflow_outage(_: &Path) -> std::io::Result<DiskSpace> {
        Err(std::io::Error::other("storage unavailable"))
    }

    fn check_low_disk(_: &Path) -> std::io::Result<DiskSpace> {
        Ok(DiskSpace::Low)
    }

    fn counted_retained_sync(file: &File) -> std::io::Result<()> {
        file.sync_all()?;
        RETAINED_SYNCS.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn counted_barrier_sync(file: &File) -> std::io::Result<()> {
        file.sync_all()?;
        BARRIER_SYNCS.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn slow_sync(file: &File) -> std::io::Result<()> {
        std::thread::sleep(Duration::from_millis(500));
        file.sync_all()
    }

    fn fail_first_three_syncs(file: &File) -> std::io::Result<()> {
        if FAILED_SYNCS.fetch_add(1, Ordering::SeqCst) < 3 {
            Err(std::io::Error::other("temporary sync failure"))
        } else {
            file.sync_all()
        }
    }

    fn storage_full_sync(_: &File) -> std::io::Result<()> {
        Err(std::io::Error::from(std::io::ErrorKind::StorageFull))
    }

    fn test_config() -> PersistConfig {
        PersistConfig {
            check: check_ok,
            sync: File::sync_all,
            retry_interval: Duration::from_millis(20),
            finish_deadline: Duration::from_millis(200),
            buffer_limit_bytes: AUDIO_BUFFER_LIMIT_BYTES,
            on_health: Arc::new(|_| {}),
        }
    }

    fn new_sink(
        session_dir: &Path,
        capture_started_at: u64,
        offset_ms: u64,
        retain_audio: bool,
    ) -> Result<ChunkedSink, ActorProcessingErr> {
        ChunkedSink::new(
            session_dir,
            capture_started_at,
            offset_ms,
            retain_audio,
            test_config(),
        )
    }

    #[test]
    fn retained_chunk_publication_syncs_the_archive_and_the_chunk() {
        RETAINED_SYNCS.store(0, Ordering::SeqCst);
        let dir = tempfile::tempdir().unwrap();
        let mut config = test_config();
        config.sync = counted_retained_sync;
        let mut sink = ChunkedSink::new(dir.path(), 123, 0, true, config).unwrap();
        write_one_chunk(&mut sink);
        sink.barrier().unwrap();

        assert_eq!(RETAINED_SYNCS.load(Ordering::SeqCst), 2);
        assert_eq!(list_recovery_chunks(dir.path()).unwrap().len(), 1);
        sink.finish().unwrap();
    }

    #[test]
    fn compressed_chunks_are_readable_and_acknowledged_independently_of_archive() {
        BARRIER_SYNCS.store(0, Ordering::SeqCst);
        let dir = tempfile::tempdir().unwrap();
        let mut config = test_config();
        config.sync = counted_barrier_sync;
        let mut sink = ChunkedSink::new(dir.path(), 123, 0, true, config).unwrap();
        for _ in 0..61 {
            sink.write(
                &vec![0.1; SAMPLE_RATE as usize],
                &vec![0.2; SAMPLE_RATE as usize],
            )
            .unwrap();
        }
        sink.barrier().unwrap();
        assert_eq!(BARRIER_SYNCS.load(Ordering::SeqCst), 4);
        let chunks = list_recovery_chunks(dir.path()).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!((chunks[0].start_ms, chunks[0].end_ms), (0, 60_000));
        let wav = dir.path().join("decoded.wav");
        anlg_mp3::decode_to_wav(Path::new(&chunks[0].path), &wav).unwrap();
        assert!(hound::WavReader::open(wav).unwrap().duration() >= SAMPLE_RATE * 59);
        acknowledge_recovery_chunk(dir.path(), &chunks[0].id).unwrap();
        sink.finish().unwrap();
        assert_eq!(list_recovery_chunks(dir.path()).unwrap().len(), 1);
        assert!(dir.path().join("audio.mp3").metadata().unwrap().len() < 2_000_000);
    }

    fn write_one_chunk(sink: &mut ChunkedSink) {
        let samples = vec![0.1; SAMPLE_RATE as usize];
        for _ in 0..60 {
            sink.write(&samples, &samples).unwrap();
        }
    }

    #[test]
    fn slow_chunk_sync_neither_blocks_the_writer_nor_publishes_early() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = test_config();
        config.sync = slow_sync;
        config.finish_deadline = Duration::from_secs(2);
        let mut sink = ChunkedSink::new(dir.path(), 123, 0, false, config).unwrap();
        let started = Instant::now();
        write_one_chunk(&mut sink);
        assert!(started.elapsed() < Duration::from_millis(500));
        assert!(list_recovery_chunks(dir.path()).unwrap().is_empty());
        sink.write(&[0.1; 160], &[0.1; 160]).unwrap();
        sink.finish().unwrap();
        let chunks = list_recovery_chunks(dir.path()).unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!((chunks[0].start_ms, chunks[0].end_ms), (0, 60_000));
        assert!(
            std::fs::read_dir(dir.path().join(RECOVERY_DIR))
                .unwrap()
                .all(|e| e.unwrap().path().extension().unwrap() == "mp3")
        );
    }

    #[test]
    fn failed_chunk_sync_is_retried_without_losing_audio() {
        FAILED_SYNCS.store(0, Ordering::SeqCst);
        let dir = tempfile::tempdir().unwrap();
        let mut config = test_config();
        config.sync = fail_first_three_syncs;
        let mut sink = ChunkedSink::new(dir.path(), 123, 0, false, config).unwrap();
        write_one_chunk(&mut sink);
        let samples = vec![0.1; SAMPLE_RATE as usize];
        for _ in 0..3 {
            sink.write(&samples, &samples).unwrap();
        }
        sink.barrier().unwrap();
        let chunks = list_recovery_chunks(dir.path()).unwrap();
        assert_eq!(chunks.len(), 1);
        let wav = dir.path().join("decoded.wav");
        anlg_mp3::decode_to_wav(Path::new(&chunks[0].path), &wav).unwrap();
        assert!(hound::WavReader::open(wav).unwrap().duration() >= SAMPLE_RATE * 59);
        sink.finish().unwrap();
    }

    #[test]
    fn low_disk_keeps_saving_audio_and_warns() {
        let dir = tempfile::tempdir().unwrap();
        let health = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut config = test_config();
        config.check = check_low_disk;
        let reported = Arc::clone(&health);
        config.on_health = Arc::new(move |event| {
            reported.lock().unwrap().push(match event {
                StorageHealth::Delayed(_) => "Delayed",
                StorageHealth::Resumed => "Resumed",
                StorageHealth::DiskLow => "DiskLow",
                StorageHealth::DiskOk => "DiskOk",
            });
        });
        let mut sink = ChunkedSink::new(dir.path(), 123, 0, false, config).unwrap();
        let samples = vec![0.1; SAMPLE_RATE as usize];
        for _ in 0..61 {
            sink.write(&samples, &samples).unwrap();
        }
        sink.barrier().unwrap();

        assert_eq!(list_recovery_chunks(dir.path()).unwrap().len(), 1);
        assert_eq!(*health.lock().unwrap(), vec!["DiskLow"]);
        sink.finish().unwrap();
    }

    #[test]
    fn full_disk_aborts_saving_without_buffering() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = test_config();
        config.sync = storage_full_sync;
        let mut sink = ChunkedSink::new(dir.path(), 123, 0, false, config).unwrap();
        let buffered = Arc::clone(&sink.buffered);
        let samples = vec![0.1; SAMPLE_RATE as usize];
        let mut write_error = None;
        for _ in 0..300 {
            if let Err(error) = sink.write(&samples, &samples) {
                write_error = Some(error.to_string());
                break;
            }
        }

        assert!(
            write_error
                .as_deref()
                .is_some_and(|error| error.contains(DISK_FULL_ERROR))
        );
        let started = Instant::now();
        let finish = sink.finish();
        assert_eq!(finish.unwrap_err().to_string(), DISK_FULL_ERROR);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(buffered.load(Ordering::Relaxed), 0);
        assert!(list_recovery_chunks(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn storage_outage_buffers_audio_and_publishes_it_in_order_after_recovery() {
        OUTAGE.store(true, Ordering::SeqCst);
        let dir = tempfile::tempdir().unwrap();
        let health = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut config = test_config();
        config.check = check_outage;
        config.buffer_limit_bytes = 64 * 1024 * 1024;
        let reported = Arc::clone(&health);
        config.on_health = Arc::new(move |event| {
            reported.lock().unwrap().push(match event {
                StorageHealth::Delayed(_) => "Delayed",
                StorageHealth::Resumed => "Resumed",
                StorageHealth::DiskLow => "DiskLow",
                StorageHealth::DiskOk => "DiskOk",
            });
        });
        let mut sink = ChunkedSink::new(dir.path(), 123, 0, true, config).unwrap();
        let samples = vec![0.1; SAMPLE_RATE as usize];
        sink.write(&samples, &samples).unwrap();
        for _ in 0..100 {
            if health.lock().unwrap().as_slice() == ["Delayed"] {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(*health.lock().unwrap(), vec!["Delayed"]);
        for _ in 1..150 {
            sink.write(&samples, &samples).unwrap();
        }
        assert!(list_recovery_chunks(dir.path()).unwrap().is_empty());
        OUTAGE.store(false, Ordering::SeqCst);
        sink.barrier().unwrap();
        let chunks = list_recovery_chunks(dir.path()).unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!((chunks[0].start_ms, chunks[0].end_ms), (0, 60_000));
        assert_eq!(
            (
                chunks[1].start_ms,
                chunks[1].end_ms,
                chunks[1].audio_start_ms
            ),
            (60_000, 120_000, 58_000)
        );
        assert_eq!(*health.lock().unwrap(), vec!["Delayed", "Resumed"]);
        sink.finish().unwrap();
        let chunks = list_recovery_chunks(dir.path()).unwrap();
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[2].start_ms, 120_000);
        let wav = dir.path().join("archive.wav");
        anlg_mp3::decode_to_wav(&dir.path().join("audio.mp3"), &wav).unwrap();
        assert!(hound::WavReader::open(wav).unwrap().duration() >= SAMPLE_RATE * 149);
    }

    #[test]
    fn buffer_overflow_fails_the_writer_and_finish_reports_unsaved_audio() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = test_config();
        config.check = check_overflow_outage;
        config.buffer_limit_bytes = 256 * 1024;
        let mut sink = ChunkedSink::new(dir.path(), 123, 0, false, config).unwrap();
        let samples = vec![0.1; SAMPLE_RATE as usize];
        let mut write_error = None;
        for _ in 0..600 {
            if let Err(error) = sink.write(&samples, &samples) {
                write_error = Some(error.to_string());
                break;
            }
        }
        let started = Instant::now();
        let finish = sink.finish();
        assert!(
            write_error
                .as_deref()
                .is_some_and(|error| error.contains(BUFFER_FULL_ERROR))
        );
        assert!(finish.is_err());
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(list_recovery_chunks(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn zero_retention_removes_unacknowledged_chunks_and_old_audio() {
        let dir = tempfile::tempdir().unwrap();
        let mut sink = new_sink(dir.path(), 123, 0, false).unwrap();
        sink.write(&vec![0.1; SAMPLE_RATE as usize], &[]).unwrap();
        sink.finish().unwrap();
        std::fs::write(dir.path().join("audio.recovery-old.wav"), b"old").unwrap();
        std::fs::write(dir.path().join("audio.mp3.tmp"), b"old conversion").unwrap();
        std::fs::write(dir.path().join("note.md"), b"keep").unwrap();
        delete_capture_audio(dir.path()).unwrap();
        assert!(list_recovery_chunks(dir.path()).unwrap().is_empty());
        assert!(!dir.path().join("audio.recovery-old.wav").exists());
        assert!(!dir.path().join("audio.mp3.tmp").exists());
        assert!(dir.path().join("note.md").exists());
    }

    #[test]
    fn zero_retention_keeps_untranscribed_chunks_until_acknowledged() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join(uuid::Uuid::new_v4().to_string());
        let mut sink = new_sink(&dir, 123, 0, false).unwrap();
        sink.write(&vec![0.1; SAMPLE_RATE as usize], &[]).unwrap();
        sink.finish().unwrap();
        assert!(!delete_transcribed_capture_audio(&dir).unwrap());
        let part = dir.join(RECOVERY_DIR).join("123-60000-60000.part");
        std::fs::write(&part, b"unpublished").unwrap();
        let chunks = list_recovery_chunks(&dir).unwrap();
        for chunk in &chunks {
            acknowledge_recovery_chunk(&dir, &chunk.id).unwrap();
        }
        assert!(!delete_transcribed_capture_audio(&dir).unwrap());
        assert!(part.exists());
        std::fs::write(&part, b"").unwrap();
        assert!(delete_transcribed_capture_audio(&dir).unwrap());
        assert!(!part.exists());
        let mut sink = new_sink(&dir, 124, 0, false).unwrap();
        sink.write(&vec![0.1; SAMPLE_RATE as usize], &[]).unwrap();
        sink.finish().unwrap();
        recover_interrupted_captures(root.path()).unwrap();
        let chunks = list_recovery_chunks(&dir).unwrap();
        assert_eq!(chunks.len(), 1);
        acknowledge_recovery_chunk(&dir, &chunks[0].id).unwrap();
        assert!(delete_transcribed_capture_audio(&dir).unwrap());
        assert!(!dir.join(RECOVERY_DIR).exists());
        assert!(!dir.join(DELETE_ON_STOP).exists());
    }

    #[test]
    fn later_capture_appends_to_untranscribed_zero_retention_audio() {
        let dir = tempfile::tempdir().unwrap();
        let mut sink = new_sink(dir.path(), 123, 0, false).unwrap();
        sink.write(&vec![0.1; SAMPLE_RATE as usize], &[]).unwrap();
        sink.finish().unwrap();
        let mut sink = new_sink(dir.path(), 200_000, 1_000, true).unwrap();
        sink.write(&vec![0.1; SAMPLE_RATE as usize], &[]).unwrap();
        sink.finish().unwrap();
        let chunks = list_recovery_chunks(dir.path()).unwrap();
        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.capture_started_at)
                .collect::<Vec<_>>(),
            vec![123, 200_000]
        );
        assert!(!dir.path().join(DELETE_ON_STOP).exists());
    }

    #[test]
    fn hour_long_capture_keeps_a_readable_tail_with_bounded_recovery_storage() {
        let dir = tempfile::tempdir().unwrap();
        let mut sink = new_sink(dir.path(), 123, 0, false).unwrap();
        let samples = vec![0.1; SAMPLE_RATE as usize];
        for second in 1..=3661 {
            sink.write(&samples, &samples).unwrap();
            assert!(sink.history_samples <= SAMPLE_RATE as usize * 2);
            if second % 60 == 0 {
                sink.barrier().unwrap();
                let chunks = list_recovery_chunks(dir.path()).unwrap();
                assert_eq!(chunks.len(), 1);
                assert_eq!(chunks[0].end_ms, second * 1000);
                acknowledge_recovery_chunk(dir.path(), &chunks[0].id).unwrap();
            }
        }
        sink.finish().unwrap();
        let chunks = list_recovery_chunks(dir.path()).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(
            (chunks[0].start_ms, chunks[0].end_ms),
            (3_660_000, 3_661_000)
        );
        let decoded = dir.path().join("tail.wav");
        anlg_mp3::decode_to_wav(Path::new(&chunks[0].path), &decoded).unwrap();
        assert!(hound::WavReader::open(decoded).unwrap().duration() >= SAMPLE_RATE);
    }

    #[test]
    fn startup_recovers_a_playable_partial_chunk_with_its_overlap_offset() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join(uuid::Uuid::new_v4().to_string());
        let mut sink = new_sink(&dir, 123, 0, true).unwrap();
        let samples = vec![0.1; SAMPLE_RATE as usize];
        for _ in 0..70 {
            sink.write(&samples, &samples).unwrap();
        }
        sink.barrier().unwrap();
        assert_eq!(list_recovery_chunks(&dir).unwrap().len(), 1);
        sink.chunk.take();
        sink.archive.take();
        sink.ops.take();
        assert!(sink.persistence.take().unwrap().join().unwrap().is_err());
        drop(sink);
        recover_interrupted_captures(root.path()).unwrap();
        let chunks = list_recovery_chunks(&dir).unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[1].start_ms, 60_000);
        assert_eq!(chunks[1].audio_start_ms, 58_000);
        assert!((69_000..=70_100).contains(&chunks[1].end_ms));
        assert!(
            anlg_audio_utils::source_from_path(&chunks[1].path)
                .unwrap()
                .count()
                > 0
        );
    }

    #[test]
    fn partial_recovery_failure_reports_its_session() {
        let root = tempfile::tempdir().unwrap();
        let session_id = uuid::Uuid::new_v4().to_string();
        let dir = root.path().join(&session_id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(RECOVERY_DIR), b"not a directory").unwrap();
        let mut failures = Vec::new();
        assert!(
            recover_interrupted_captures_except(
                root.path(),
                &HashSet::new(),
                &mut |id, deleting, result| {
                    assert!(!deleting);
                    if result.is_err() {
                        failures.push(id.to_string());
                    }
                },
            )
            .is_err()
        );
        assert_eq!(failures, vec![session_id]);
    }

    #[test]
    fn cleanup_continues_after_a_session_cannot_be_deleted() {
        let root = tempfile::tempdir().unwrap();
        let broken = root.path().join(uuid::Uuid::new_v4().to_string());
        let zero = root.path().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(broken.join(DELETE_ON_STOP)).unwrap();
        std::fs::create_dir_all(&zero).unwrap();
        std::fs::write(zero.join(DELETE_ON_STOP), b"").unwrap();
        std::fs::write(zero.join("audio.mp3"), b"private").unwrap();
        assert!(recover_interrupted_captures(root.path()).is_err());
        assert!(!zero.join("audio.mp3").exists());
        assert!(broken.join(DELETE_ON_STOP).exists());
    }

    #[test]
    fn storage_budget_reports_low_disk_and_preserves_the_recovery_budget() {
        assert_eq!(
            storage_budget(Some(DISK_RESERVE_BYTES - 1), 0).unwrap(),
            DiskSpace::Low
        );
        assert_eq!(
            storage_budget(Some(DISK_RESERVE_BYTES), 0).unwrap(),
            DiskSpace::Ample
        );
        assert!(storage_budget(Some(u64::MAX), RECOVERY_BUDGET_BYTES).is_err());
    }

    #[test]
    fn startup_removes_only_interrupted_zero_retention_recordings() {
        let dir = tempfile::tempdir().unwrap();
        let zero = dir.path().join(uuid::Uuid::new_v4().to_string());
        let retained = dir.path().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(zero.join(RECOVERY_DIR)).unwrap();
        std::fs::create_dir_all(&retained).unwrap();
        std::fs::write(zero.join(DELETE_ON_STOP), b"").unwrap();
        std::fs::write(
            zero.join(RECOVERY_DIR).join("unfinished.part"),
            b"private audio",
        )
        .unwrap();
        std::fs::write(retained.join("audio.mp3"), b"keep").unwrap();
        recover_interrupted_captures(dir.path()).unwrap();
        assert!(!zero.join(RECOVERY_DIR).exists());
        assert_eq!(std::fs::read(retained.join("audio.mp3")).unwrap(), b"keep");
    }
}
