use std::collections::{HashSet, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anlg_audio_utils::Source;
use anlg_mp3::StereoStreamEncoder;
use ractor::ActorProcessingErr;

use super::super::SAMPLE_RATE;

const CHUNK_SAMPLES: u64 = SAMPLE_RATE as u64 * 60;
const DISK_RESERVE_BYTES: u64 = 256 * 1024 * 1024;
const RECOVERY_BUDGET_BYTES: u64 = 256 * 1024 * 1024;
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

struct EncoderFile {
    encoder: StereoStreamEncoder,
    file: BufWriter<File>,
    output: Vec<u8>,
}

impl EncoderFile {
    fn new(path: &Path, append: bool) -> Result<Self, ActorProcessingErr> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(path)?;
        Ok(Self {
            encoder: StereoStreamEncoder::new(SAMPLE_RATE)?,
            file: BufWriter::new(file),
            output: Vec::new(),
        })
    }

    fn write(&mut self, mic: &[f32], speaker: &[f32]) -> Result<(), ActorProcessingErr> {
        self.output.clear();
        self.encoder.encode_f32(mic, speaker, &mut self.output)?;
        self.file.write_all(&self.output)?;
        Ok(())
    }

    fn finish(mut self) -> Result<(), ActorProcessingErr> {
        self.output.clear();
        self.encoder.flush(&mut self.output)?;
        self.file.write_all(&self.output)?;
        self.file.flush()?;
        self.file.get_ref().sync_all()?;
        Ok(())
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
    chunk: Option<EncoderFile>,
    archive: Option<EncoderFile>,
    last_flush: Instant,
    last_space_check: Instant,
    pub recovered_audio: bool,
}

impl ChunkedSink {
    pub fn new(
        session_dir: &Path,
        capture_started_at: u64,
        offset_ms: u64,
        retain_audio: bool,
    ) -> Result<Self, ActorProcessingErr> {
        std::fs::create_dir_all(session_dir)?;
        if retain_audio && session_dir.join(DELETE_ON_STOP).exists() {
            delete_capture_audio(session_dir)?;
        }
        recover_partial_chunks(session_dir)?;
        check_storage(session_dir)?;
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
        let archive = retain_audio
            .then(|| EncoderFile::new(&session_dir.join("audio.mp3"), true))
            .transpose()?;
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
            last_flush: Instant::now(),
            last_space_check: Instant::now(),
            recovered_audio,
        })
    }

    fn partial_path(&self) -> PathBuf {
        self.dir.join(format!(
            "{}-{}-{}.part",
            self.capture_started_at, self.start_ms, self.audio_start_ms
        ))
    }

    pub fn write(&mut self, mic: &[f32], speaker: &[f32]) -> Result<(), ActorProcessingErr> {
        if self.last_space_check.elapsed() >= Duration::from_secs(5) {
            check_storage(self.dir.parent().unwrap())?;
            self.last_space_check = Instant::now();
        }
        if self.chunk.is_none() {
            self.audio_start_ms = self
                .start_ms
                .saturating_sub(self.history_samples as u64 * 1000 / SAMPLE_RATE as u64);
            let mut chunk = EncoderFile::new(&self.partial_path(), false)?;
            for (mic, speaker) in &self.history {
                chunk.write(mic, speaker)?;
            }
            self.chunk = Some(chunk);
        }
        // Frame-sized input and encoded output are the only in-memory audio.
        if let Some(archive) = &mut self.archive {
            archive.write(mic, speaker)?;
        }
        self.chunk.as_mut().unwrap().write(mic, speaker)?;
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
        if self.last_flush.elapsed() >= Duration::from_secs(1) {
            if let Some(chunk) = &mut self.chunk {
                chunk.file.flush()?;
            }
            if let Some(archive) = &mut self.archive {
                archive.file.flush()?;
            }
            self.last_flush = Instant::now();
        }
        Ok(())
    }

    fn close_chunk(&mut self) -> Result<(), ActorProcessingErr> {
        let Some(chunk) = self.chunk.take() else {
            return Ok(());
        };
        chunk.finish()?;
        let end_ms = self.start_ms + self.chunk_samples * 1000 / SAMPLE_RATE as u64;
        let ready = self.dir.join(format!(
            "{}-{}-{}-{}.mp3",
            self.capture_started_at, self.start_ms, end_ms, self.audio_start_ms
        ));
        std::fs::rename(self.partial_path(), ready)?;
        self.start_ms = end_ms;
        self.chunk_samples = 0;
        Ok(())
    }

    pub fn finish(mut self) -> Result<(), ActorProcessingErr> {
        let chunks = self.close_chunk();
        let archive = self.archive.take().map(EncoderFile::finish).transpose();
        chunks?;
        archive?;
        Ok(())
    }
}

fn check_storage(session_dir: &Path) -> Result<(), ActorProcessingErr> {
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
    storage_budget(available, used)?;
    Ok(())
}

fn storage_budget(available: Option<u64>, used: u64) -> std::io::Result<()> {
    if available.is_some_and(|bytes| bytes < DISK_RESERVE_BYTES) {
        return Err(std::io::Error::other(
            "Low disk space: audio saving paused to leave room for your transcript",
        ));
    }
    if used >= RECOVERY_BUDGET_BYTES {
        return Err(std::io::Error::other(
            "Audio recovery storage is full; unresolved audio has been preserved",
        ));
    }
    Ok(())
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
                let result = delete_capture_audio(&dir);
                on_cleanup(&name, true, &result);
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

    #[test]
    fn compressed_chunks_are_readable_and_acknowledged_independently_of_archive() {
        let dir = tempfile::tempdir().unwrap();
        let mut sink = ChunkedSink::new(dir.path(), 123, 0, true).unwrap();
        for _ in 0..61 {
            sink.write(
                &vec![0.1; SAMPLE_RATE as usize],
                &vec![0.2; SAMPLE_RATE as usize],
            )
            .unwrap();
        }
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

    #[test]
    fn zero_retention_removes_unacknowledged_chunks_and_old_audio() {
        let dir = tempfile::tempdir().unwrap();
        let mut sink = ChunkedSink::new(dir.path(), 123, 0, false).unwrap();
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
    fn hour_long_capture_keeps_a_readable_tail_with_bounded_recovery_storage() {
        let dir = tempfile::tempdir().unwrap();
        let mut sink = ChunkedSink::new(dir.path(), 123, 0, false).unwrap();
        let samples = vec![0.1; SAMPLE_RATE as usize];
        for second in 1..=3661 {
            sink.write(&samples, &samples).unwrap();
            assert!(sink.history_samples <= SAMPLE_RATE as usize * 2);
            if second % 60 == 0 {
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
        let mut sink = ChunkedSink::new(&dir, 123, 0, true).unwrap();
        let samples = vec![0.1; SAMPLE_RATE as usize];
        for _ in 0..70 {
            sink.write(&samples, &samples).unwrap();
        }
        sink.chunk.as_mut().unwrap().file.flush().unwrap();
        assert_eq!(list_recovery_chunks(&dir).unwrap().len(), 1);
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
    fn storage_budget_preserves_database_headroom_and_pending_audio() {
        assert!(storage_budget(Some(DISK_RESERVE_BYTES - 1), 0).is_err());
        assert!(storage_budget(Some(u64::MAX), RECOVERY_BUDGET_BYTES).is_err());
        assert!(storage_budget(Some(DISK_RESERVE_BYTES), 0).is_ok());
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
