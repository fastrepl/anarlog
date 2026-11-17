use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use desktop_runtime::{CancellationToken, Result, ServiceError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::failure;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Fingerprint {
    size: u64,
    sha256: String,
    directory: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PreparedMove {
    pub source: PathBuf,
    pub destination: PathBuf,
    staged: PathBuf,
    files: BTreeMap<PathBuf, Fingerprint>,
}

impl PreparedMove {
    pub fn prepare(source: &Path, destination: &Path, cancel: &CancellationToken) -> Result<Self> {
        let source = source.canonicalize().map_err(failure)?;
        let destination = if destination.exists() {
            destination.canonicalize().map_err(failure)?
        } else {
            let parent = destination
                .parent()
                .ok_or_else(|| failure("Destination has no parent"))?
                .canonicalize()
                .map_err(failure)?;
            parent.join(
                destination
                    .file_name()
                    .ok_or_else(|| failure("Destination has no name"))?,
            )
        };
        if destination.starts_with(&source) || source.starts_with(&destination) {
            return Err(failure("Storage locations must not contain one another"));
        }
        if destination.exists()
            && fs::read_dir(&destination)
                .map_err(failure)?
                .next()
                .is_some()
        {
            return Err(failure(
                "Choose an empty destination; existing data will never be merged or replaced",
            ));
        }
        let files = inventory(&source, cancel)?;
        let stage = tempfile::Builder::new()
            .prefix(".anarlog-copy-")
            .tempdir_in(
                destination
                    .parent()
                    .ok_or_else(|| failure("Cannot replace a filesystem root"))?,
            )
            .map_err(failure)?;
        for (relative, fingerprint) in &files {
            if cancel.is_cancelled() {
                return Err(ServiceError::Cancelled);
            }
            let target = stage.path().join(relative);
            if fingerprint.directory {
                fs::create_dir_all(&target).map_err(failure)?;
                fs::set_permissions(
                    &target,
                    fs::metadata(source.join(relative))
                        .map_err(failure)?
                        .permissions(),
                )
                .map_err(failure)?;
                continue;
            }
            fs::create_dir_all(
                target
                    .parent()
                    .ok_or_else(|| failure("Invalid storage path"))?,
            )
            .map_err(failure)?;
            let mut input = File::open(source.join(relative)).map_err(failure)?;
            let mut output = File::create(&target).map_err(failure)?;
            let mut buffer = [0u8; 64 * 1024];
            loop {
                if cancel.is_cancelled() {
                    return Err(ServiceError::Cancelled);
                }
                let count = input.read(&mut buffer).map_err(failure)?;
                if count == 0 {
                    break;
                }
                output.write_all(&buffer[..count]).map_err(failure)?;
            }
            output
                .set_permissions(input.metadata().map_err(failure)?.permissions())
                .map_err(failure)?;
            output.sync_all().map_err(failure)?;
        }
        if inventory(stage.path(), cancel)? != files || inventory(&source, cancel)? != files {
            return Err(ServiceError::Conflict);
        }
        let staged = stage.keep();
        Ok(Self {
            source,
            destination,
            staged,
            files,
        })
    }

    /// The host must hold its writer/recording pause until relaunch, including after this call.
    pub fn commit(&self, pointer: &Path, cancel: &CancellationToken) -> Result<CommittedMove> {
        let parent = pointer
            .parent()
            .ok_or_else(|| failure("Pointer has no parent"))?
            .canonicalize()
            .map_err(failure)?;
        if parent.starts_with(&self.source) || parent.starts_with(&self.destination) {
            return Err(failure(
                "Storage pointer must be outside both storage directories",
            ));
        }
        let previous = match fs::read(pointer) {
            Ok(bytes) => Some(String::from_utf8(bytes).map_err(failure)?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(failure(error)),
        };
        if inventory(&self.source, cancel)? != self.files
            || inventory(&self.staged, cancel)? != self.files
        {
            return Err(ServiceError::Conflict);
        }
        if self.destination.exists()
            && fs::read_dir(&self.destination)
                .map_err(failure)?
                .next()
                .is_some()
        {
            return Err(ServiceError::Conflict);
        }
        if cancel.is_cancelled() {
            return Err(ServiceError::Cancelled);
        }
        if self.destination.exists() {
            fs::remove_dir(&self.destination).map_err(failure)?;
        }
        if let Err(error) = fs::rename(&self.staged, &self.destination) {
            let _ = fs::create_dir(&self.destination);
            return Err(failure(error));
        }
        let value = serde_json::to_string(&self.destination).map_err(failure)?;
        if let Err(error) = anlg_storage::fs::atomic_write(pointer, &value) {
            fs::rename(&self.destination, &self.staged).map_err(failure)?;
            fs::create_dir(&self.destination).map_err(failure)?;
            return Err(failure(error));
        }
        Ok(CommittedMove {
            pointer: pointer.to_owned(),
            previous,
            installed: value,
        })
    }

    pub fn abort(self) -> Result<()> {
        if inventory(&self.staged, &CancellationToken::new())? != self.files {
            return Err(ServiceError::Conflict);
        }
        fs::remove_dir_all(self.staged).map_err(failure)
    }
}

pub struct CommittedMove {
    pointer: PathBuf,
    previous: Option<String>,
    installed: String,
}

impl CommittedMove {
    pub fn rollback(self) -> Result<()> {
        if fs::read_to_string(&self.pointer).map_err(failure)? != self.installed {
            return Err(ServiceError::Conflict);
        }
        match self.previous {
            Some(previous) => {
                anlg_storage::fs::atomic_write(&self.pointer, &previous).map_err(failure)
            }
            None => fs::remove_file(&self.pointer).map_err(failure),
        }
    }
}

fn inventory(root: &Path, cancel: &CancellationToken) -> Result<BTreeMap<PathBuf, Fingerprint>> {
    let mut result = BTreeMap::new();
    let mut pending = vec![root.to_owned()];
    let mut buffer = vec![0; 64 * 1024];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).map_err(failure)? {
            if cancel.is_cancelled() {
                return Err(ServiceError::Cancelled);
            }
            let entry = entry.map_err(failure)?;
            let kind = entry.file_type().map_err(failure)?;
            if kind.is_symlink() || (!kind.is_file() && !kind.is_dir()) {
                return Err(failure(
                    "Storage contains links or special files; resolve them before moving",
                ));
            }
            if kind.is_dir() {
                result.insert(
                    entry.path().strip_prefix(root).map_err(failure)?.to_owned(),
                    Fingerprint {
                        size: 0,
                        sha256: String::new(),
                        directory: true,
                    },
                );
                pending.push(entry.path());
                continue;
            }
            let mut file = File::open(entry.path()).map_err(failure)?;
            let mut digest = Sha256::new();
            let mut size = 0;
            loop {
                if cancel.is_cancelled() {
                    return Err(ServiceError::Cancelled);
                }
                let count = file.read(&mut buffer).map_err(failure)?;
                if count == 0 {
                    break;
                }
                digest.update(&buffer[..count]);
                size += count as u64;
            }
            let relative = entry.path().strip_prefix(root).map_err(failure)?.to_owned();
            result.insert(
                relative,
                Fingerprint {
                    size,
                    directory: false,
                    sha256: digest
                        .finalize()
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect(),
                },
            );
            if result.len() > 1_000_000 {
                return Err(failure("Storage inventory exceeds one million files"));
            }
        }
    }
    Ok(result)
}

pub fn install_copy(source: &Path, target: &Path) -> Result<()> {
    let mut file = File::open(source).map_err(failure)?;
    let parent = target
        .parent()
        .ok_or_else(|| failure("Missing install directory"))?;
    fs::create_dir_all(parent).map_err(failure)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(failure)?;
    std::io::copy(&mut file, &mut temporary).map_err(failure)?;
    temporary.flush().map_err(failure)?;
    temporary
        .as_file()
        .set_permissions(file.metadata().map_err(failure)?.permissions())
        .map_err(failure)?;
    temporary.as_file().sync_all().map_err(failure)?;
    temporary.persist_noclobber(target).map_err(failure)?;
    Ok(())
}
