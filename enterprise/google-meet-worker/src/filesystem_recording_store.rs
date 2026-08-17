use std::{
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{RecordingChunkStore, StoredRecordingObject};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

pub struct FilesystemRecordingStore {
    root: PathBuf,
}

impl FilesystemRecordingStore {
    pub async fn new(root: impl Into<PathBuf>) -> Result<Self, FilesystemRecordingStoreError> {
        let root = root.into();
        if !root.is_absolute() {
            return Err(FilesystemRecordingStoreError::RootMustBeAbsolute);
        }
        tokio::fs::create_dir_all(&root).await.map_err(|source| {
            FilesystemRecordingStoreError::Io {
                operation: "create recording root",
                source,
            }
        })?;
        let root = tokio::fs::canonicalize(root).await.map_err(|source| {
            FilesystemRecordingStoreError::Io {
                operation: "canonicalize recording root",
                source,
            }
        })?;
        let metadata = tokio::fs::metadata(&root).await.map_err(|source| {
            FilesystemRecordingStoreError::Io {
                operation: "inspect recording root",
                source,
            }
        })?;
        if !metadata.is_dir() {
            return Err(FilesystemRecordingStoreError::RootIsNotDirectory);
        }
        Ok(Self { root })
    }

    async fn put_inner(
        &self,
        key: &str,
        body: &[u8],
    ) -> Result<StoredRecordingObject, FilesystemRecordingStoreError> {
        let relative = validate_key(key)?;
        let parent_relative = relative.parent().unwrap_or_else(|| Path::new(""));
        let requested_parent = ensure_safe_parent(&self.root, parent_relative).await?;
        let parent = tokio::fs::canonicalize(&requested_parent)
            .await
            .map_err(|source| FilesystemRecordingStoreError::Io {
                operation: "canonicalize recording object directory",
                source,
            })?;
        if !parent.starts_with(&self.root) {
            return Err(FilesystemRecordingStoreError::UnsafeObjectPath);
        }
        let file_name = relative
            .file_name()
            .ok_or(FilesystemRecordingStoreError::UnsafeObjectKey)?;
        let destination = parent.join(file_name);
        if existing_matches(&destination, body).await? {
            return Ok(StoredRecordingObject { uri: key.into() });
        }

        let temp_id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let temp = parent.join(format!(
            ".{}.tmp-{}-{temp_id}",
            file_name.to_string_lossy(),
            std::process::id()
        ));
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .await
            .map_err(|source| FilesystemRecordingStoreError::Io {
                operation: "create temporary recording object",
                source,
            })?;
        if let Err(source) = file.write_all(body).await {
            drop(file);
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(FilesystemRecordingStoreError::Io {
                operation: "write temporary recording object",
                source,
            });
        }
        if let Err(source) = file.sync_all().await {
            drop(file);
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(FilesystemRecordingStoreError::Io {
                operation: "sync temporary recording object",
                source,
            });
        }
        drop(file);

        match tokio::fs::hard_link(&temp, &destination).await {
            Ok(()) => {}
            Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
                if !existing_matches(&destination, body).await? {
                    let _ = tokio::fs::remove_file(&temp).await;
                    return Err(FilesystemRecordingStoreError::ObjectConflict(key.into()));
                }
            }
            Err(source) => {
                let _ = tokio::fs::remove_file(&temp).await;
                return Err(FilesystemRecordingStoreError::Io {
                    operation: "publish recording object",
                    source,
                });
            }
        }
        tokio::fs::remove_file(&temp).await.map_err(|source| {
            FilesystemRecordingStoreError::Io {
                operation: "remove temporary recording object",
                source,
            }
        })?;
        Ok(StoredRecordingObject { uri: key.into() })
    }
}

#[async_trait]
impl RecordingChunkStore for FilesystemRecordingStore {
    type Error = FilesystemRecordingStoreError;

    async fn put(
        &mut self,
        key: &str,
        _content_type: &str,
        body: Vec<u8>,
    ) -> Result<StoredRecordingObject, Self::Error> {
        self.put_inner(key, &body).await
    }
}

fn validate_key(value: &str) -> Result<PathBuf, FilesystemRecordingStoreError> {
    if value.is_empty()
        || value.len() > 1024
        || value.contains('\0')
        || value.contains('\\')
        || value.contains("//")
    {
        return Err(FilesystemRecordingStoreError::UnsafeObjectKey);
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(value) if !value.is_empty()))
    {
        return Err(FilesystemRecordingStoreError::UnsafeObjectKey);
    }
    Ok(path.to_path_buf())
}

async fn ensure_safe_parent(
    root: &Path,
    relative: &Path,
) -> Result<PathBuf, FilesystemRecordingStoreError> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(FilesystemRecordingStoreError::UnsafeObjectKey);
        };
        current.push(component);
        match tokio::fs::symlink_metadata(&current).await {
            Ok(metadata) if metadata.file_type().is_dir() => {}
            Ok(_) => return Err(FilesystemRecordingStoreError::UnsafeObjectPath),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match tokio::fs::create_dir(&current).await {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(source) => {
                        return Err(FilesystemRecordingStoreError::Io {
                            operation: "create recording object directory",
                            source,
                        });
                    }
                }
                let metadata = tokio::fs::symlink_metadata(&current)
                    .await
                    .map_err(|source| FilesystemRecordingStoreError::Io {
                        operation: "inspect recording object directory",
                        source,
                    })?;
                if !metadata.file_type().is_dir() {
                    return Err(FilesystemRecordingStoreError::UnsafeObjectPath);
                }
            }
            Err(source) => {
                return Err(FilesystemRecordingStoreError::Io {
                    operation: "inspect recording object directory",
                    source,
                });
            }
        }
    }
    Ok(current)
}

