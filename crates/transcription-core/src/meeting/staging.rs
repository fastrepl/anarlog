//! Raw helper files for one capture live under `<root>/<capture_id>/` with a manifest naming the
//! capture. They are a temporary second copy: removed once Anarlog adopted the audio, kept for
//! recovery otherwise, and never kept past `STAGING_MAX_AGE`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anlg_audio::meeting::CaptureIdentity;

pub const STAGING_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const MANIFEST: &str = "capture.json";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StagedCapture {
    pub capture: CaptureIdentity,
    pub created_at_ms: i64,
}

/// A new, empty directory for one engine open of this capture.
pub fn create_staging_dir(root: &Path, capture: &CaptureIdentity) -> io::Result<PathBuf> {
    let capture_dir = capture_dir(root, &capture.capture_id)?;
    create_private_dir(&capture_dir)?;
    let manifest = capture_dir.join(MANIFEST);
    if !manifest.exists() {
        let staged = StagedCapture {
            capture: capture.clone(),
            created_at_ms: super::now_ms(),
        };
        let temp = capture_dir.join(format!("{MANIFEST}.tmp"));
        fs::write(&temp, serde_json::to_vec(&staged)?)?;
        fs::rename(temp, manifest)?;
    }
    let open = capture_dir.join(uuid::Uuid::new_v4().simple().to_string());
    create_private_dir(&open)?;
    Ok(open)
}

pub fn remove_staging(root: &Path, capture_id: &str) -> io::Result<()> {
    match fs::remove_dir_all(capture_dir(root, capture_id)?) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

/// Removes staged captures that are not active and were adopted, or are too old to recover.
/// Returns the removed capture ids.
pub fn sweep_staging(
    root: &Path,
    active_capture_id: Option<&str>,
    adopted: impl Fn(&StagedCapture) -> bool,
    max_age: Duration,
) -> io::Result<Vec<String>> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut removed = Vec::new();
    for entry in entries.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if Some(name.as_str()) == active_capture_id || !entry.path().is_dir() {
            continue;
        }
        let staged = fs::read(entry.path().join(MANIFEST))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<StagedCapture>(&bytes).ok())
            .filter(|staged| staged.capture.capture_id == name);
        let expired = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age > max_age);
        // An unreadable manifest cannot be matched to a recording, so only age removes it.
        if expired || staged.as_ref().is_some_and(&adopted) {
            fs::remove_dir_all(entry.path())?;
            removed.push(name);
        }
    }
    Ok(removed)
}

pub fn list_staging(root: &Path) -> Vec<StagedCapture> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| fs::read(entry.path().join(MANIFEST)).ok())
        .filter_map(|bytes| serde_json::from_slice(&bytes).ok())
        .collect()
}

fn capture_dir(root: &Path, capture_id: &str) -> io::Result<PathBuf> {
    let valid = !capture_id.is_empty()
        && capture_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-');
    if !valid {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid capture id",
        ));
    }
    Ok(root.join(capture_id))
}

fn create_private_dir(path: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}
