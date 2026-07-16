mod config;
mod error;
mod routes;
mod shared_notes;
mod snapshot;
mod state;

pub use config::{SharedNotesConfig, SyncConfig, SyncEnv};
pub use error::{Result, SyncError};
pub use routes::{openapi, router};
pub use shared_notes::{
    SharedNotesState, openapi as shared_notes_openapi, router as shared_notes_router,
};
pub use state::AppState;
