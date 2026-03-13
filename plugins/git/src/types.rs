use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct StatusInfo {
    pub staged: Vec<FileStatus>,
    pub unstaged: Vec<FileStatus>,
    pub untracked: Vec<String>,
    pub conflicted: Vec<String>,
    pub has_changes: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FileStatus {
    pub path: String,
    pub status: FileChangeType,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum FileChangeType {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CommitInfo {
    pub id: String,
    pub message: String,
    pub author: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RemoteInfo {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RemoteStatus {
    pub ahead: u32,
    pub behind: u32,
    pub remote_name: String,
    pub branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum PullResult {
    Success { commits_pulled: u32 },
    AlreadyUpToDate,
    Conflicts { files: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum PushResult {
    Success { commits_pushed: u32 },
    AlreadyUpToDate,
    Rejected { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ConflictInfo {
    pub files: Vec<String>,
}
