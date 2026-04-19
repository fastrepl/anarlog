#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum SyncStatus {
    Idle,
    Scheduled,
    Running,
}

#[derive(Debug, Clone)]
pub enum CalendarSyncWorkerEvent {
    StatusChanged { status: SyncStatus },
    SyncStarted,
    SyncFinished { data_changed: bool },
    SyncFailed { error: String },
}

pub trait CalendarSyncRuntime: Send + Sync + 'static {
    fn emit(&self, event: CalendarSyncWorkerEvent);
}
