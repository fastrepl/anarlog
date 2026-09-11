//! A session's note attachments: `<session dir>/attachments/<attachment id>`,
//! where the id is the (uniquified) file name.

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::types::{AttachmentInfo, AttachmentSaveResult};

pub fn dir(session_dir: &Path) -> PathBuf {
    session_dir.join("attachments")
}

pub fn save(session_dir: &Path, data: &[u8], filename: &str) -> Result<AttachmentSaveResult> {
    let attachments_dir = dir(session_dir);
    std::fs::create_dir_all(&attachments_dir)?;

    let safe_filename = crate::sanitize_filename(filename)?;
    let (file_path, final_filename) =
        crate::write_unique_file(&attachments_dir, &safe_filename, data)?;

    Ok(AttachmentSaveResult {
        path: file_path.to_string_lossy().to_string(),
        attachment_id: final_filename,
    })
}

pub fn list(session_dir: &Path) -> Result<Vec<AttachmentInfo>> {
    crate::list_named_files(&dir(session_dir))
}

pub fn read(session_dir: &Path, attachment_id: &str) -> Result<Vec<u8>> {
    let safe_attachment_id = crate::sanitize_filename(attachment_id)?;
    Ok(std::fs::read(dir(session_dir).join(safe_attachment_id))?)
}

pub fn remove(session_dir: &Path, attachment_id: &str) -> Result<()> {
    crate::remove_named_file(&dir(session_dir), attachment_id)
}

/// The file behind an attachment id, when it exists.
pub fn path(session_dir: &Path, attachment_id: &str) -> Option<PathBuf> {
    path_in(&dir(session_dir), attachment_id)
}

/// `path` for an attachments folder resolved already.
pub fn path_in(attachments_dir: &Path, attachment_id: &str) -> Option<PathBuf> {
    let safe_attachment_id = crate::sanitize_filename(attachment_id).ok()?;
    let path = attachments_dir.join(safe_attachment_id);
    path.is_file().then_some(path)
}
