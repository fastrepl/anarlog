#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum DownloadProgress {
    Started,
    Progress(u64, u64),
    Finished,
}
