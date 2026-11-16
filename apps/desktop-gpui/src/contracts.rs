use std::sync::Arc;

use desktop_runtime::{
    AttachmentId, DocumentSnapshot, HumanId, RuntimeHandle, ServiceError, SessionId,
};
use gpui::FocusHandle;

#[derive(Clone)]
pub struct LaneContext {
    pub runtime: RuntimeHandle,
}

#[derive(Clone)]
pub struct EditorInit {
    pub session_id: SessionId,
    pub document: DocumentSnapshot,
    pub return_focus: FocusHandle,
}

#[derive(Clone, Debug)]
pub enum EditorEvent {
    Dirty {
        session_id: SessionId,
        dirty: bool,
    },
    Saved(DocumentSnapshot),
    SaveFailed {
        session_id: SessionId,
        error: ServiceError,
    },
    OpenLink(Arc<str>),
    OpenAttachment(AttachmentId),
    MentionHuman(HumanId),
}

#[derive(Clone, Debug)]
pub enum MeetingIntent {
    Open(SessionId),
    Start { session_id: SessionId },
    Stop,
}

#[derive(Clone, Debug)]
pub enum MeetingEvent {
    NoteEnhanced(SessionId),
    Recording { session_id: SessionId, active: bool },
    OpenSession(SessionId),
    Failed(ServiceError),
}

#[derive(Clone, Debug)]
pub enum ProductRoute {
    Onboarding,
    Permissions,
    Settings,
    Transcription,
    Intelligence,
    Account,
    Billing,
    CloudSync,
    Share(SessionId),
    Import,
    Export(SessionId),
    Models,
    Integrations,
    Calendar,
}

#[derive(Clone, Debug)]
pub enum ProductEvent {
    NavigateWorkspace,
    OpenSession(SessionId),
    Failed(ServiceError),
}

#[derive(Clone, Debug)]
pub enum WorkspaceEvent {
    SessionsDeleted(Arc<[SessionId]>),
    AttachFile(SessionId),
    OpenEditor {
        session_id: SessionId,
        document: DocumentSnapshot,
    },
    Meeting(MeetingIntent),
    Product(ProductRoute),
}
