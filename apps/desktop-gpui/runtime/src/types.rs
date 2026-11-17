use std::sync::Arc;

use serde::{Deserialize, Serialize};

macro_rules! resource_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Arc<str>);

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value.into())
            }
        }
    };
}

resource_id!(SessionId);
resource_id!(DocumentId);
resource_id!(HumanId);
resource_id!(AttachmentId);

#[derive(Clone, Debug, thiserror::Error)]
pub enum ServiceError {
    #[error(
        "The runtime is starting or its work queue is full. Wait for readiness or pending work."
    )]
    Busy,
    #[error("The runtime is closed.")]
    Closed,
    #[error("Request cancelled.")]
    Cancelled,
    #[error("The record changed or was removed. Reload before saving.")]
    Conflict,
    #[error("Unsupported: {0}")]
    Unsupported(Arc<str>),
    #[error("{0}")]
    Failed(Arc<str>),
}

pub type Result<T> = std::result::Result<T, ServiceError>;

pub(crate) fn failure(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Failed(error.to_string().into())
}

#[derive(Clone, Debug)]
pub enum LoadState<T> {
    Pending {
        previous: Option<Arc<T>>,
    },
    Ready(Arc<T>),
    Error {
        previous: Option<Arc<T>>,
        error: ServiceError,
    },
    Unsupported {
        previous: Option<Arc<T>>,
        reason: Arc<str>,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Generation(u64);

impl Generation {
    pub fn advance(&mut self) -> Self {
        self.0 = self.0.checked_add(1).expect("request generation exhausted");
        *self
    }
}

#[derive(Default)]
pub struct RequestGate {
    generation: Generation,
    cancellation: crate::CancellationToken,
}

impl RequestGate {
    pub fn begin(&mut self) -> (Generation, crate::CancellationToken) {
        self.cancellation.cancel();
        self.cancellation = crate::CancellationToken::new();
        (self.generation.advance(), self.cancellation.clone())
    }

    pub fn is_current(&self, generation: Generation) -> bool {
        self.generation == generation && !self.cancellation.is_cancelled()
    }

    pub fn cancel(&self) {
        self.cancellation.cancel();
    }
}

impl Drop for RequestGate {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[derive(Clone, Debug)]
pub struct SessionSummary {
    pub id: SessionId,
    pub title: Arc<str>,
    pub updated_at: Arc<str>,
    pub created_at: Arc<str>,
    pub folder_path: Arc<str>,
    pub tag_line: Arc<str>,
    /// Start and end of the calendar event the note was created from (the live event when it
    /// still exists, else the copy saved on the note); empty for notes without one.
    pub event_start: Arc<str>,
    pub event_end: Arc<str>,
}

impl SessionSummary {
    /// When the note happened: its event's start, else its creation time (shipping's timeline key).
    pub fn starts_at(&self) -> &str {
        if self.event_start.is_empty() {
            &self.created_at
        } else {
            &self.event_start
        }
    }
}

#[derive(Clone, Debug)]
pub struct LibraryQuery {
    pub search: Arc<str>,
    pub offset: u32,
    /// Up to 200 rows are served from the page cache; larger requests read straight through.
    pub limit: u32,
    /// Orders the whole library oldest first instead of newest first.
    pub oldest: bool,
}

impl Default for LibraryQuery {
    fn default() -> Self {
        Self {
            search: "".into(),
            offset: 0,
            limit: 100,
            oldest: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LibraryPage {
    pub items: Arc<[SessionSummary]>,
    pub offset: u32,
    pub has_more: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DocumentSnapshot {
    pub id: DocumentId,
    pub session_id: SessionId,
    pub body_format: Arc<str>,
    /// Exact stored bytes, including unknown nodes, marks and attributes.
    pub body: Arc<str>,
    pub updated_at: Arc<str>,
}

impl DocumentSnapshot {
    /// The memo of a session that has none yet, as shipping's empty editor shows it. The first
    /// save creates the document (id = session id, as shipping's memo upsert does); it never
    /// replaces a memo that exists by then.
    pub fn unsaved_note(session: SessionId) -> Self {
        Self {
            id: DocumentId(session.0.clone()),
            session_id: session,
            body_format: "prosemirror_json".into(),
            body: r#"{"type":"doc","content":[{"type":"paragraph"}]}"#.into(),
            updated_at: "".into(),
        }
    }

    /// Not stored yet: see [`Self::unsaved_note`].
    pub fn is_unsaved(&self) -> bool {
        self.updated_at.is_empty()
    }
}

#[derive(Clone, Debug)]
pub struct OpenSession {
    pub summary: SessionSummary,
    pub note: Option<DocumentSnapshot>,
}

#[derive(Clone, Debug)]
pub struct SaveDocument {
    pub base: DocumentSnapshot,
    pub body: Arc<str>,
}

#[derive(Clone, Debug)]
pub struct RenameSession {
    pub base: SessionSummary,
    pub title: Arc<str>,
}