async fn existing_matches(
    path: &Path,
    expected: &[u8],
) -> Result<bool, FilesystemRecordingStoreError> {
    let metadata = match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(source) => {
            return Err(FilesystemRecordingStoreError::Io {
                operation: "inspect recording object",
                source,
            });
        }
    };
    if !metadata.file_type().is_file() {
        return Err(FilesystemRecordingStoreError::UnsafeObjectPath);
    }
    if metadata.len() != u64::try_from(expected.len()).unwrap_or(u64::MAX) {
        return Ok(false);
    }

    let mut file =
        tokio::fs::File::open(path)
            .await
            .map_err(|source| FilesystemRecordingStoreError::Io {
                operation: "open recording object",
                source,
            })?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0; 64 * 1024];
    loop {
        let read =
            file.read(&mut buffer)
                .await
                .map_err(|source| FilesystemRecordingStoreError::Io {
                    operation: "read recording object",
                    source,
                })?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(digest.finalize().as_slice() == Sha256::digest(expected).as_slice())
}

#[derive(Debug, thiserror::Error)]
pub enum FilesystemRecordingStoreError {
    #[error("recording root must be an absolute path")]
    RootMustBeAbsolute,
    #[error("recording root is not a directory")]
    RootIsNotDirectory,
    #[error("recording object key is not a safe relative path")]
    UnsafeObjectKey,
    #[error("recording object path escapes the configured root or is not a regular file")]
    UnsafeObjectPath,
    #[error("recording object already exists with different content: {0}")]
    ObjectConflict(String),
    #[error("failed to {operation}")]
    Io {
        operation: &'static str,
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use crate::{
        AudioFrame, AudioFrameSink, AudioFrameSinkOutput, ChunkedRecordingConfig,
        ChunkedRecordingSink,
    };

    use super::*;

    #[tokio::test]
    async fn atomically_writes_and_idempotently_reuses_recording_objects() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = FilesystemRecordingStore::new(directory.path())
            .await
            .unwrap();

        let stored = store
            .put("recordings/job-a/00000000.wav", "audio/wav", vec![1, 2, 3])
            .await
            .unwrap();
        assert_eq!(stored.uri, "recordings/job-a/00000000.wav");
        assert_eq!(
            tokio::fs::read(directory.path().join(&stored.uri))
                .await
                .unwrap(),
            vec![1, 2, 3]
        );

        store
            .put("recordings/job-a/00000000.wav", "audio/wav", vec![1, 2, 3])
            .await
            .unwrap();
        assert!(matches!(
            store
                .put("recordings/job-a/00000000.wav", "audio/wav", vec![9])
                .await,
            Err(FilesystemRecordingStoreError::ObjectConflict(_))
        ));

        let entries = std::fs::read_dir(directory.path().join("recordings/job-a"))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[tokio::test]
    async fn rejects_unsafe_keys_and_relative_roots() {
        assert!(matches!(
            FilesystemRecordingStore::new("relative-recordings").await,
            Err(FilesystemRecordingStoreError::RootMustBeAbsolute)
        ));

        let directory = tempfile::tempdir().unwrap();
        let mut store = FilesystemRecordingStore::new(directory.path())
            .await
            .unwrap();
        for key in ["../escape.wav", "/absolute.wav", "nested//empty.wav"] {
            assert!(matches!(
                store.put(key, "audio/wav", vec![1]).await,
                Err(FilesystemRecordingStoreError::UnsafeObjectKey)
            ));
        }
    }

    #[tokio::test]
    async fn persists_chunks_from_the_bounded_recorder() {
        let directory = tempfile::tempdir().unwrap();
        let store = FilesystemRecordingStore::new(directory.path())
            .await
            .unwrap();
        let mut sink = ChunkedRecordingSink::new(
            ChunkedRecordingConfig {
                object_prefix: "recordings/job-a".into(),
                chunk_duration: std::time::Duration::from_secs(1),
                max_lateness: std::time::Duration::ZERO,
            },
            store,
        )
        .unwrap();
        sink.write_frame(AudioFrame {
            sequence: 1,
            track_index: 0,
            sample_rate: 16_000,
            start_ms: 0,
            samples: vec![123],
            speaker: None,
        })
        .await
        .unwrap();

        let AudioFrameSinkOutput::RecordingChunkReady(chunk) = sink
            .finish(std::time::Duration::from_millis(1))
            .await
            .unwrap()
            .remove(0)
        else {
            panic!("expected recording chunk")
        };
        let body = tokio::fs::read(directory.path().join(chunk.uri))
            .await
            .unwrap();
        assert_eq!(&body[..4], b"RIFF");
        assert_eq!(i16::from_le_bytes([body[44], body[45]]), 123);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn refuses_parent_symlinks_that_escape_the_recording_root() {
        let directory = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), directory.path().join("linked")).unwrap();
        let mut store = FilesystemRecordingStore::new(directory.path())
            .await
            .unwrap();

        assert!(matches!(
            store.put("linked/escape.wav", "audio/wav", vec![1]).await,
            Err(FilesystemRecordingStoreError::UnsafeObjectPath)
        ));
        assert!(!outside.path().join("escape.wav").exists());
    }
}
