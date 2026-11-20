// Opt-in dictation recovery. Behavior follows Char Next's pending dictations
// (char/desktop-dictation/src/main/pending.ts, d4608723): private per-dictation folders, only the
// signed-in account's records are listed or sent and other accounts' are deleted, audio is
// deleted once its text is saved and the record once its text is claimed. Anarlog adds a record
// bound, a storage bound, expiry and the captured local-only policy, and leaves transcription to
// the app's existing batch path.
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::engine::{DictationError, DictationPolicy, DictationSurface, private_dir};

pub const MAX_RECORDS: usize = 20;
pub const MAX_BYTES: u64 = 100 * 1024 * 1024;
pub const EXPIRY_MS: u64 = 24 * 60 * 60 * 1_000;
const MAX_TEXT_BYTES: usize = 100_000;
const META: &str = "meta.json";
pub const AUDIO: &str = "audio.wav";
pub const SUFFIX: &str = "suffix.wav";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum RecordState {
    /// An operation of this process is still writing it.
    Active,
    Audio,
    Text,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Meta {
    account_id: String,
    surface: DictationSurface,
    created_at: u64,
    duration_ms: u64,
    policy: DictationPolicy,
    state: RecordState,
    text: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DictationRecoveryState {
    Audio,
    Text,
}

#[derive(Clone, Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DictationRecoveryItem {
    pub id: String,
    pub surface: DictationSurface,
    pub created_at: u64,
    pub expires_at: u64,
    pub duration_ms: u64,
    pub state: DictationRecoveryState,
    pub text: Option<String>,
    pub policy: DictationPolicy,
}

pub struct RecoveryStore {
    root: PathBuf,
    /// Space held for each record still being written: a full recording plus its suffix.
    reserve_bytes: u64,
    swept: Mutex<bool>,
}

impl RecoveryStore {
    pub fn new(root: PathBuf, reserve_bytes: u64) -> Self {
        Self {
            root,
            reserve_bytes,
            swept: Mutex::new(false),
        }
    }

    /// Deletes expired records and finishes what an earlier process left behind.
    pub fn prune(&self, now: u64) {
        drop(self.sweep(now));
    }

    /// Creates a record for a starting operation, or None when the store is full. Nothing already
    /// kept is evicted to make room.
    pub fn reserve(
        &self,
        id: &str,
        account_id: &str,
        surface: DictationSurface,
        policy: &DictationPolicy,
        now: u64,
    ) -> Result<Option<PathBuf>, DictationError> {
        let _swept = self.sweep(now)?;
        let records = self.records();
        let used: u64 = records
            .iter()
            .map(|(directory, meta)| {
                let size = directory_size(directory);
                if meta.state == RecordState::Active {
                    size.max(self.reserve_bytes)
                } else {
                    size
                }
            })
            .sum();
        if records.len() >= MAX_RECORDS || used + self.reserve_bytes > MAX_BYTES {
            return Ok(None);
        }
        let directory = self.root.join(id);
        private_dir(&directory).map_err(DictationError::recording)?;
        write_meta(
            &directory,
            &Meta {
                account_id: account_id.to_string(),
                surface,
                created_at: now,
                duration_ms: 0,
                policy: policy.clone(),
                state: RecordState::Active,
                text: None,
            },
        )?;
        Ok(Some(directory))
    }

    /// Keeps a finished operation's record: its text when it has one (audio deleted), else audio.
    pub fn keep(
        &self,
        id: &str,
        duration_ms: u64,
        text: Option<String>,
    ) -> Result<(), DictationError> {
        let _swept = self.lock();
        let directory = self.root.join(id);
        let mut meta = read_meta(&directory).ok_or_else(DictationError::recovery_not_found)?;
        let _ = std::fs::remove_file(directory.join(SUFFIX));
        meta.duration_ms = duration_ms;
        match text {
            Some(text) if !text.trim().is_empty() => {
                meta.state = RecordState::Text;
                meta.text = Some(text);
                write_meta(&directory, &meta)?;
                let _ = std::fs::remove_file(directory.join(AUDIO));
            }
            Some(_) => remove_record(&directory),
            None => {
                meta.state = RecordState::Audio;
                write_meta(&directory, &meta)?;
            }
        }
        Ok(())
    }

    pub fn remove(&self, id: &str) {
        let _swept = self.lock();
        remove_record(&self.root.join(id));
    }

    /// Deletes every record of `account_id`, as when that account signs out.
    pub fn purge_account(&self, account_id: &str) {
        let _swept = self.lock();
        for (directory, meta) in self.records() {
            if meta.account_id == account_id {
                remove_record(&directory);
            }
        }
    }

    pub fn list(
        &self,
        account_id: &str,
        now: u64,
    ) -> Result<Vec<DictationRecoveryItem>, DictationError> {
        let _swept = self.sweep(now)?;
        let mut items = Vec::new();
        for (directory, meta) in self.records() {
            if meta.state == RecordState::Active {
                continue;
            }
            if meta.account_id != account_id {
                remove_record(&directory);
                continue;
            }
            let Some(id) = directory.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            items.push(DictationRecoveryItem {
                id: id.to_string(),
                surface: meta.surface,
                created_at: meta.created_at,
                expires_at: meta.created_at + EXPIRY_MS,
                duration_ms: meta.duration_ms,
                state: if meta.state == RecordState::Text {
                    DictationRecoveryState::Text
                } else {
                    DictationRecoveryState::Audio
                },
                text: meta.text,
                policy: meta.policy,
            });
        }
        items.sort_by_key(|item| item.created_at);
        Ok(items)
    }

    /// The kept audio for a retry under `policy`. A recording captured local-only is never handed
    /// to a remote route, whatever the current preference says.
    pub fn prepare_audio(
        &self,
        account_id: &str,
        id: &str,
        policy: &DictationPolicy,
        now: u64,
    ) -> Result<String, DictationError> {
        let _swept = self.sweep(now)?;
        let (directory, meta) = self.owned(account_id, id, RecordState::Audio)?;
        if meta.policy.local_only && !policy.local_only {
            return Err(DictationError::policy(
                "This recording was captured for on-device transcription only.",
            ));
        }
        Ok(directory.join(AUDIO).to_string_lossy().into_owned())
    }

    pub fn save_text(
        &self,
        account_id: &str,
        id: &str,
        text: String,
        now: u64,
    ) -> Result<(), DictationError> {
        if text.len() > MAX_TEXT_BYTES || text.contains('\0') {
            return Err(DictationError::invalid("Recovered text is too long."));
        }
        let _swept = self.sweep(now)?;
        let (directory, mut meta) = self.owned(account_id, id, RecordState::Audio)?;
        if text.trim().is_empty() {
            remove_record(&directory);
            return Ok(());
        }
        meta.state = RecordState::Text;
        meta.text = Some(text);
        write_meta(&directory, &meta)?;
        let _ = std::fs::remove_file(directory.join(AUDIO));
        Ok(())
    }

    pub fn claim(&self, account_id: &str, id: &str, now: u64) -> Result<String, DictationError> {
        let _swept = self.sweep(now)?;
        let (directory, meta) = self.owned(account_id, id, RecordState::Text)?;
        remove_record(&directory);
        meta.text.ok_or_else(DictationError::recovery_not_found)
    }

    pub fn discard(&self, account_id: &str, id: &str, now: u64) -> Result<(), DictationError> {
        let _swept = self.sweep(now)?;
        let directory = self.root.join(checked_id(id)?);
        match read_meta(&directory) {
            Some(meta) if meta.account_id == account_id && meta.state != RecordState::Active => {
                remove_record(&directory);
                Ok(())
            }
            _ => Err(DictationError::recovery_not_found()),
        }
    }

    fn owned(
        &self,
        account_id: &str,
        id: &str,
        state: RecordState,
    ) -> Result<(PathBuf, Meta), DictationError> {
        let directory = self.root.join(checked_id(id)?);
        match read_meta(&directory) {
            Some(meta) if meta.account_id == account_id && meta.state == state => {
                Ok((directory, meta))
            }
            _ => Err(DictationError::recovery_not_found()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, bool> {
        self.swept.lock().unwrap_or_else(|error| error.into_inner())
    }

    /// Takes the store lock. The first time in a process it also finishes what an earlier process
    /// left: torn records are deleted and interrupted recordings become readable audio records.
    /// Expired records are deleted every time.
    fn sweep(&self, now: u64) -> Result<std::sync::MutexGuard<'_, bool>, DictationError> {
        let mut swept = self.lock();
        if !self.root.exists() {
            *swept = true;
            return Ok(swept);
        }
        let first = !*swept;
        *swept = true;
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return Ok(swept);
        };
        for entry in entries.flatten() {
            let directory = entry.path();
            if !directory.is_dir() {
                continue;
            }
            let Some(mut meta) = read_meta(&directory) else {
                if first || checked_id(&entry.file_name().to_string_lossy()).is_err() {
                    remove_record(&directory);
                }
                continue;
            };
            if now.saturating_sub(meta.created_at) >= EXPIRY_MS {
                remove_record(&directory);
                continue;
            }
            if first && meta.state == RecordState::Active {
                let _ = std::fs::remove_file(directory.join(SUFFIX));
                match crate::engine::wav::repair(&directory.join(AUDIO)) {
                    Ok(samples) if samples > 0 => {
                        meta.state = RecordState::Audio;
                        meta.duration_ms = crate::engine::wav::duration_ms(samples);
                        write_meta(&directory, &meta)?;
                    }
                    _ => remove_record(&directory),
                }
            }
        }
        Ok(swept)
    }

    fn records(&self) -> Vec<(PathBuf, Meta)> {
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return Vec::new();
        };
        entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .filter_map(|path| read_meta(&path).map(|meta| (path, meta)))
            .collect()
    }
}

fn checked_id(id: &str) -> Result<&str, DictationError> {
    if crate::engine::is_operation_id(id) {
        Ok(id)
    } else {
        Err(DictationError::recovery_not_found())
    }
}

fn read_meta(directory: &Path) -> Option<Meta> {
    serde_json::from_slice(&std::fs::read(directory.join(META)).ok()?).ok()
}

fn write_meta(directory: &Path, meta: &Meta) -> Result<(), DictationError> {
    let temporary = directory.join(format!("{META}.tmp"));
    let _ = std::fs::remove_file(&temporary);
    let mut file = crate::engine::private_file(&temporary).map_err(DictationError::recording)?;
    let bytes = serde_json::to_vec(meta)
        .map_err(|error| DictationError::recording(std::io::Error::other(error)))?;
    std::io::Write::write_all(&mut file, &bytes).map_err(DictationError::recording)?;
    file.sync_all().map_err(DictationError::recording)?;
    std::fs::rename(&temporary, directory.join(META)).map_err(DictationError::recording)
}

fn remove_record(directory: &Path) {
    let _ = std::fs::remove_dir_all(directory);
}

fn directory_size(directory: &Path) -> u64 {
    std::fs::read_dir(directory)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| entry.metadata().ok())
                .map(|metadata| metadata.len())
                .sum()
        })
        .unwrap_or(0)
}
